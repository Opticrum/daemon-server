//! Order service integration tests.
//!
//! The order service has inline #[cfg(test)] tests in src/services/order_service.rs
//! that cover the core logic. This file tests the public API from an external
//! integration-test perspective, using the shared test utilities.

mod common;

use common::test_db;
use rust_server::services::order_service;

#[actix_rt::test]
async fn create_and_list_orders_flow() {
    let pool = test_db();

    // Create two orders
    let r1 = order_service::create_order(
        &rust_server::services::MockChainProvider::new(),
        &pool,
        "ckt1q...a",
        100_000_000_000,
        300_000,
        None,
    )
    .await
    .expect("create order 1");

    let r2 = order_service::create_order(
        &rust_server::services::MockChainProvider::new(),
        &pool,
        "ckt1q...b",
        200_000_000_000,
        400_000,
        None,
    )
    .await
    .expect("create order 2");

    assert_ne!(r1.order_id, r2.order_id);

    // List all
    let all = order_service::list_orders(&pool, None).unwrap();
    assert_eq!(all.len(), 2);
}

#[actix_rt::test]
async fn create_and_cancel_flow() {
    let pool = test_db();
    let provider = rust_server::services::MockChainProvider::new();

    let created =
        order_service::create_order(&provider, &pool, "buyer", 50_000_000_000, 100_000, None)
            .await
            .unwrap();

    let tx_hash = order_service::cancel_order(&provider, &pool, created.order_id)
        .await
        .unwrap();
    assert!(!tx_hash.is_empty());

    // Verify cancelled
    let orders = order_service::list_orders(&pool, Some("cancelled")).unwrap();
    assert_eq!(orders.len(), 1);
}
