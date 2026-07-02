//! Auto-match engine — background task that scans on-chain orders and
//! automatically matches qualified ones by opening fresh Fiber channels.
//!
//! The contract requires `channel_block > order_block`, so a new channel
//! must be opened for every match — existing channels can never be reused.
//!
//! Runs on a configurable interval, applies filters from `Config`, and
//! matches each eligible order using the configured signer and wallet.

use tracing::{debug, info};

use opticrum_calculator::config::ORDER_TO_MATCH_CAPACITY_RESERVE;

use crate::db::DbPool;

use std::sync::{Arc, RwLock};

use crate::db::matches as match_db;
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
    pool: &DbPool,
    chain_provider: &(dyn ChainProvider + Send + Sync),
    signer: &(dyn Signer + Send + Sync),
    runtime_config: &Arc<RwLock<RuntimeConfig>>,
) -> Result<u64, AppError> {
    let rc = runtime_config.read().unwrap();
    if !rc.auto_match_enabled {
        return Ok(0);
    }
    let min_capacity = rc.auto_match_min_capacity;
    let max_escrow_blocks = rc.auto_match_max_escrow_blocks;
    drop(rc);

    let mut conn = pool.get()?;

    // Scan all live orders on chain
    let orders = chain_provider.scan_orders().await?;
    debug!(on_chain_orders = orders.len(), "Auto-match: scanned chain");

    // Get already-matched order outpoints from local DB to skip them
    let existing_matches = match_db::list_matches(&mut conn, None)?;
    let matched_outpoints: Vec<(String, i32)> = existing_matches
        .iter()
        .map(|m| (m.order_tx_hash.clone(), m.order_output_index))
        .collect();

    // Filter and match — each match gets a fresh channel (contract requirement).
    let mut matched_count = 0u64;
    let max_per_cycle = 10u64; // safety cap to avoid runaway matching

    for order in &orders {
        if matched_count >= max_per_cycle {
            break;
        }

        // Skip if already matched
        if matched_outpoints.iter().any(|(h, i)| {
            h == &hex::encode(order.order_outpoint.tx_hash)
                && *i == order.order_outpoint.index as i32
        }) {
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
            signer_address: None, // TODO: set to the actual seller address when multi-address signing is needed
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

        // Record in local DB
        let shannons_per_block = order.order_data.shannons_per_block;
        match_db::insert_match(
            &mut conn,
            &tx_hash,
            0,
            &hex::encode(order.order_outpoint.tx_hash),
            order.order_outpoint.index as i32,
            seller_address,
            shannons_per_block,
            order.ckb_capacity,
            None::<&str>,
            Some(&channel.channel_id),
        )?;

        info!(
            order_tx = %hex::encode(order.order_outpoint.tx_hash),
            channel = %channel.tx_hash,
            channel_index = channel.output_index,
            match_tx = %tx_hash,
            "Auto-match: order matched"
        );

        matched_count += 1;
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
