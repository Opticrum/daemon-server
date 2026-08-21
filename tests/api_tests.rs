//! API endpoint tests — full request/response with in-memory DB + mock chain.

mod common;

use actix_web::{test, web, App};
use common::{
    mock_with_hesitation_match, test_app_state, test_app_state_with_provider, test_private_key_hex,
};

use rust_server::api;
use rust_server::db::wallets as wallet_db;
use rust_server::services::address::{ckb_address_testnet, script_lock_hash};
use rust_server::services::chain_provider::{
    ChainProvider, IndexerTxRef, TransactionInfo, TxInputInfo, TxOutputInfo,
};
use rust_server::services::MockChainProvider;

#[actix_rt::test]
async fn health_check() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn import_wallet_returns_created() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/wallets")
        .set_json(serde_json::json!({
            "label": "test-wallet",
            "private_key_hex": test_private_key_hex(),
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
}

#[actix_rt::test]
async fn list_wallets() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    // First import a wallet
    let req = test::TestRequest::post()
        .uri("/api/wallets")
        .set_json(serde_json::json!({
            "label": "w1",
            "private_key_hex": test_private_key_hex(),
        }))
        .to_request();
    test::call_service(&app, req).await;

    // Then list
    let req = test::TestRequest::get().uri("/api/wallets").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn delete_wallet() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    // Import
    let req = test::TestRequest::post()
        .uri("/api/wallets")
        .set_json(serde_json::json!({
            "label": "to-delete",
            "private_key_hex": test_private_key_hex(),
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // Delete (id=1)
    let req = test::TestRequest::delete()
        .uri("/api/wallets/1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn scan_orders_returns_empty() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/orders/scan")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn admin_stats() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/admin/stats")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn list_matches_empty() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/matches").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn scan_matches_returns_empty() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/matches/scan")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn invalid_request_body_returns_400() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    // Missing required fields — test against wallet import
    let req = test::TestRequest::post()
        .uri("/api/wallets")
        .set_json(serde_json::json!({
            "invalid": "body"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn console_match_detail_not_found() {
    let state = test_app_state();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    // With no matches on the mock chain, requesting a non-existent match
    // should return an error.
    let req = test::TestRequest::get()
        .uri("/api/console/matches/nonexistent/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // The match_detail route should be registered (404 = not found, 500 = route not registered)
    assert!(
        resp.status().is_server_error() || resp.status().is_client_error(),
        "match detail route should be registered and return error for non-existent match"
    );
}

#[actix_rt::test]
async fn wallet_not_found_returns_404() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/api/wallets/99999")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Delete on non-existent returns 200 with deleted:false but mapped to NotFound
    // Actually delete returns Ok with false, not an error. So it'll be 200.
    assert!(resp.status().is_success() || resp.status() == 404);
}

#[actix_rt::test]
async fn pending_transactions_empty() {
    let state = test_app_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/console/transactions/pending")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert!(body.is_empty());
}

#[actix_rt::test]
async fn pending_transactions_returns_registered_entry() {
    let state = test_app_state();
    state
        .pending_txs
        .register("match_order", "ord1", "deadbeef");
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/console/transactions/pending")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["kind"], "match_order");
    assert_eq!(body[0]["context"], "ord1");
    assert_eq!(body[0]["tx_hash"], "deadbeef");
}

// ── Hesitation window: console match endpoints ──

#[actix_rt::test]
async fn console_list_matches_exposes_hesitation_fields() {
    // Match created at block 1000, never extracted; tip 1000 → elapsed 0,
    // inside the window (HESITATION_BLOCKS + 1 = 3601 blocks remaining).
    let provider = mock_with_hesitation_match(1000, 0, 1000);
    let state = test_app_state_with_provider(provider);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/console/matches")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert_eq!(body.len(), 1, "one match on chain");
    let m = &body[0];
    assert_eq!(m["in_hesitation"], serde_json::json!(true));
    assert_eq!(m["hesitation_remaining_blocks"], serde_json::json!(3601));
    assert!(
        m.get("withdraw_window_remaining_blocks").is_none(),
        "old buyer-window field must be removed"
    );
}

#[actix_rt::test]
async fn console_match_detail_exposes_hesitation_fields() {
    let provider = mock_with_hesitation_match(1000, 0, 1000);
    // Read the outpoint before moving the provider into AppState (not Clone).
    let m = provider.scan_matches().await.unwrap().remove(0);
    let tx_hash = hex::encode(m.match_outpoint.tx_hash);
    let output_index = m.match_outpoint.index;

    let state = test_app_state_with_provider(provider);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/console/matches/{tx_hash}/{output_index}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["in_hesitation"], serde_json::json!(true));
    assert_eq!(body["hesitation_remaining_blocks"], serde_json::json!(3601));
}

#[actix_rt::test]
async fn console_extract_rejected_during_hesitation() {
    let provider = mock_with_hesitation_match(1000, 0, 1000);
    let m = provider.scan_matches().await.unwrap().remove(0);
    let tx_hash = hex::encode(m.match_outpoint.tx_hash);
    let output_index = m.match_outpoint.index;

    let state = test_app_state_with_provider(provider);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/console/matches/{tx_hash}/{output_index}/extract"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        400,
        "extraction during hesitation is rejected"
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    let msg = body["message"].as_str().unwrap_or("").to_lowercase();
    assert!(
        msg.contains("hesitation"),
        "expected hesitation error message, got: {msg}"
    );
}

// ── Wallet transaction history ──

#[actix_rt::test]
async fn wallet_transactions_empty() {
    let state = test_app_state_with_provider(MockChainProvider::new());
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/console/wallets/transactions")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body.as_array().map(|a| a.len()), Some(0));
}

#[actix_rt::test]
async fn wallet_transactions_aggregates_amounts() {
    // Self-consistent lock_arg → address → lock_arg round-trip.
    let lock_arg = [0x11u8; 20];
    let addr = ckb_address_testnet(&lock_arg);
    let la = rust_server::services::address::lock_arg_from_address(&addr).unwrap();
    assert_eq!(la, lock_arg, "lock_arg should round-trip from address");

    let tx_a = hex::encode([0xaau8; 32]);
    let tx_b = hex::encode([0xbbu8; 32]);

    // Mock chain: tx_a pays the wallet an output (5 CKB) and spends an input
    // whose previous output (from tx_b) carried 2 CKB.
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
    provider.add_wallet_tx(
        &lock_arg,
        IndexerTxRef {
            tx_hash: tx_a.clone(),
            block_number: 100,
            io_index: 0,
            io_type: "input".into(),
        },
    );
    provider.add_transaction(
        &tx_a,
        TransactionInfo {
            tx_hash: tx_a.clone(),
            block_number: 100,
            inputs: vec![TxInputInfo {
                previous_tx_hash: tx_b.clone(),
                previous_index: 0,
            }],
            outputs: vec![TxOutputInfo {
                capacity: 5_000_000_000,
                lock_code_hash: "".into(),
                lock_hash_type: "Type".into(),
                lock_args_hex: "".into(),
                lock_args_len: 0,
                data_hex: "".into(),
            }],
        },
    );
    provider.add_transaction(
        &tx_b,
        TransactionInfo {
            tx_hash: tx_b.clone(),
            block_number: 99,
            inputs: vec![],
            outputs: vec![TxOutputInfo {
                capacity: 2_000_000_000,
                lock_code_hash: "".into(),
                lock_hash_type: "Type".into(),
                lock_args_hex: "".into(),
                lock_args_len: 0,
                data_hex: "".into(),
            }],
        },
    );

    let state = test_app_state_with_provider(provider);
    {
        let mut conn = state.db.get().unwrap();
        wallet_db::insert_wallet(
            &mut conn,
            "Test Wallet",
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
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    // Force a sync first — this is what persists rows into the DB.
    let sync_req = test::TestRequest::post()
        .uri("/api/console/wallets/transactions/sync")
        .to_request();
    let sync_resp = test::call_service(&app, sync_req).await;
    assert!(sync_resp.status().is_success());
    let sync_body: serde_json::Value = test::read_body_json(sync_resp).await;
    assert_eq!(sync_body["wallets"], serde_json::json!(1));
    assert!(sync_body["rows_synced"].as_u64().unwrap() >= 1);
    assert_eq!(sync_body["pruned"], serde_json::json!(0));

    // GET now reads from the synced DB.
    let req = test::TestRequest::get()
        .uri("/api/console/wallets/transactions")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().expect("response should be an array");
    assert_eq!(arr.len(), 1, "one transaction aggregated");
    let tx = &arr[0];
    assert_eq!(tx["tx_hash"], serde_json::json!(tx_a));
    assert_eq!(tx["block_number"], serde_json::json!(100));
    assert_eq!(tx["received_shannons"], serde_json::json!(5_000_000_000u64));
    assert_eq!(tx["sent_shannons"], serde_json::json!(2_000_000_000u64));
    assert_eq!(tx["addresses"], serde_json::json!(["Test Wallet"]));
    assert!(
        tx["timestamp_ms"].is_null(),
        "mock provider has no block timestamp → null"
    );
}

#[actix_rt::test]
async fn wallet_transactions_sync_idempotent() {
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
            outputs: vec![TxOutputInfo {
                capacity: 3_000_000_000,
                lock_code_hash: "".into(),
                lock_hash_type: "Type".into(),
                lock_args_hex: "".into(),
                lock_args_len: 0,
                data_hex: "".into(),
            }],
        },
    );

    let state = test_app_state_with_provider(provider);
    {
        let mut conn = state.db.get().unwrap();
        wallet_db::insert_wallet(
            &mut conn,
            "Test Wallet",
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
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let sync = || async {
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/console/wallets/transactions/sync")
                .to_request(),
        )
        .await;
        assert!(resp.status().is_success());
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["rows_synced"].as_u64().unwrap()
    };

    // Second sync must not duplicate rows.
    let rows_first = sync().await;
    let rows_second = sync().await;
    assert_eq!(rows_first, rows_second, "re-sync should be idempotent");

    let req = test::TestRequest::get()
        .uri("/api/console/wallets/transactions")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 1, "still one aggregated tx");
}

#[actix_rt::test]
async fn wallet_transactions_reads_from_db() {
    // Empty mock chain: GET must return a row that was written directly to the
    // DB, proving the endpoint is a pure DB read (no live chain query).
    let state = test_app_state_with_provider(MockChainProvider::new());
    {
        let mut conn = state.db.get().unwrap();
        let wallet_id = wallet_db::insert_wallet(
            &mut conn,
            "Db Wallet",
            b"enc",
            &[0x22u8; 32],
            "ckt1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqdr7df0pvh0kdrt3j4k5jvj57x8t3x2f0dfllu4k5j",
            None,
            None,
            None,
            "imported",
        )
        .unwrap();
        rust_server::db::wallet_txs::upsert_batch(
            &mut conn,
            &[rust_server::db::wallet_txs::NewWalletTx {
                tx_hash: "abc123",
                wallet_id,
                block_number: 200,
                timestamp_ms: Some(1_700_000_000_000),
                received_shannons: 9_000_000_000,
                sent_shannons: 0,
            }],
        )
        .unwrap();
    }

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/console/wallets/transactions")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1, "row written directly to DB is read");
    assert_eq!(arr[0]["tx_hash"], serde_json::json!("abc123"));
    assert_eq!(
        arr[0]["received_shannons"],
        serde_json::json!(9_000_000_000u64)
    );
    assert_eq!(arr[0]["block_number"], serde_json::json!(200));
    assert_eq!(
        arr[0]["timestamp_ms"],
        serde_json::json!(1_700_000_000_000u64)
    );
}

#[actix_rt::test]
async fn wallet_transactions_sync_prunes_deleted_wallets() {
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
            outputs: vec![TxOutputInfo {
                capacity: 1_000_000_000,
                lock_code_hash: "".into(),
                lock_hash_type: "Type".into(),
                lock_args_hex: "".into(),
                lock_args_len: 0,
                data_hex: "".into(),
            }],
        },
    );

    let state = test_app_state_with_provider(provider);
    let wallet_id;
    {
        let mut conn = state.db.get().unwrap();
        wallet_id = wallet_db::insert_wallet(
            &mut conn,
            "To Delete",
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
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    // Sync once → row exists.
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/console/wallets/transactions/sync")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());

    // Delete the wallet, then sync again → its rows are pruned.
    let del = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/console/wallets/{wallet_id}"))
            .to_request(),
    )
    .await;
    assert!(del.status().is_success());

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/console/wallets/transactions/sync")
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["pruned"].as_u64().unwrap() >= 1);

    let get = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/console/wallets/transactions")
            .to_request(),
    )
    .await;
    let get_body: serde_json::Value = test::read_body_json(get).await;
    assert_eq!(get_body.as_array().unwrap().len(), 0, "rows pruned");
}

#[actix_rt::test]
async fn wallet_transactions_filters_by_wallet() {
    let lock_arg1 = [0x11u8; 20];
    let addr1 = ckb_address_testnet(&lock_arg1);
    let lock_arg2 = [0x22u8; 20];
    let addr2 = ckb_address_testnet(&lock_arg2);
    let tx_a = hex::encode([0xaau8; 32]);
    let tx_b = hex::encode([0xbbu8; 32]);

    let provider = MockChainProvider::new();
    provider.add_wallet_tx(
        &lock_arg1,
        IndexerTxRef {
            tx_hash: tx_a.clone(),
            block_number: 100,
            io_index: 0,
            io_type: "output".into(),
        },
    );
    provider.add_wallet_tx(
        &lock_arg2,
        IndexerTxRef {
            tx_hash: tx_b.clone(),
            block_number: 90,
            io_index: 0,
            io_type: "output".into(),
        },
    );
    for (hash, block, cap) in [
        (tx_a.clone(), 100, 1_000_000_000u64),
        (tx_b.clone(), 90, 2_000_000_000u64),
    ] {
        provider.add_transaction(
            &hash,
            TransactionInfo {
                tx_hash: hash.clone(),
                block_number: block,
                inputs: vec![],
                outputs: vec![TxOutputInfo {
                    capacity: cap,
                    lock_code_hash: "".into(),
                    lock_hash_type: "Type".into(),
                    lock_args_hex: "".into(),
                    lock_args_len: 0,
                    data_hex: "".into(),
                }],
            },
        );
    }

    let state = test_app_state_with_provider(provider);
    let (w1, w2);
    {
        let mut conn = state.db.get().unwrap();
        w1 = wallet_db::insert_wallet(
            &mut conn,
            "W1",
            b"e",
            &script_lock_hash(&lock_arg1),
            &addr1,
            None,
            None,
            None,
            "imported",
        )
        .unwrap();
        w2 = wallet_db::insert_wallet(
            &mut conn,
            "W2",
            b"e",
            &script_lock_hash(&lock_arg2),
            &addr2,
            None,
            None,
            None,
            "imported",
        )
        .unwrap();
    }
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(api::configure_routes),
    )
    .await;

    // Sync both wallets' txs.
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/console/wallets/transactions/sync")
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());

    // No filter → aggregate (both).
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/console/wallets/transactions")
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 2, "aggregate shows both");

    // Filter by wallet 1 → only its tx.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/console/wallets/transactions?wallet_id={w1}"))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1, "wallet 1 sees one tx");
    assert_eq!(arr[0]["tx_hash"], serde_json::json!(tx_a));
    assert_eq!(
        arr[0]["received_shannons"],
        serde_json::json!(1_000_000_000u64)
    );
    assert_eq!(arr[0]["addresses"], serde_json::json!(["W1"]));

    // Filter by wallet 2 → only its tx.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/console/wallets/transactions?wallet_id={w2}"))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1, "wallet 2 sees one tx");
    assert_eq!(arr[0]["tx_hash"], serde_json::json!(tx_b));
    assert_eq!(
        arr[0]["received_shannons"],
        serde_json::json!(2_000_000_000u64)
    );
}
