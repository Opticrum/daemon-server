//! Database layer tests — wallets, extraction_history (statistics cache),
//! and dismissed_fiber_channels (console channel tombstone).
//!
//! After the chain-first refactor, `tracked_matches` was removed; matches are
//! derived from on-chain data. `dismissed_fiber_channels` was later restored
//! because dismissing a closed channel is a console display preference that
//! is not chain-reconstructable.

mod common;

use common::test_db;
use rust_server::db::{
    destroyed_matches as destroyed_db, dismissed_channels as dismissed_db, matches as match_db,
    schema, wallet_txs as wallet_txs_db, wallets as wallet_db,
};

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

// ── destroyed_matches tests ──

#[actix_rt::test]
async fn insert_and_list_destroyed_matches() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    destroyed_db::insert_destroyed_match(
        &mut conn, "tx_a", 0, "order_a", 0, "lock_a", 100, 5000, 200, None, 1000, 50,
    )
    .unwrap();
    destroyed_db::insert_destroyed_match(
        &mut conn, "tx_b", 1, "order_b", 0, "lock_b", 200, 8000, 300, None, 2000, 60,
    )
    .unwrap();

    let list = destroyed_db::list_destroyed_matches(&mut conn).unwrap();
    assert_eq!(list.len(), 2);
    // Both rows present (ordering is by destroyed_at — same-second inserts
    // are non-deterministic, so we don't assert on order).
    let has_a = list.iter().any(|r| r.tx_hash == "tx_a");
    let has_b = list.iter().any(|r| r.tx_hash == "tx_b");
    assert!(has_a);
    assert!(has_b);
    // Verify fields on one entry
    let b = list.iter().find(|r| r.tx_hash == "tx_b").unwrap();
    assert_eq!(b.shannons_per_block, 200);
    assert_eq!(b.ckb_capacity, 8000);
    assert_eq!(b.extracted_total, 2000);
}

#[actix_rt::test]
async fn get_destroyed_match_found() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    destroyed_db::insert_destroyed_match(
        &mut conn, "tx_find", 3, "order_f", 0, "lock_f", 100, 5000, 100, None, 1500, 80,
    )
    .unwrap();

    let row = destroyed_db::get_destroyed_match(&mut conn, "tx_find", 3)
        .unwrap()
        .unwrap();
    assert_eq!(row.tx_hash, "tx_find");
    assert_eq!(row.output_index, 3);
    assert_eq!(row.seller_lock_hash, "lock_f");
}

#[actix_rt::test]
async fn get_destroyed_match_not_found() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    let result = destroyed_db::get_destroyed_match(&mut conn, "nonexistent", 0).unwrap();
    assert!(result.is_none());
}

#[actix_rt::test]
async fn count_destroyed_matches() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    assert_eq!(destroyed_db::count_destroyed_matches(&mut conn).unwrap(), 0);
    destroyed_db::insert_destroyed_match(
        &mut conn, "tx_c", 0, "order_c", 0, "lock_c", 100, 5000, 0, None, 0, 0,
    )
    .unwrap();
    assert_eq!(destroyed_db::count_destroyed_matches(&mut conn).unwrap(), 1);
}

#[actix_rt::test]
async fn destroyed_match_unique_constraint() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    destroyed_db::insert_destroyed_match(
        &mut conn, "tx_dup", 0, "order_d", 0, "lock_d", 100, 5000, 0, None, 0, 0,
    )
    .unwrap();

    let result = destroyed_db::insert_destroyed_match(
        &mut conn, "tx_dup", 0, "order_e", 0, "lock_e", 200, 6000, 0, None, 0, 0,
    );
    assert!(result.is_err());
}

// ── Dismissed fiber channels (console tombstone) ──────────────────────────

#[test]
fn dismissed_channel_roundtrip() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    assert!(!dismissed_db::is_dismissed(&mut conn, "ch_a").unwrap());
    assert!(dismissed_db::list_dismissed_ids(&mut conn)
        .unwrap()
        .is_empty());

    dismissed_db::dismiss_channel(&mut conn, "ch_a").unwrap();
    assert!(dismissed_db::is_dismissed(&mut conn, "ch_a").unwrap());
    assert_eq!(
        dismissed_db::list_dismissed_ids(&mut conn).unwrap(),
        vec!["ch_a"]
    );

    // Re-dismissing is idempotent (single row, no error).
    dismissed_db::dismiss_channel(&mut conn, "ch_a").unwrap();
    assert_eq!(
        dismissed_db::list_dismissed_ids(&mut conn).unwrap().len(),
        1
    );

    dismissed_db::dismiss_channel(&mut conn, "ch_b").unwrap();
    let ids = dismissed_db::list_dismissed_ids(&mut conn).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"ch_a".to_string()));
    assert!(ids.contains(&"ch_b".to_string()));
}

#[test]
fn wallet_tx_upsert_batch_idempotent() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    let row = || wallet_txs_db::NewWalletTx {
        tx_hash: "aabb",
        wallet_id: 1,
        block_number: 100,
        timestamp_ms: Some(1_700_000_000_000),
        received_shannons: 5_000_000_000,
        sent_shannons: 0,
    };

    wallet_txs_db::upsert_batch(&mut conn, &[row()]).unwrap();
    wallet_txs_db::upsert_batch(&mut conn, &[row()]).unwrap();
    assert_eq!(wallet_txs_db::list_all(&mut conn).unwrap().len(), 1);

    // Update the existing row (same key).
    let updated = wallet_txs_db::NewWalletTx {
        received_shannons: 7_000_000_000,
        block_number: 101,
        ..row()
    };
    wallet_txs_db::upsert_batch(&mut conn, &[updated]).unwrap();
    let rows = wallet_txs_db::list_all(&mut conn).unwrap();
    assert_eq!(rows.len(), 1, "upsert updates, not duplicates");
    assert_eq!(rows[0].received_shannons, 7_000_000_000);
    assert_eq!(rows[0].block_number, 101);
}

#[test]
fn wallet_tx_prune_other_wallets() {
    let pool = test_db();
    let mut conn = pool.get().unwrap();

    wallet_txs_db::upsert_batch(
        &mut conn,
        &[
            wallet_txs_db::NewWalletTx {
                tx_hash: "t1",
                wallet_id: 1,
                block_number: 10,
                timestamp_ms: None,
                received_shannons: 1,
                sent_shannons: 0,
            },
            wallet_txs_db::NewWalletTx {
                tx_hash: "t2",
                wallet_id: 2,
                block_number: 20,
                timestamp_ms: None,
                received_shannons: 1,
                sent_shannons: 0,
            },
        ],
    )
    .unwrap();

    let pruned = wallet_txs_db::prune_other_wallets(&mut conn, &[1]).unwrap();
    assert_eq!(pruned, 1, "wallet 2 rows pruned");
    let rows = wallet_txs_db::list_all(&mut conn).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].wallet_id, 1);

    // Empty keep set → wipe everything.
    let pruned = wallet_txs_db::prune_other_wallets(&mut conn, &[]).unwrap();
    assert_eq!(pruned, 1);
    assert!(wallet_txs_db::list_all(&mut conn).unwrap().is_empty());
}
