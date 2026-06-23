//! Rent service integration tests.
//!
//! The rent service has inline #[cfg(test)] tests in src/services/rent_service.rs.

mod common;

use common::test_db;
use rust_server::db::matches as match_db;
use rust_server::services::{rent_service, MockChainProvider};

#[actix_rt::test]
async fn extract_and_destroy_flow() {
    let pool = test_db();
    let provider = MockChainProvider::new();
    provider.set_tip_block(2000);

    let conn = pool.get().unwrap();
    let match_id = match_db::insert_match(
        &conn,
        "m_extract_flow",
        0,
        "o_extract_flow",
        0,
        "seller_flow",
        100.0, // 100 shannons/block
        300_000,
        None::<&str>,
    )
    .unwrap();
    match_db::update_match_extraction(&conn, match_id, 1000).unwrap();

    // Extract rent
    let result = rent_service::extract_rent(&provider, &pool, match_id)
        .await
        .expect("extract should succeed");
    assert!(result.extracted_amount > 0);
    assert!(!result.is_exhausted);

    // Destroy
    provider.set_tip_block(5000);
    let tx_hash = rent_service::destroy_match(&provider, &pool, match_id)
        .await
        .expect("destroy should succeed");
    assert!(!tx_hash.is_empty());
}

#[actix_rt::test]
async fn extract_nonexistent_match_fails() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    let result = rent_service::extract_rent(&provider, &pool, 9999).await;
    assert!(result.is_err());
}

#[actix_rt::test]
async fn destroy_nonexistent_match_fails() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    let result = rent_service::destroy_match(&provider, &pool, 9999).await;
    assert!(result.is_err());
}
