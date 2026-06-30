//! Match service — match on-chain orders with Fiber channels.
//!
//! When a seller matches an order, the service:
//! 1. Looks for an existing compatible channel (ChannelReady, same counterparty,
//!    enough capacity). Reuses it if found.
//! 2. If no compatible channel exists, calls `open_channel` on the Fiber RPC
//!    and polls `list_channels` until the channel reaches ChannelReady.
//! 3. Builds the match transaction with the channel outpoint and records the
//!    result in the local database.
//!
//! Transaction assembly is delegated to `TransactionAssembler` when available.

use std::time::Duration;
use tracing::{debug, info};

use crate::db::matches as match_db;
use crate::db::DbPool;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;

/// Result of matching an order.
#[derive(serde::Serialize, Debug)]
pub struct MatchOrderResult {
    pub tx_hash: String,
    pub output_index: i32,
    pub match_id: i64,
}

/// Maximum time to wait for a newly-opened channel to reach ChannelReady.
const CHANNEL_READY_TIMEOUT_SECS: u64 = 120;
/// Initial polling interval.
const POLL_INITIAL_SECS: u64 = 2;
/// Polling interval after ramp-up.
const POLL_LATER_SECS: u64 = 5;
/// Switch to longer interval after this many seconds.
const POLL_RAMP_SECS: u64 = 30;

