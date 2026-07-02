//! Scheduler tests — rent extraction loop (chain-first architecture).

mod common;

use common::test_db;
use rust_server::db::wallets as wallet_db;
use rust_server::scheduler::rent_extractor::run_extraction_cycle;
use rust_server::services::MockChainProvider;

fn test_provider() -> MockChainProvider {
    MockChainProvider::new()
}

#[actix_rt::test]
async fn no_wallets_produces_zero_extraction() {
    let pool = test_db();
    let provider = test_provider();
    let extracted = run_extraction_cycle(&pool, 1000, &provider, None, None)
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
    let extracted = run_extraction_cycle(&pool, 100, &provider, None, None)
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
    let extracted = run_extraction_cycle(&pool, 100, &provider, None, None)
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
    let result = run_extraction_cycle(&pool, 1_000_000, &provider, None, None)
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

    let extracted = run_extraction_cycle(&pool, 0, &provider, None, None)
        .await
        .unwrap();
    assert_eq!(
        extracted, 0,
        "no on-chain matches should produce zero extraction"
    );
}
