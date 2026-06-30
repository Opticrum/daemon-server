//! Scheduler tests — rent extraction loop logic.

mod common;

use common::test_db;
use rust_server::db::{matches as match_db, wallets as wallet_db};
use rust_server::scheduler::rent_extractor::run_extraction_cycle;
use rust_server::services::MockChainProvider;

/// Helper to create a fresh mock provider for each test.
fn test_provider() -> MockChainProvider {
    MockChainProvider::new()
}

#[actix_rt::test]
async fn no_wallets_produces_zero_extraction() {
    let pool = test_db();
    let provider = test_provider();
    let extracted = run_extraction_cycle(&pool, 1000, &provider).await.unwrap();
    assert_eq!(extracted, 0);
}

#[actix_rt::test]
async fn extracts_when_above_threshold() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let provider = test_provider();

    // Add a wallet
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

    // Add a match with high shannons_per_block (1000 shannons/block)
    match_db::insert_match(
        &mut conn,
        "match_high",
        0,
        "order_high",
        0,
        "seller_high",
        1000,
        None::<&str>,
    )
    .unwrap();

    // Threshold 100_000, extractable = 1000 * 1000 = 1_000_000 > 100_000
    let extracted = run_extraction_cycle(&pool, 100_000, &provider)
        .await
        .unwrap();
    assert!(extracted > 0);
}

#[actix_rt::test]
async fn skips_when_below_threshold() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let provider = test_provider();

    wallet_db::insert_wallet(
        &mut conn,
        "w2",
        b"enc2",
        &[11u8; 32],
        "addr11",
        None,
        None,
        None,
        "imported",
    )
    .unwrap();

    // Low shannons_per_block: 1 shannon/block
    match_db::insert_match(
        &mut conn,
        "match_low",
        0,
        "order_low",
        0,
        "seller_low",
        1, // 1 shannon/block
        None::<&str>,
    )
    .unwrap();

    // Extractable = 1 * 1000 = 1000 < 1_000_000_000 threshold
    let extracted = run_extraction_cycle(&pool, 1_000_000_000, &provider)
        .await
        .unwrap();
    assert_eq!(extracted, 0);
}

#[actix_rt::test]
async fn respects_min_extraction_different_levels() {
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

    match_db::insert_match(
        &mut conn,
        "match_mid",
        0,
        "order_mid",
        0,
        "seller_mid",
        50, // 50 shannons/block
        None::<&str>,
    )
    .unwrap();

    // Extractable = 50 * 1000 = 50_000
    // High threshold: skipped
    let result = run_extraction_cycle(&pool, 1_000_000, &provider)
        .await
        .unwrap();
    assert_eq!(result, 0, "should skip with high threshold");

    // Low threshold: extracted
    let result = run_extraction_cycle(&pool, 100, &provider).await.unwrap();
    assert!(result > 0, "should extract with low threshold");
}

#[actix_rt::test]
async fn only_processes_live_matches() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let provider = test_provider();

    wallet_db::insert_wallet(
        &mut conn,
        "w4",
        b"enc4",
        &[13u8; 32],
        "addr13",
        None,
        None,
        None,
        "imported",
    )
    .unwrap();

    // Insert a destroyed match
    match_db::insert_match(
        &mut conn,
        "dead_match",
        0,
        "dead_order",
        0,
        "dead_seller",
        1000,
        None::<&str>,
    )
    .unwrap();
    match_db::update_match_status(&mut conn, 1, "destroyed").unwrap();

    // Should skip destroyed matches
    let extracted = run_extraction_cycle(&pool, 0, &provider).await.unwrap();
    assert_eq!(extracted, 0, "should skip destroyed matches");
}

#[actix_rt::test]
async fn multiple_matches_all_processed() {
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

    for i in 0..5 {
        match_db::insert_match(
            &mut conn,
            &format!("multi_match_{i}"),
            0,
            &format!("multi_order_{i}"),
            0,
            &format!("seller_{i}"),
            200,
            None::<&str>,
        )
        .unwrap();
    }

    let extracted = run_extraction_cycle(&pool, 0, &provider).await.unwrap();
    assert!(extracted > 0, "should process all live matches");
}
