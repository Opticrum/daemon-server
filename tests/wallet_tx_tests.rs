//! Wallet transaction sync service + scheduler cycle tests.
//!
//! Uses the in-memory mock chain provider (no CKB node needed); the SQLite
//! migrations (including `wallet_transactions`) run via `init_test_db`.

mod common;

use std::sync::{Arc, RwLock};

use common::test_db;
use rust_server::db::wallets as wallet_db;
use rust_server::scheduler::wallet_tx_sync::run_wallet_tx_sync_cycle;
use rust_server::services::address::{ckb_address_testnet, script_lock_hash};
use rust_server::services::chain_provider::{IndexerTxRef, TransactionInfo, TxOutputInfo};
use rust_server::services::console::scheduler_state::{SchedulerState, SharedSchedulerState};
use rust_server::services::{wallet_tx, MockChainProvider};

fn tx_output(capacity: u64) -> TxOutputInfo {
    TxOutputInfo {
        capacity,
        lock_code_hash: "".into(),
        lock_hash_type: "Type".into(),
        lock_args_hex: "".into(),
        lock_args_len: 0,
        data_hex: "".into(),
    }
}

#[actix_rt::test]
async fn sync_persists_and_lists() {
    let pool = test_db();
    let lock_arg = [0x11u8; 20];
    let addr = ckb_address_testnet(&lock_arg);
    let tx_a = hex::encode([0xaau8; 32]);

    let provider = MockChainProvider::new();
    provider.add_wallet_tx(
        &lock_arg,
        IndexerTxRef {
            tx_hash: tx_a.clone(),
            block_number: 100,
            io_index: 0,
            io_type: "output".into(),
        },
    );
    provider.add_transaction(
        &tx_a,
        TransactionInfo {
            tx_hash: tx_a.clone(),
            block_number: 100,
            inputs: vec![],
            outputs: vec![tx_output(5_000_000_000)],
        },
    );
    // Block 100 has a resolvable timestamp.
    provider.set_block_timestamp(100, 1_700_000_000_000);

    {
        let mut conn = pool.get().unwrap();
        wallet_db::insert_wallet(
            &mut conn,
            "W",
            b"enc",
            &script_lock_hash(&lock_arg),
            &addr,
            None,
            None,
            None,
            "imported",
        )
        .unwrap();
    }

    let stats = wallet_tx::sync(&pool, &provider).await.unwrap();
    assert_eq!(stats.wallets, 1);
    assert_eq!(stats.rows_synced, 1);
    assert_eq!(stats.pruned, 0);

    let list = wallet_tx::list(&pool, None).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].tx_hash, tx_a);
    assert_eq!(list[0].received_shannons, 5_000_000_000);
    assert_eq!(list[0].sent_shannons, 0);
    assert_eq!(list[0].timestamp_ms, Some(1_700_000_000_000));
    assert_eq!(list[0].addresses, vec!["W".to_string()]);

    // Re-sync is idempotent (upsert, not duplicate).
    let stats2 = wallet_tx::sync(&pool, &provider).await.unwrap();
    assert_eq!(stats2.rows_synced, 1);
    assert_eq!(wallet_tx::list(&pool, None).await.unwrap().len(), 1);
}

#[actix_rt::test]
async fn sync_cycle_reports_rows_and_events() {
    let pool = test_db();
    let lock_arg = [0x11u8; 20];
    let addr = ckb_address_testnet(&lock_arg);
    let tx_a = hex::encode([0xaau8; 32]);

    let provider = MockChainProvider::new();
    provider.add_wallet_tx(
        &lock_arg,
        IndexerTxRef {
            tx_hash: tx_a.clone(),
            block_number: 100,
            io_index: 0,
            io_type: "output".into(),
        },
    );
    provider.add_transaction(
        &tx_a,
        TransactionInfo {
            tx_hash: tx_a.clone(),
            block_number: 100,
            inputs: vec![],
            outputs: vec![tx_output(1_000_000_000)],
        },
    );

    {
        let mut conn = pool.get().unwrap();
        wallet_db::insert_wallet(
            &mut conn,
            "W2",
            b"enc",
            &script_lock_hash(&lock_arg),
            &addr,
            None,
            None,
            None,
            "imported",
        )
        .unwrap();
    }

    let shared: SharedSchedulerState = Arc::new(RwLock::new(SchedulerState::new()));
    let rows = run_wallet_tx_sync_cycle(&pool, &provider, Some(&shared))
        .await
        .unwrap();
    assert_eq!(rows, 1, "cycle returns rows synced");

    let events = shared.read().unwrap().events_since(0);
    assert!(
        events
            .iter()
            .any(|e| e.source == "wallet_tx" && e.message.contains("Synced")),
        "expected a wallet_tx sync event, got: {:?}",
        events.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}