/// Match an on-chain order with a Fiber channel.
///
/// The channel outpoint is resolved automatically:
/// - If a compatible ChannelReady channel to the buyer's Fiber pubkey already
///   exists, it is reused.
/// - Otherwise a new channel is opened and the call blocks until it becomes
///   ChannelReady (up to 120 seconds).
pub async fn match_order<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    order_tx_hash: &str,
    order_output_index: u32,
    seller_address: &str,
) -> Result<MatchOrderResult, AppError> {
    // ── 1. Fetch the order from on-chain scan ──────────────────────────
    let orders = provider.scan_orders().await?;
    let order = orders
        .into_iter()
        .find(|o| {
            hex::encode(o.order_outpoint.tx_hash) == order_tx_hash
                && o.order_outpoint.index == order_output_index
        })
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "Order {order_tx_hash}:{order_output_index} not found on chain"
            ))
        })?;

    let fiber_pubkey_hex = hex::encode(order.order_args.fiber_pubkey.to_bytes());
    let required_capacity = order.order_data.channel_capacity;
    info!(
        order_tx = %order_tx_hash,
        peer = %fiber_pubkey_hex,
        capacity = required_capacity,
        "Match: looking for compatible channel"
    );

    // ── 2. Check for an existing compatible channel ────────────────────
    let existing = find_compatible_channel(provider, &fiber_pubkey_hex, required_capacity).await?;
    let (channel_tx_hash, channel_output_index) = if let Some(ch) = existing {
        info!(
            channel_id = %ch.channel_id,
            tx_hash = %ch.tx_hash,
            "Match: reusing existing channel"
        );
        (ch.tx_hash, ch.output_index)
    } else {
        // ── 3. Open a new channel ──────────────────────────────────────
        info!(
            peer = %fiber_pubkey_hex,
            amount = required_capacity,
            "Match: opening new channel"
        );
        let _temp_id = provider
            .open_channel(&fiber_pubkey_hex, required_capacity)
            .await?;

        // ── 4. Poll until channel is ChannelReady ──────────────────────
        let channel = wait_for_channel_ready(
            provider,
            &fiber_pubkey_hex,
            required_capacity,
            CHANNEL_READY_TIMEOUT_SECS,
        )
        .await?;
        info!(
            channel_id = %channel.channel_id,
            tx_hash = %channel.tx_hash,
            "Match: new channel ready"
        );
        (channel.tx_hash, channel.output_index)
    };

    // ── 5. Build match transaction ─────────────────────────────────────
    let tx_hex = format!(
        "match_order:{}:{}:{}:{}",
        order_tx_hash, order_output_index, channel_tx_hash, channel_output_index
    );
    let tx_hash = provider.send_transaction(&tx_hex).await?;
    let output_index = 0; // Match Cell is output[0]

    // ── 6. Persist tracked match ───────────────────────────────────────
    let shannons_per_block = order.order_data.shannons_per_block;
    let mut conn = pool.get()?;
    let match_id = match_db::insert_match(
        &mut conn,
        &tx_hash,
        output_index,
        order_tx_hash,
        order_output_index as i32,
        seller_address,
        shannons_per_block,
        None::<&str>,
    )?;

    info!(
        match_id = match_id,
        tx_hash = %tx_hash,
        seller = %seller_address,
        channel_tx = %channel_tx_hash,
        channel_index = channel_output_index,
        order_tx = %order_tx_hash,
        "Order matched"
    );

    Ok(MatchOrderResult {
        tx_hash,
        output_index,
        match_id,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Find an existing ChannelReady channel that matches the given counterparty
/// pubkey and has at least the required capacity.
async fn find_compatible_channel<P: ChainProvider + ?Sized>(
    provider: &P,
    counterparty_pubkey: &str,
    min_capacity: u64,
) -> Result<Option<crate::services::chain_provider::FiberChannelInfo>, AppError> {
    let channels = provider.scan_fiber_channels(&[]).await?;
    Ok(channels.into_iter().find(|ch| {
        ch.counterparty_fiber_key == counterparty_pubkey
            && ch.state_name == "ChannelReady"
            && ch.capacity >= min_capacity
    }))
}

/// Poll `scan_fiber_channels` until a ChannelReady channel matching the
/// given counterparty pubkey and capacity appears, or the timeout is reached.
async fn wait_for_channel_ready<P: ChainProvider + ?Sized>(
    provider: &P,
    counterparty_pubkey: &str,
    min_capacity: u64,
    timeout_secs: u64,
) -> Result<crate::services::chain_provider::FiberChannelInfo, AppError> {
    let started = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        if started.elapsed() >= timeout {
            return Err(AppError::ChainError(format!(
                "Channel to {counterparty_pubkey} did not become ready within {timeout_secs}s"
            )));
        }

        // Determine poll interval: shorter early, longer later.
        let elapsed = started.elapsed().as_secs();
        let delay = if elapsed < POLL_RAMP_SECS {
            POLL_INITIAL_SECS
        } else {
            POLL_LATER_SECS
        };

        let channels = provider.scan_fiber_channels(&[]).await?;
        let found = channels.into_iter().find(|ch| {
            ch.counterparty_fiber_key == counterparty_pubkey
                && ch.state_name == "ChannelReady"
                && ch.capacity >= min_capacity
        });

        match found {
            Some(ch) => return Ok(ch),
            None => {
                debug!(
                    elapsed = elapsed,
                    "Match: channel not ready yet, retrying in {}s", delay
                );
                actix_rt::time::sleep(Duration::from_secs(delay)).await;
            }
        }
    }
}

/// List tracked matches.
pub fn list_matches(
    pool: &DbPool,
    status_filter: Option<&str>,
) -> Result<Vec<match_db::TrackedMatch>, AppError> {
    let mut conn = pool.get()?;
    let matches = match_db::list_matches(&mut conn, status_filter)?;
    debug!(
        count = matches.len(),
        filter = status_filter.unwrap_or("all"),
        "Matches listed"
    );
    Ok(matches)
}

/// Get a single tracked match by ID.
pub fn get_match(pool: &DbPool, match_id: i64) -> Result<match_db::TrackedMatch, AppError> {
    let mut conn = pool.get()?;
    match_db::get_match_by_id(&mut conn, match_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::services::MockChainProvider;

    #[actix_rt::test]
    async fn match_order_fails_when_order_not_on_chain() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        // No orders on chain → should fail with "not found on chain".
        let result = match_order(&provider, &pool, "order_not_found", 0, "ckt1q...seller").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found on chain"));
    }

    #[actix_rt::test]
    async fn match_order_no_channels_attempts_open() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        // No orders on chain, no channels → fails at order lookup first.
        let result = match_order(&provider, &pool, "any_order", 0, "ckt1q...seller").await;
        assert!(result.is_err());

        // open_channel was never called because order lookup fails first.
        let calls = provider.open_channels.lock().unwrap();
        assert!(calls.is_empty());
    }
}
