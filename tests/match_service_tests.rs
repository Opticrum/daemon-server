//! Match service integration tests (seller-side only).

mod common;

use common::test_db;
use rust_server::services::{match_service, MockChainProvider};

#[actix_rt::test]
async fn full_match_flow() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    // Add a channel cell so get_cell succeeds
    provider.add_cell(
        "channel_abc",
        0,
        rust_server::services::chain_provider::CellOutput {
            capacity: 200_000_000_000,
            lock_hash: [0u8; 32],
            type_hash: None,
            data: vec![],
        },
    );

    // Match an on-chain order (identified by tx_hash + output_index)
    let m = match_service::match_order(
        &provider,
        &pool,
        "order_tx_onchain",
        0,
        "seller",
        "channel_abc",
        0,
    )
    .await
    .expect("match should succeed");

    assert!(m.match_id > 0);
}

#[actix_rt::test]
async fn match_nonexistent_channel_fails() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    let result = match_service::match_order(
        &provider,
        &pool,
        "some_order_tx",
        0,
        "seller",
        "nonexistent_channel",
        0,
    )
    .await;
    assert!(result.is_err());
}
