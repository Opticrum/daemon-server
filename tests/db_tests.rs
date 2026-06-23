//! Database layer tests — CRUD operations with in-memory SQLite.

mod common;

use common::test_db;
use rust_server::db::{matches as match_db, orders as order_db, schema, wallets as wallet_db};

#[test]
fn migration_idempotent() {
    let pool = test_db();
    let conn = pool.get().unwrap();
    // Running migrations again should not error
    schema::run_migrations(&conn).expect("migrations should be idempotent");
}

#[test]
fn wallet_create_and_get() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id = wallet_db::insert_wallet(
        &conn,
        "my-wallet",
        b"encrypted_key_data",
        &[0xabu8; 32],
        "ckt1q...abc",
    )
    .expect("insert should succeed");
    assert!(id > 0);

    let wallet = wallet_db::get_wallet_by_id(&conn, id).expect("should find wallet");
    assert_eq!(wallet.label, "my-wallet");
    assert_eq!(wallet.encrypted_key, b"encrypted_key_data");
    assert_eq!(wallet.lock_hash, vec![0xabu8; 32]);
    assert_eq!(wallet.ckb_address, "ckt1q...abc");
}

#[test]
fn wallet_list() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    wallet_db::insert_wallet(&conn, "w1", b"k1", &[1u8; 32], "addr1").unwrap();
    wallet_db::insert_wallet(&conn, "w2", b"k2", &[2u8; 32], "addr2").unwrap();
    wallet_db::insert_wallet(&conn, "w3", b"k3", &[3u8; 32], "addr3").unwrap();

    let wallets = wallet_db::list_wallets(&conn).unwrap();
    assert_eq!(wallets.len(), 3);
}

#[test]
fn wallet_delete() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id = wallet_db::insert_wallet(&conn, "to-delete", b"key", &[9u8; 32], "addr").unwrap();
    let deleted = wallet_db::delete_wallet(&conn, id).unwrap();
    assert!(deleted);

    let result = wallet_db::get_wallet_by_id(&conn, id);
    assert!(result.is_err());
}

#[test]
fn wallet_delete_nonexistent() {
    let pool = test_db();
    let conn = pool.get().unwrap();
    let deleted = wallet_db::delete_wallet(&conn, 9999).unwrap();
    assert!(!deleted);
}

#[test]
fn wallet_unique_lock_hash() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    wallet_db::insert_wallet(&conn, "w1", b"k1", &[5u8; 32], "addr").unwrap();
    let result = wallet_db::insert_wallet(&conn, "w2", b"k2", &[5u8; 32], "addr2");
    assert!(result.is_err());
}

#[test]
fn wallet_get_by_lock_hash() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    wallet_db::insert_wallet(&conn, "by-hash", b"key", &[7u8; 32], "addr7").unwrap();
    let wallet = wallet_db::get_wallet_by_lock_hash(&conn, &[7u8; 32]).unwrap();
    assert_eq!(wallet.label, "by-hash");
}

#[test]
fn order_create_and_get() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id = order_db::insert_order(
        &conn,
        "tx_001",
        0,
        "ckt1q...buyer",
        100_000_000_000,
        300_000,
        None,
    )
    .unwrap();
    assert!(id > 0);

    let order = order_db::get_order_by_id(&conn, id).unwrap();
    assert_eq!(order.tx_hash, "tx_001");
    assert_eq!(order.buyer_address, "ckt1q...buyer");
    assert_eq!(order.status, "live");
}

#[test]
fn order_list_by_status() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    order_db::insert_order(&conn, "tx1", 0, "a", 100, 10, None).unwrap();
    order_db::insert_order(&conn, "tx2", 0, "b", 200, 20, None).unwrap();
    order_db::insert_order(&conn, "tx3", 0, "c", 300, 30, None).unwrap();

    // Cancel tx2
    order_db::update_order_status(&conn, 2, "cancelled").unwrap();

    let live = order_db::list_orders(&conn, Some("live")).unwrap();
    assert_eq!(live.len(), 2);

    let cancelled = order_db::list_orders(&conn, Some("cancelled")).unwrap();
    assert_eq!(cancelled.len(), 1);
}

