//! Rent service integration tests (chain-first architecture).

mod common;

use common::test_db;
use rust_server::services::{rent_service, MockChainProvider};

#[actix_rt::test]
async fn extract_nonexistent_match_fails() {
    let pool = test_db();
    let provider = MockChainProvider::new();

    let result = rent_service::extract_rent(
        &provider,
        &pool,
        "nonexistent_tx",
        9999,
        &rent_service::ExtractRentOptions::mock(),
    )
    .await;
    assert!(result.is_err());
}

#[actix_rt::test]
async fn destroy_nonexistent_match_fails() {
    let provider = MockChainProvider::new();

    let result = rent_service::destroy_match(&provider, "nonexistent_tx", 9999).await;
    assert!(result.is_err());
}
