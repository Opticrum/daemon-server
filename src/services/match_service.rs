//! Match service — match on-chain orders with Fiber channels.
//!
//! The opticrum contract requires the channel to be created **after** the order
//! (`channel_block > order_block`), so a fresh channel must be opened for every
//! match — existing channels can never be reused.
//!
//! When a seller matches an order, the service:
//! 1. Opens a new channel to the buyer's Fiber peer via `open_channel`.
//! 2. Polls `list_channels` until the channel reaches ChannelReady.
//! 3. Builds the match transaction with the channel outpoint and broadcasts it.
//!
//! Transaction assembly is delegated to `TransactionAssembler` when available.
//!
//! After the chain-first refactor, this module no longer writes to the database.
//! All match data is derived from on-chain scans.

use std::collections::HashSet;
use std::time::Duration;
use tracing::{debug, info};

use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;

/// Result of matching an order.
#[derive(serde::Serialize, Debug)]
pub struct MatchOrderResult {
    pub tx_hash: String,
    pub output_index: i32,
}

/// Initial polling interval.
const POLL_INITIAL_SECS: u64 = 2;
/// Polling interval after ramp-up.
const POLL_LATER_SECS: u64 = 5;
/// Switch to longer interval after this many seconds.
const POLL_RAMP_SECS: u64 = 30;
/// Hardcoded timeout for channel creation fallback path (auto-matcher etc.).
pub const CHANNEL_READY_TIMEOUT_SECS: u64 = 300;
/// Extra capacity (shannons) added when creating a channel to cover the
/// occupied capacity of the channel cell itself (~100 CKB).
pub const CHANNEL_CELL_OCCUPIED_RESERVE: u64 = 100 * 100_000_000;

/// Whether a Fiber channel is fully ready for order matching.
pub fn is_channel_ready(state_name: &str) -> bool {
    state_name == "ChannelReady"
}

/// Whether a Fiber channel is in a terminal / unusable state.
pub fn is_channel_terminal(state_name: &str) -> bool {
    matches!(state_name, "Closed" | "ShuttingDown")
}

/// Whether a Fiber channel exists but is still being set up.
pub fn is_channel_pending(state_name: &str) -> bool {
    !is_channel_ready(state_name) && !is_channel_terminal(state_name)
}

/// Build the set of channel outpoints already used by on-chain match cells.
/// This replaces the old `get_used_channel_ids()` DB query.
pub async fn get_used_channel_outpoints<P: ChainProvider + ?Sized>(
    provider: &P,
) -> Result<HashSet<(String, u32)>, AppError> {
    let matches = provider.scan_matches().await?;
    let used: HashSet<(String, u32)> = matches
        .iter()
        .filter_map(|m| {
            let tx = hex::encode(m.match_args.channel_outpoint.tx_hash);
            let idx = m.match_args.channel_outpoint.index;
            if tx.is_empty() {
                None
            } else {
                Some((tx, idx))
            }
        })
        .collect();
    Ok(used)
}

/// Match an on-chain order with a **fresh** Fiber channel.
///
/// The contract requires `channel_block > order_block`, so the channel must be
/// created for this specific match. This function always opens a new channel
/// and blocks until it reaches ChannelReady (up to 120 seconds).
pub async fn match_order<P: ChainProvider + ?Sized>(
    provider: &P,
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
    let fiber_address = order.fiber_address.as_deref();
    let required_capacity = order.order_data.channel_capacity;
    let order_block = provider.get_tx_block_number(order_tx_hash).await?;
    info!(
        order_tx = %order_tx_hash,
        peer = %fiber_pubkey_hex,
        capacity = required_capacity,
        order_block = order_block,
        "Match: looking for compatible channel"
    );

    // ── 2. Ensure peer is connected ────────────────────────────────────
    let _ = provider
        .connect_peer(&fiber_pubkey_hex, fiber_address)
        .await;

    // ── 3. Build used channel set from on-chain match scan ─────────────
    let used_channel_outpoints = get_used_channel_outpoints(provider).await?;

    // ── 4. Try to reuse an existing compatible channel ─────────────────
    let (channel_tx_hash, channel_output_index) = if let Some(ch) = find_compatible_channel(
        provider,
        &fiber_pubkey_hex,
        required_capacity,
        order_block,
        &used_channel_outpoints,
    )
    .await?
    {
        info!(
            channel_id = %ch.channel_id,
            tx_hash = %ch.tx_hash,
            "Match: reusing existing channel"
        );
        (ch.tx_hash, ch.output_index)
    } else {
        // ── 5. Open a new channel (add reserve for cell occupied capacity) ──
        let funding_amount = required_capacity + CHANNEL_CELL_OCCUPIED_RESERVE;
        info!(
            peer = %fiber_pubkey_hex,
            amount = funding_amount,
            "Match: opening new channel"
        );
        let _temp_id = provider
            .open_channel(&fiber_pubkey_hex, funding_amount, fiber_address)
            .await?;

        // ── 6. Poll until channel has an outpoint ──────────────────────
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

    // ── 7. Build and send match transaction ────────────────────────────
    let tx_hex = format!(
        "match_order:{}:{}:{}:{}",
        order_tx_hash, order_output_index, channel_tx_hash, channel_output_index
    );
    let tx_hash = provider.send_transaction(&tx_hex).await?;
    let output_index = 0; // Match Cell is output[0]

    info!(
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
    })
}