#[test]
fn order_update_status() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id = order_db::insert_order(&conn, "tx_upd", 0, "buyer", 500, 50, None).unwrap();
    order_db::update_order_status(&conn, id, "matched").unwrap();

    let order = order_db::get_order_by_id(&conn, id).unwrap();
    assert_eq!(order.status, "matched");
}

#[test]
fn order_update_status_nonexistent() {
    let pool = test_db();
    let conn = pool.get().unwrap();
    let result = order_db::update_order_status(&conn, 9999, "cancelled");
    assert!(result.is_err());
}

#[test]
fn match_create_and_get() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id = match_db::insert_match(
        &conn,
        "match_tx",
        0,
        "order_tx",
        0,
        "ckt1q...seller",
        150.5,
        200_000,
        None::<&str>,
    )
    .unwrap();

    let m = match_db::get_match_by_id(&conn, id).unwrap();
    assert_eq!(m.tx_hash, "match_tx");
    assert_eq!(m.seller_address, "ckt1q...seller");
    assert_eq!(m.rent_per_block, 150.5);
    assert_eq!(m.status, "live");
    assert_eq!(m.last_extraction_block, 0);
}

#[test]
fn match_update_extraction() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id = match_db::insert_match(
        &conn,
        "m_tx",
        0,
        "o_tx",
        0,
        "seller",
        50.0,
        100_000,
        None::<&str>,
    )
    .unwrap();

    match_db::update_match_extraction(&conn, id, 5000).unwrap();
    let m = match_db::get_match_by_id(&conn, id).unwrap();
    assert_eq!(m.last_extraction_block, 5000);
}

#[test]
fn match_update_status() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id =
        match_db::insert_match(&conn, "m2", 0, "o2", 0, "s2", 10.0, 500, None::<&str>).unwrap();

    match_db::update_match_status(&conn, id, "exhausted").unwrap();
    let m = match_db::get_match_by_id(&conn, id).unwrap();
    assert_eq!(m.status, "exhausted");
}

#[test]
fn match_list_by_status() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    match_db::insert_match(&conn, "m1", 0, "o1", 0, "s1", 1.0, 100, None::<&str>).unwrap();
    match_db::insert_match(&conn, "m2", 0, "o2", 0, "s2", 2.0, 200, None::<&str>).unwrap();
    match_db::update_match_status(&conn, 1, "destroyed").unwrap();

    let live = match_db::list_matches(&conn, Some("live")).unwrap();
    assert_eq!(live.len(), 1);

    let destroyed = match_db::list_matches(&conn, Some("destroyed")).unwrap();
    assert_eq!(destroyed.len(), 1);
}

#[test]
fn extraction_history_insert() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    let id =
        match_db::insert_extraction(&conn, "match_tx", 0, 50_000, 1500, "extract_tx_hash").unwrap();
    assert!(id > 0);

    let history = match_db::get_extractions_for_match(&conn, "match_tx", 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].extracted_amount, 50_000);
}

#[test]
fn total_extracted_aggregates() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    match_db::insert_extraction(&conn, "a", 0, 1000, 100, "tx1").unwrap();
    match_db::insert_extraction(&conn, "b", 0, 2000, 200, "tx2").unwrap();
    match_db::insert_extraction(&conn, "c", 0, 3000, 300, "tx3").unwrap();

    let total = match_db::total_extracted(&conn).unwrap();
    assert_eq!(total, 6000);
}

#[test]
fn order_unique_outpoint() {
    let pool = test_db();
    let conn = pool.get().unwrap();

    order_db::insert_order(&conn, "dup_tx", 0, "a", 100, 10, None).unwrap();
    let result = order_db::insert_order(&conn, "dup_tx", 0, "b", 200, 20, None);
    assert!(result.is_err());
}
