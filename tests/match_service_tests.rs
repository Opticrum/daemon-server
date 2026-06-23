//! Match service integration tests.
//!
//! The match service has inline #[cfg(test)] tests in src/services/match_service.rs.

mod common;

use common::test_db;
use rust_server::services::{match_service, order_service, MockChainProvider};

#[actix_rt::test]
async fn full_match_flow() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    // Add a channel cell
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

    // Create order
    let order =
        order_service::create_order(&provider, &pool, "buyer", 100_000_000_000, 300_000, None)
            .await
            .unwrap();

    // Match it
    let m =
        match_service::match_order(&provider, &pool, order.order_id, "seller", "channel_abc", 0)
            .await
            .expect("match should succeed");

    assert!(m.match_id > 0);
}

#[actix_rt::test]
async fn match_nonexistent_order_fails() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    let result = match_service::match_order(&provider, &pool, 9999, "seller", "channel", 0).await;
    assert!(result.is_err());
}
