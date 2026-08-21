//! Channel dismissal tests — `delete_channel` persistence + list filtering.
//!
//! Closed channels stay visible in the Fiber node's `list_channels` response
//! until funds settle, so the console "delete" action dismisses them via the
//! `dismissed_fiber_channels` tombstone. These tests verify that dismissing a
//! channel hides it from every channel list and that non-closed channels are
//! rejected.

mod common;

use common::test_db;
use rust_server::error::AppError;
use rust_server::services::chain_provider::{FiberChannelInfo, MockChainProvider};
use rust_server::services::console::gateway_service::GatewayService;

fn channel(id: &str, state_name: &str) -> FiberChannelInfo {
    FiberChannelInfo {
        channel_id: id.to_string(),
        // 33-byte compressed secp256k1 pubkey (66 hex chars).
        counterparty_fiber_key: format!("03{}", "ab".repeat(32)),
        tx_hash: format!("0x{}", "cd".repeat(32)),
        output_index: 0,
        capacity: 1_000_000_000_000,
        local_balance: 600_000_000_000,
        remote_balance: 400_000_000_000,
        state_name: state_name.to_string(),
        close_flags: None,
        is_public: false,
        enabled: true,
        created_at: 1_700_000_000_000,
    }
}

#[actix_rt::test]
async fn delete_closed_channel_hides_it_from_all_lists() {
    let pool = test_db();
    let closed_id = "closed-channel-01";
    let ready_id = "ready-channel-01";
    let provider = MockChainProvider::with_fiber_channels(vec![
        channel(closed_id, "Closed"),
        channel(ready_id, "ChannelReady"),
    ]);

    // Before dismissal, both channels are listed.
    let all = GatewayService::get_channels_only(&pool, &provider)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    // Dismiss the closed channel.
    GatewayService::delete_channel(&pool, &provider, closed_id)
        .await
        .unwrap();

    // The fast path (`channels-only`) no longer includes it.
    let remaining = GatewayService::get_channels_only(&pool, &provider)
        .await
        .unwrap();
    let ids: Vec<&str> = remaining.iter().map(|c| c.channel_id.as_str()).collect();
    assert_eq!(ids, vec![ready_id]);

    // The full path (`channels`, with match cross-reference) agrees.
    let cwms = GatewayService::get_channels_with_matches(&pool, &provider)
        .await
        .unwrap();
    assert_eq!(cwms.len(), 1);
    assert_eq!(cwms[0].channel.channel_id, ready_id);

    // The progressive-match path (`channel-matches`) agrees too.
    let cwm_progressive = GatewayService::get_channel_matches(&pool, &provider)
        .await
        .unwrap();
    assert_eq!(cwm_progressive.len(), 1);
    assert_eq!(cwm_progressive[0].channel.channel_id, ready_id);
}

#[actix_rt::test]
async fn delete_is_idempotent_for_same_channel() {
    let pool = test_db();
    let closed_id = "closed-channel-02";
    let provider = MockChainProvider::with_fiber_channels(vec![channel(closed_id, "Closed")]);

    GatewayService::delete_channel(&pool, &provider, closed_id)
        .await
        .unwrap();
    GatewayService::delete_channel(&pool, &provider, closed_id)
        .await
        .unwrap();
}

#[actix_rt::test]
async fn delete_rejects_non_closed_channel() {
    let pool = test_db();
    let provider =
        MockChainProvider::with_fiber_channels(vec![channel("ready-01", "ChannelReady")]);

    let err = GatewayService::delete_channel(&pool, &provider, "ready-01")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::BadRequest(_)));
}

#[actix_rt::test]
async fn delete_unknown_channel_returns_not_found() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    let err = GatewayService::delete_channel(&pool, &provider, "no-such-channel")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)));
}
