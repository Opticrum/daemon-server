//! Scheduler tests — rent extraction loop (chain-first architecture).

mod common;

use std::sync::{Arc, RwLock};

use common::{mock_with_hesitation_match, test_db};
use rust_server::db::wallets as wallet_db;
use rust_server::scheduler::rent_extractor::run_extraction_cycle;
use rust_server::services::console::scheduler_state::{SchedulerState, SharedSchedulerState};
use rust_server::services::MockChainProvider;

fn test_provider() -> MockChainProvider {
    MockChainProvider::new()
}

#[actix_rt::test]
async fn no_wallets_produces_zero_extraction() {
    let pool = test_db();
    let provider = test_provider();
    let extracted = run_extraction_cycle(&pool, 1000, &provider, None, None, None)
        .await
        .unwrap();
    assert_eq!(extracted, 0);
}

#[actix_rt::test]
async fn with_wallets_but_no_matches_on_chain() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let provider = test_provider();

    // Insert a wallet — but no matches on chain
    wallet_db::insert_wallet(
        &mut conn,
        "w",
        b"enc",
        &[10u8; 32],
        "addr10",
        None,
        None,
        None,
        "imported",
    )
    .unwrap();

    // No matches in mock provider → extraction returns 0
    let extracted = run_extraction_cycle(&pool, 100, &provider, None, None, None)
        .await
        .unwrap();
    assert_eq!(extracted, 0);
}

#[actix_rt::test]
async fn with_wallets_but_seller_lock_hash_mismatch() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let provider = test_provider();

    // Insert wallet with lock_hash [10u8; 32]
    wallet_db::insert_wallet(
        &mut conn,
        "w",
        b"enc",
        &[10u8; 32],
        "addr10",
        None,
        None,
        None,
        "imported",
    )
    .unwrap();

    // No matches in mock provider — even if there were, they'd need matching seller_lock_hash
    let extracted = run_extraction_cycle(&pool, 100, &provider, None, None, None)
        .await
        .unwrap();
    assert_eq!(extracted, 0);
}

#[actix_rt::test]
async fn respects_min_extraction_amount() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let provider = test_provider();

    wallet_db::insert_wallet(
        &mut conn,
        "w3",
        b"enc3",
        &[12u8; 32],
        "addr12",
        None,
        None,
        None,
        "imported",
    )
    .unwrap();

    // No on-chain matches → extraction should return 0 regardless of threshold
    let result = run_extraction_cycle(&pool, 1_000_000, &provider, None, None, None)
        .await
        .unwrap();
    assert_eq!(result, 0, "no matches on chain should return 0");
}

#[actix_rt::test]
async fn no_matches_on_chain_but_wallets_exist() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let provider = test_provider();

    wallet_db::insert_wallet(
        &mut conn,
        "multi",
        b"enc",
        &[14u8; 32],
        "addr14",
        None,
        None,
        None,
        "imported",
    )
    .unwrap();

    let extracted = run_extraction_cycle(&pool, 0, &provider, None, None, None)
        .await
        .unwrap();
    assert_eq!(
        extracted, 0,
        "no on-chain matches should produce zero extraction"
    );
}

#[actix_rt::test]
async fn skips_matches_in_hesitation() {
    let pool = test_db();
    {
        let mut conn = pool.get().unwrap();
        // Managed wallet whose lock hash matches the hesitation match's seller
        // (`[0xcdu8; 32]`), so the cycle treats the match as managed.
        wallet_db::insert_wallet(
            &mut conn,
            "hesitation-wallet",
            b"enc",
            &[0xcdu8; 32],
            "addr_cd",
            None,
            None,
            None,
            "imported",
        )
        .unwrap();
    }

    // Match created at block 100, never extracted; tip 2000 → elapsed 1900,
    // still inside the 3600-block hesitation window.
    let provider = mock_with_hesitation_match(100, 0, 2000);
    let shared: SharedSchedulerState = Arc::new(RwLock::new(SchedulerState::new()));

    let extracted = run_extraction_cycle(&pool, 0, &provider, None, None, Some(&shared))
        .await
        .unwrap();
    assert_eq!(
        extracted, 0,
        "nothing should be extracted while the match is in hesitation"
    );
    assert!(
        provider.submitted_txs.lock().unwrap().is_empty(),
        "no extraction tx should be broadcast during hesitation"
    );

    let events = shared.read().unwrap().events_since(0);
    assert!(
        events
            .iter()
            .any(|e| e.message.to_lowercase().contains("hesitation")),
        "expected a 'hesitation' scheduler event, got: {:?}",
        events.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
    assert!(
        !events.iter().any(|e| e.message.contains("failed for cell")),
        "hesitation skips must not be logged as per-cell failures, got: {:?}",
        events.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}

#[actix_rt::test]
async fn extracts_after_hesitation_elapsed() {
    let pool = test_db();
    {
        let mut conn = pool.get().unwrap();
        wallet_db::insert_wallet(
            &mut conn,
            "hesitation-wallet-2",
            b"enc",
            &[0xcdu8; 32],
            "addr_cd2",
            None,
            None,
            None,
            "imported",
        )
        .unwrap();
    }

    // tip 5000 → elapsed 4900 > 3600 → window elapsed, extraction proceeds.
    let provider = mock_with_hesitation_match(100, 0, 5000);

    let extracted = run_extraction_cycle(&pool, 0, &provider, None, None, None)
        .await
        .unwrap();
    assert!(
        extracted > 0,
        "extraction after the hesitation window should succeed"
    );
}
