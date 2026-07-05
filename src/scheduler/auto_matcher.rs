//! Auto-match engine — background task that scans on-chain orders and
//! automatically matches qualified ones by opening fresh Fiber channels.
//!
//! The contract requires `channel_block > order_block`, so a new channel
//! must be opened for every match — existing channels can never be reused.
//!
//! Runs on a configurable interval, applies filters from `Config`, and
//! matches each eligible order using the configured signer and wallet.
//!
//! After the chain-first refactor, deduplication uses on-chain match scans
//! instead of a database table. No match records are written to DB.

use tracing::{debug, info};

use opticrum_calculator::config::ORDER_TO_MATCH_CAPACITY_RESERVE;

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::console::scheduler_state::{push_event, trunc_hex, SharedSchedulerState};
use crate::services::match_service::{
    wait_for_channel_ready, CHANNEL_CELL_OCCUPIED_RESERVE, CHANNEL_READY_TIMEOUT_SECS,
};
use crate::services::signer::{SignRequest, Signer};
use crate::services::RuntimeConfig;

/// Run one auto-match cycle.
///
/// Scans the chain for live orders, filters by config criteria,
/// and attempts to match each eligible order. Returns the number
/// of orders matched in this cycle.
pub async fn run_auto_match_cycle(
    chain_provider: &(dyn ChainProvider + Send + Sync),
    signer: &(dyn Signer + Send + Sync),
    runtime_config: &Arc<RwLock<RuntimeConfig>>,
    scheduler_state: Option<&SharedSchedulerState>,
) -> Result<u64, AppError> {
    push_event(
        scheduler_state,
        "matcher",
        "info",
        "Cycle started — scanning on-chain orders",
    );

    // Bail early if the signer is locked — don't waste chain resources
    // opening channels we can't sign for.
    if !signer.is_unlocked() {
        debug!("Auto-match: HD wallet locked — skipping cycle");
        push_event(
            scheduler_state,
            "matcher",
            "warn",
            "HD wallet locked — cycle skipped (no signing)",
        );
        return Ok(0);
    }

    let (min_capacity, max_escrow_blocks, automation_signer_address) = {
        let rc = runtime_config.read().unwrap();
        if !rc.auto_match_enabled {
            return Ok(0);
        }
        (
            rc.auto_match_min_capacity,
            rc.auto_match_max_escrow_blocks,
            rc.automation_signer_address.clone(),
        )
    };

    let orders = chain_provider.scan_orders().await?;
    debug!(on_chain_orders = orders.len(), "Auto-match: scanned chain");

    let on_chain_matches = chain_provider.scan_matches().await?;

    push_event(
        scheduler_state,
        "matcher",
        "info",
        format!(
            "Chain scan — {} live orders, {} on-chain matches",
            orders.len(),
            on_chain_matches.len()
        ),
    );

    // Build set of already-matched order outpoints from on-chain match data
    // Each MatchInfo has the order_args which can identify the order
    let matched_order_keys: HashSet<(String,)> = on_chain_matches
        .iter()
        .map(|m| {
            // Match cells reference orders by fiber_pubkey + buyer_lock_hash + shannons_per_block
            // Use order fiber_pubkey as the dedup key (each buyer has one active order)
            (hex::encode(m.match_args.order_args.fiber_pubkey.to_bytes()),)
        })
        .collect();

    let mut matched_count = 0u64;
    let max_per_cycle = 10u64; // safety cap to avoid runaway matching
    let mut skip_already_matched = 0u64;
    let mut skip_low_capacity = 0u64;
    let mut skip_high_escrow = 0u64;
    let mut skip_open_channel = 0u64;
    let mut skip_channel_ready = 0u64;
    let mut skip_sign_locked = 0u64;

    for order in &orders {
        if matched_count >= max_per_cycle {
            break;
        }

        // Skip if already matched (dedup by fiber_pubkey)
        let order_key = (hex::encode(order.order_args.fiber_pubkey.to_bytes()),);
        if matched_order_keys.contains(&order_key) {
            skip_already_matched += 1;
            continue;
        }

        // Apply config filters
        if order.ckb_capacity < min_capacity {
            debug!(
                capacity = order.ckb_capacity,
                min = min_capacity,
                "Auto-match: skipped — capacity below minimum"
            );
            skip_low_capacity += 1;
            continue;
        }
        // Derive effective escrow blocks from capacity / rent rate.
        let effective_capacity = order
            .ckb_capacity
            .saturating_sub(ORDER_TO_MATCH_CAPACITY_RESERVE);
        if let Some(effective_escrow) =
            effective_capacity.checked_div(order.order_data.shannons_per_block)
        {
            if effective_escrow > max_escrow_blocks {
                debug!(
                    effective_escrow,
                    max = max_escrow_blocks,
                    "Auto-match: skipped — escrow blocks above maximum"
                );
                skip_high_escrow += 1;
                continue;
            }
        }

        // Open a fresh channel for this order (contract requires channel
        // created AFTER the order).
        let fiber_pubkey_hex = hex::encode(order.order_args.fiber_pubkey.to_bytes());
        let fiber_address = order.fiber_address.as_deref();
        let required_capacity = order.ckb_capacity + CHANNEL_CELL_OCCUPIED_RESERVE;

        // Fiber requires the peer to be connected before open_channel.
        let _ = chain_provider
            .connect_peer(&fiber_pubkey_hex, fiber_address)
            .await;

        info!(
            peer = %fiber_pubkey_hex,
            amount = required_capacity,
            "Auto-match: opening fresh channel"
        );
        push_event(
            scheduler_state,
            "matcher",
            "info",
            format!(
                "Opening Fiber channel — peer {} · capacity {} shannons",
                trunc_hex(&fiber_pubkey_hex, 8, 6),
                required_capacity
            ),
        );
        if let Err(e) = chain_provider
            .open_channel(&fiber_pubkey_hex, required_capacity, fiber_address)
            .await
        {
            tracing::warn!(
                error = %e,
                peer = %fiber_pubkey_hex,
                "Auto-match: open_channel failed — skipping order"
            );
            skip_open_channel += 1;
            push_event(
                scheduler_state,
                "matcher",
                "warn",
                format!(
                    "open_channel failed for peer {} — {}",
                    trunc_hex(&fiber_pubkey_hex, 8, 6),
                    e
                ),
            );
            continue;
        }

        // Poll until the new channel reaches ChannelReady
        let channel = match wait_for_channel_ready(
            chain_provider,
            &fiber_pubkey_hex,
            required_capacity,
            CHANNEL_READY_TIMEOUT_SECS,
        )
        .await
        {
            Ok(ch) => ch,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    peer = %fiber_pubkey_hex,
                    "Auto-match: channel did not become ready — skipping order"
                );
                skip_channel_ready += 1;
                push_event(
                    scheduler_state,
                    "matcher",
                    "warn",
                    format!(
                        "Channel not ready for peer {} — {}",
                        trunc_hex(&fiber_pubkey_hex, 8, 6),
                        e
                    ),
                );
                continue;
            }
        };
        info!(
            channel_id = %channel.channel_id,
            tx_hash = %channel.tx_hash,
            "Auto-match: fresh channel ready"
        );
        push_event(
            scheduler_state,
            "matcher",
            "info",
            format!(
                "Channel ready — {}:{}",
                trunc_hex(&channel.tx_hash, 8, 6),
                channel.output_index
            ),
        );

        // Attempt to match
        let seller_address = if automation_signer_address.is_empty() {
            "auto-matcher".to_string()
        } else {
            automation_signer_address.clone()
        };
        let tx_hex = format!(
            "auto_match:{}:{}:{}:{}",
            hex::encode(order.order_outpoint.tx_hash),
            order.order_outpoint.index,
            channel.tx_hash,
            channel.output_index
        );

        // Sign the match transaction
        let signer_address = if automation_signer_address.is_empty() {
            None
        } else {
            Some(automation_signer_address.clone())
        };
        let sign_request = SignRequest {
            operation: "match_order".into(),
            tx_hex: tx_hex.clone(),
            context: serde_json::json!({
                "order_outpoint": {
                    "tx_hash": hex::encode(order.order_outpoint.tx_hash),
                    "index": order.order_outpoint.index,
                },
                "channel_outpoint": {
                    "tx_hash": channel.tx_hash,
                    "index": channel.output_index,
                },
                "seller_address": seller_address,
            }),
            signer_address,
        };

        let sign_result = signer.sign(sign_request).await?;
        let signed_tx_hex = match sign_result {
            crate::services::signer::SignResult::Signed { tx_hex } => tx_hex,
            crate::services::signer::SignResult::Unsigned { .. } => {
                tracing::warn!(
                    "Auto-match: HD wallet is locked — skipping order {}",
                    hex::encode(order.order_outpoint.tx_hash)
                );
                skip_sign_locked += 1;
                continue;
            }
        };

        // Submit to chain
        let tx_hash = chain_provider.send_transaction(&signed_tx_hex).await?;

        let order_tx = hex::encode(order.order_outpoint.tx_hash);
        info!(
            order_tx = %order_tx,
            channel = %channel.tx_hash,
            channel_index = channel.output_index,
            match_tx = %tx_hash,
            "Auto-match: order matched"
        );
        push_event(
            scheduler_state,
            "matcher",
            "info",
            format!(
                "Match submitted — order {}:{} → match tx {}",
                trunc_hex(&order_tx, 8, 6),
                order.order_outpoint.index,
                trunc_hex(&tx_hash, 8, 6)
            ),
        );

        matched_count += 1;
        // The match cell is now on-chain — no DB write needed.
    }

    let total_skipped = skip_already_matched
        + skip_low_capacity
        + skip_high_escrow
        + skip_open_channel
        + skip_channel_ready
        + skip_sign_locked;
    if total_skipped > 0 {
        push_event(
            scheduler_state,
            "matcher",
            "info",
            format!(
                "Cycle finished — matched {}, skipped {} (already matched {}, low capacity {}, high escrow {}, open_channel {}, channel ready {}, sign locked {}) of {} orders",
                matched_count,
                total_skipped,
                skip_already_matched,
                skip_low_capacity,
                skip_high_escrow,
                skip_open_channel,
                skip_channel_ready,
                skip_sign_locked,
                orders.len()
            ),
        );
    } else {
        push_event(
            scheduler_state,
            "matcher",
            "info",
            format!(
                "Cycle finished — {} matched of {} orders scanned",
                matched_count,
                orders.len()
            ),
        );
    }

    if matched_count > 0 {
        info!(
            matched = matched_count,
            total_scanned = orders.len(),
            "Auto-match cycle complete"
        );
    }

    Ok(matched_count)
}
