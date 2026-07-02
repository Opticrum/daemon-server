//! Database layer tests — wallets and extraction_history (statistics cache).
//!
//! After the chain-first refactor, tracked_matches and dismissed_fiber_channels
//! have been removed. This file tests wallets (local key store) and
//! extraction_history (statistics cache).

mod common;

use common::test_db;
use rust_server::db::{matches as match_db, schema, wallets as wallet_db};

#[test]
fn migration_idempotent() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    schema::run_migrations(&mut conn).expect("migrations should be idempotent");
}

// ── Wallet tests (unchanged) ─────────────────────────────────────────────

#[test]
fn wallet_create_and_get() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    let id = wallet_db::insert_wallet(
        &mut conn,
        "my-wallet",
        b"encrypted_key_data",
        &[0xabu8; 32],
        "ckt1q...abc",
        None,
        None,
        None,
        "imported",
    )
    .expect("insert should succeed");
    assert!(id > 0);

    let wallet = wallet_db::get_wallet_by_id(&mut conn, id).expect("should find wallet");
    assert_eq!(wallet.label, "my-wallet");
    assert_eq!(wallet.encrypted_key, b"encrypted_key_data");
    assert_eq!(wallet.lock_hash, vec![0xabu8; 32]);
    assert_eq!(wallet.ckb_address, "ckt1q...abc");
}

#[test]
fn wallet_list() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    wallet_db::insert_wallet(
        &mut conn, "w1", b"k1", &[1u8; 32], "addr1", None, None, None, "imported",
    )
    .unwrap();
    wallet_db::insert_wallet(
        &mut conn, "w2", b"k2", &[2u8; 32], "addr2", None, None, None, "imported",
    )
    .unwrap();
    wallet_db::insert_wallet(
        &mut conn, "w3", b"k3", &[3u8; 32], "addr3", None, None, None, "imported",
    )
    .unwrap();

    let wallets = wallet_db::list_wallets(&mut conn).unwrap();
    assert_eq!(wallets.len(), 3);
}

#[test]
fn wallet_delete() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    let id = wallet_db::insert_wallet(
        &mut conn,
        "to-delete",
        b"key",
        &[9u8; 32],
        "addr",
        None,
        None,
        None,
        "imported",
    )
    .unwrap();
    let deleted = wallet_db::delete_wallet(&mut conn, id).unwrap();
    assert!(deleted);

    let result = wallet_db::get_wallet_by_id(&mut conn, id);
    assert!(result.is_err());
}

#[test]
fn wallet_delete_nonexistent() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();
    let deleted = wallet_db::delete_wallet(&mut conn, 9999).unwrap();
    assert!(!deleted);
}

#[test]
fn wallet_unique_lock_hash() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    wallet_db::insert_wallet(
        &mut conn, "w1", b"k1", &[5u8; 32], "addr", None, None, None, "imported",
    )
    .unwrap();
    let result = wallet_db::insert_wallet(
        &mut conn, "w2", b"k2", &[5u8; 32], "addr2", None, None, None, "imported",
    );
    assert!(result.is_err());
}

#[test]
fn wallet_get_by_lock_hash() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    wallet_db::insert_wallet(
        &mut conn, "by-hash", b"key", &[7u8; 32], "addr7", None, None, None, "imported",
    )
    .unwrap();
    let wallet = wallet_db::get_wallet_by_lock_hash(&mut conn, &[7u8; 32]).unwrap();
    assert_eq!(wallet.label, "by-hash");
}

// ── Extraction history tests (statistics cache) ──────────────────────────

#[test]
fn extraction_history_insert() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    let id = match_db::insert_extraction(&mut conn, "match_tx", 0, 50_000, 1500, "extract_tx_hash")
        .unwrap();
    assert!(id > 0);

    let history = match_db::get_extractions_for_match(&mut conn, "match_tx", 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].extracted_amount, 50_000);
}

#[test]
fn total_extracted_aggregates() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    match_db::insert_extraction(&mut conn, "a", 0, 1000, 100, "tx1").unwrap();
    match_db::insert_extraction(&mut conn, "b", 0, 2000, 200, "tx2").unwrap();
    match_db::insert_extraction(&mut conn, "c", 0, 3000, 300, "tx3").unwrap();

    let total = match_db::total_extracted(&mut conn).unwrap();
    assert_eq!(total, 6000);
}

#[test]
fn extracted_for_match_sums_correctly() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    match_db::insert_extraction(&mut conn, "match_xyz", 0, 100, 10, "tx_a").unwrap();
    match_db::insert_extraction(&mut conn, "match_xyz", 0, 200, 20, "tx_b").unwrap();
    match_db::insert_extraction(&mut conn, "other", 1, 500, 30, "tx_c").unwrap();

    let sum = match_db::extracted_for_match(&mut conn, "match_xyz", 0).unwrap();
    assert_eq!(sum, 300);
}