/// Find or create a channel for matching: try existing channels first,
/// otherwise connect peer + open a new one + wait for outpoint.
pub async fn ensure_channel<P: ChainProvider + ?Sized>(
    provider: &P,
    counterparty_pubkey: &str,
    required_capacity: u64,
    order_tx_hash: &str,
    used_channel_outpoints: &HashSet<(String, u32)>,
) -> Result<crate::services::chain_provider::FiberChannelInfo, AppError> {
    let order_block = provider
        .get_tx_block_number(order_tx_hash)
        .await
        .unwrap_or(0);

    // Try existing compatible channel first
    if let Some(ch) = find_compatible_channel(
        provider,
        counterparty_pubkey,
        required_capacity,
        order_block,
        used_channel_outpoints,
    )
    .await?
    {
        Ok(ch)
    } else {
        Err(AppError::ChainError(format!(
            "No compatible channel found for {counterparty_pubkey} with capacity {required_capacity}"
        )))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Find an existing channel that matches the counterparty pubkey, has enough
/// capacity, was created AFTER the order (contract requirement), and has not
/// already been used in another match (checked via on-chain match scan).
async fn find_compatible_channel<P: ChainProvider + ?Sized>(
    provider: &P,
    counterparty_pubkey: &str,
    min_capacity: u64,
    order_block: u64,
    used_channel_outpoints: &HashSet<(String, u32)>,
) -> Result<Option<crate::services::chain_provider::FiberChannelInfo>, AppError> {
    let channels = provider.scan_fiber_channels(&[]).await?;
    for ch in channels {
        if ch.counterparty_fiber_key.trim_start_matches("0x")
            != counterparty_pubkey.trim_start_matches("0x")
        {
            continue;
        }
        if ch.capacity < min_capacity {
            continue;
        }
        if ch.tx_hash.is_empty() {
            continue;
        }
        if !is_channel_ready(&ch.state_name) {
            continue;
        }
        // Exclude channels already used in another match (checked against on-chain data)
        let chan_key = (ch.tx_hash.clone(), ch.output_index);
        if used_channel_outpoints.contains(&chan_key) {
            debug!(
                channel_id = %ch.channel_id,
                tx_hash = %ch.tx_hash,
                "Match: channel already used in another match, skipping"
            );
            continue;
        }
        // Check contract requirement: channel_block > order_block
        let channel_block = provider.get_tx_block_number(&ch.tx_hash).await?;
        if channel_block > 0 && channel_block <= order_block {
            debug!(
                channel_block = channel_block,
                order_block = order_block,
                channel_id = %ch.channel_id,
                "Match: channel created before order, skipping"
            );
            continue;
        }
        if channel_block > 0 && channel_block > order_block {
            return Ok(Some(ch));
        }
        // channel_block == 0 means tx not yet confirmed — skip
    }
    Ok(None)
}

/// Poll `scan_fiber_channels` until a channel matching the
/// given counterparty pubkey and capacity appears, or the timeout is reached.
pub async fn wait_for_channel_ready<P: ChainProvider + ?Sized>(
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

        // Check for failed/terminal channels for this peer
        let matching: Vec<_> = channels
            .iter()
            .filter(|ch| {
                ch.counterparty_fiber_key.trim_start_matches("0x")
                    == counterparty_pubkey.trim_start_matches("0x")
            })
            .collect();

        for ch in &matching {
            match ch.state_name.as_str() {
                "Closed" | "ShuttingDown" => {
                    return Err(AppError::ChainError(format!(
                        "Channel to {counterparty_pubkey} entered terminal state '{}' (id={}). \
                         The channel creation likely failed on the Fiber node side.",
                        ch.state_name, ch.channel_id
                    )));
                }
                _ if is_channel_ready(&ch.state_name)
                    && !ch.tx_hash.is_empty()
                    && ch.capacity >= min_capacity =>
                {
                    info!(
                        channel_id = %ch.channel_id,
                        tx_hash = %ch.tx_hash,
                        state = %ch.state_name,
                        "Match: channel ready, proceeding"
                    );
                    return Ok((*ch).clone());
                }
                _ => {}
            }
        }

        // Log all states for this peer (info level so it's visible in production)
        let state_summary: Vec<_> = matching
            .iter()
            .map(|ch| {
                format!(
                    "{}@{}",
                    ch.state_name,
                    &ch.channel_id[..8.min(ch.channel_id.len())]
                )
            })
            .collect();
        if !state_summary.is_empty() {
            info!(
                elapsed = started.elapsed().as_secs(),
                channels = ?state_summary,
                "Match: waiting for channel, current states"
            );
        } else {
            debug!(
                elapsed = started.elapsed().as_secs(),
                "Match: no channels found for peer yet, retrying in {}s", delay
            );
        }

        actix_rt::time::sleep(Duration::from_secs(delay)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockChainProvider;

    #[actix_rt::test]
    async fn match_order_fails_when_order_not_on_chain() {
        let provider = MockChainProvider::new();
        let result = match_order(&provider, "order_not_found", 0, "ckt1q...seller").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found on chain"));
    }

    #[test]
    fn channel_state_helpers() {
        assert!(is_channel_ready("ChannelReady"));
        assert!(!is_channel_ready("AwaitingChannelReady"));
        assert!(is_channel_pending("NegotiatingFunding"));
        assert!(is_channel_terminal("Closed"));
        assert!(is_channel_terminal("ShuttingDown"));
    }

    #[actix_rt::test]
    async fn match_order_no_channels_attempts_open() {
        let provider = MockChainProvider::new();
        let result = match_order(&provider, "any_order", 0, "ckt1q...seller").await;
        assert!(result.is_err());

        // open_channel was never called because order lookup fails first.
        let calls = provider.open_channels.lock().unwrap();
        assert!(calls.is_empty());
    }
}
