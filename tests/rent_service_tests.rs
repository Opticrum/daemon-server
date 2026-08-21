//! Rent service integration tests (chain-first architecture).

mod common;

use common::{mock_with_hesitation_match, mock_with_match, test_db};
use rust_server::services::chain_provider::ChainProvider;
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
    let pool = test_db();
    let provider = MockChainProvider::new();

    let result = rent_service::destroy_match(&provider, &pool, "nonexistent_tx", 9999).await;
    assert!(result.is_err());
}

#[actix_rt::test]
async fn extract_rent_caps_at_remaining_capacity() {
    let pool = test_db();
    // Match: rate=100 shannons/block, last_extraction_block=500, capacity=50e9.
    let provider = mock_with_match();
    // Raw accrual 100 × (600_000_500 − 500) = 60e9 exceeds the 50e9 capacity.
    provider.set_tip_block(600_000_500);

    let m = &provider.scan_matches().await.unwrap()[0];
    let tx_hash = hex::encode(m.match_outpoint.tx_hash);
    let capacity = m.ckb_capacity;

    let result = rent_service::extract_rent(
        &provider,
        &pool,
        &tx_hash,
        m.match_outpoint.index,
        &rent_service::ExtractRentOptions::mock(),
    )
    .await
    .unwrap();

    assert_eq!(result.extracted_amount, capacity);
    assert!(result.is_exhausted);
}

#[actix_rt::test]
async fn extract_rent_rejected_during_hesitation() {
    let pool = test_db();
    // Match created at block 100, never extracted; tip 2000 → elapsed 1900,
    // still inside the 3600-block hesitation window.
    let provider = mock_with_hesitation_match(100, 0, 2000);

    let m = &provider.scan_matches().await.unwrap()[0];
    let tx_hash = hex::encode(m.match_outpoint.tx_hash);

    let result = rent_service::extract_rent(
        &provider,
        &pool,
        &tx_hash,
        m.match_outpoint.index,
        &rent_service::ExtractRentOptions::mock(),
    )
    .await;

    let err = result.expect_err("extraction during hesitation should fail");
    assert!(
        err.to_string().to_lowercase().contains("hesitation"),
        "unexpected error message: {err}"
    );
    assert!(
        provider.submitted_txs.lock().unwrap().is_empty(),
        "no transaction should be broadcast during hesitation"
    );
}

#[actix_rt::test]
async fn extract_rent_allowed_after_hesitation() {
    let pool = test_db();
    // tip 5000 → elapsed 4900 > 3600 → window elapsed, extraction allowed.
    let provider = mock_with_hesitation_match(100, 0, 5000);

    let m = &provider.scan_matches().await.unwrap()[0];
    let tx_hash = hex::encode(m.match_outpoint.tx_hash);

    let result = rent_service::extract_rent(
        &provider,
        &pool,
        &tx_hash,
        m.match_outpoint.index,
        &rent_service::ExtractRentOptions::mock(),
    )
    .await;
    assert!(
        result.is_ok(),
        "extraction after the hesitation window should succeed"
    );
}

#[actix_rt::test]
async fn extract_rent_allowed_when_already_extracted() {
    let pool = test_db();
    // mock_with_match → last_extraction_block = 500, never in hesitation.
    let provider = mock_with_match();
    provider.set_tip_block(2000);

    let m = &provider.scan_matches().await.unwrap()[0];
    let tx_hash = hex::encode(m.match_outpoint.tx_hash);

    let result = rent_service::extract_rent(
        &provider,
        &pool,
        &tx_hash,
        m.match_outpoint.index,
        &rent_service::ExtractRentOptions::mock(),
    )
    .await;
    assert!(
        result.is_ok(),
        "extraction after the seller has already extracted should succeed"
    );
}
