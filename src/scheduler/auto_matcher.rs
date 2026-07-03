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
) -> Result<u64, AppError> {
    let (min_capacity, max_escrow_blocks) = {
        let rc = runtime_config.read().unwrap();
        if !rc.auto_match_enabled {
            return Ok(0);
        }
        (rc.auto_match_min_capacity, rc.auto_match_max_escrow_blocks)
    };

    // Scan all live orders and matches on chain
    let orders = chain_provider.scan_orders().await?;
    debug!(on_chain_orders = orders.len(), "Auto-match: scanned chain");

    // Build dedup set from on-chain match cells (order outpoint matching)
    let on_chain_matches = chain_provider.scan_matches().await?;

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

    for order in &orders {
        if matched_count >= max_per_cycle {
            break;
        }

        // Skip if already matched (dedup by fiber_pubkey)
        let order_key = (hex::encode(order.order_args.fiber_pubkey.to_bytes()),);
        if matched_order_keys.contains(&order_key) {
            continue;
        }

        // Apply config filters
        if order.ckb_capacity < min_capacity {
            debug!(
                capacity = order.ckb_capacity,
                min = min_capacity,
                "Auto-match: skipped — capacity below minimum"
            );
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
                continue;
            }
        }

        // Open a fresh channel for this order (contract requires channel
        // created AFTER the order).
        let fiber_pubkey_hex = hex::encode(order.order_args.fiber_pubkey.to_bytes());
        let required_capacity = order.ckb_capacity + CHANNEL_CELL_OCCUPIED_RESERVE;

        // Fiber requires the peer to be connected before open_channel.
        let _ = chain_provider.connect_peer(&fiber_pubkey_hex).await;

        info!(
            peer = %fiber_pubkey_hex,
            amount = required_capacity,
            "Auto-match: opening fresh channel"
        );
        if let Err(e) = chain_provider
            .open_channel(&fiber_pubkey_hex, required_capacity)
            .await
        {
            tracing::warn!(
                error = %e,
                peer = %fiber_pubkey_hex,
                "Auto-match: open_channel failed — skipping order"
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
                continue;
            }
        };
        info!(
            channel_id = %channel.channel_id,
            tx_hash = %channel.tx_hash,
            "Auto-match: fresh channel ready"
        );

        // Attempt to match
        let seller_address = "auto-matcher"; // Phase 6: derive from signer wallet
        let tx_hex = format!(
            "auto_match:{}:{}:{}:{}",
            hex::encode(order.order_outpoint.tx_hash),
            order.order_outpoint.index,
            channel.tx_hash,
            channel.output_index
        );

        // Sign the match transaction
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
            signer_address: None,
        };

        let sign_result = signer.sign(sign_request).await?;
        let signed_tx_hex = match sign_result {
            crate::services::signer::SignResult::Signed { tx_hex } => tx_hex,
            crate::services::signer::SignResult::Unsigned { .. } => {
                tracing::warn!(
                    "Auto-match: HD wallet is locked — skipping order {}",
                    hex::encode(order.order_outpoint.tx_hash)
                );
                continue;
            }
        };

        // Submit to chain
        let tx_hash = chain_provider.send_transaction(&signed_tx_hex).await?;

        info!(
            order_tx = %hex::encode(order.order_outpoint.tx_hash),
            channel = %channel.tx_hash,
            channel_index = channel.output_index,
            match_tx = %tx_hash,
            "Auto-match: order matched"
        );

        matched_count += 1;
        // The match cell is now on-chain — no DB write needed.
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
