//! Auto-match engine — background task that scans on-chain orders and
//! automatically matches qualified ones against available Fiber channels.
//!
//! Runs on a configurable interval, applies filters from `Config`, and
//! matches each eligible order using the configured signer and wallet.

use tracing::{debug, info};

use crate::db::DbPool;

use crate::config::Config;
use crate::db::matches as match_db;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::signer::{SignRequest, Signer};

/// Run one auto-match cycle.
///
/// Scans the chain for live orders, filters by config criteria,
/// and attempts to match each eligible order. Returns the number
/// of orders matched in this cycle.
pub async fn run_auto_match_cycle(
    pool: &DbPool,
    chain_provider: &(dyn ChainProvider + Send + Sync),
    signer: &(dyn Signer + Send + Sync),
    config: &Config,
) -> Result<u64, AppError> {
    if !config.auto_match_enabled {
        return Ok(0);
    }

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

    // Scan available Fiber channels
    let channels = chain_provider.scan_fiber_channels(&[]).await?;
    if channels.is_empty() {
        debug!("Auto-match: no Fiber channels available, skipping cycle");
        return Ok(0);
    }
    debug!(available_channels = channels.len(), "Auto-match: channels found");

    // Filter and match
    let mut matched_count = 0u64;
    let max_per_cycle = 10u64; // safety cap to avoid runaway matching

    for order in &orders {
        if matched_count >= max_per_cycle {
            break;
        }

        let _order_outpoint = (
            hex::encode(order.order_args.fiber_pubkey),
            order.order_outpoint.index as i32,
        );

        // Skip if already matched
        if matched_outpoints.iter().any(|(h, i)| {
            h == &hex::encode(order.order_outpoint.tx_hash)
                && *i == order.order_outpoint.index as i32
        }) {
            continue;
        }

        // Apply config filters
        if order.ckb_capacity < config.auto_match_min_capacity {
            continue;
        }
        if order.order_data.escrow_blocks > config.auto_match_max_escrow_blocks {
            continue;
        }

        // Find a compatible channel (capacity >= order capacity)
        let compatible_channel = channels.iter().find(|ch| ch.capacity >= order.ckb_capacity);
        let channel = match compatible_channel {
            Some(c) => c,
            None => {
                debug!(
                    required_capacity = order.ckb_capacity,
                    "Auto-match: no compatible channel found, skipping order"
                );
                continue;
            }
        };

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
        };

        let sign_result = signer.sign(sign_request).await?;
        let signed_tx_hex = match sign_result {
            crate::services::signer::SignResult::Signed { tx_hex } => tx_hex,
            crate::services::signer::SignResult::Unsigned { .. } => {
                tracing::warn!(
                    "Auto-match: external signer cannot auto-sign — skipping order {}",
                    hex::encode(order.order_outpoint.tx_hash)
                );
                continue;
            }
        };

        // Submit to chain
        let tx_hash = chain_provider.send_transaction(&signed_tx_hex).await?;

        // Record in local DB
        let rent_per_block = order.ckb_capacity as f64 / order.order_data.escrow_blocks as f64;
        match_db::insert_match(
            &mut conn,
            &tx_hash,
            0,
            &hex::encode(order.order_outpoint.tx_hash),
            order.order_outpoint.index as i32,
            seller_address,
            rent_per_block,
            order.order_data.escrow_blocks,
            None::<&str>,
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
        info!(matched = matched_count, total_scanned = orders.len(), "Auto-match cycle complete");
    }

    Ok(matched_count)
}
