//! Shared test utilities — in-memory DB setup, test keys, mock chain data.

#![allow(dead_code)]

use opticrum_calculator::types::MatchInfo;
use opticrum_protocol::{CompressedPubkey, MatchArgs, MatchData, OrderArgs, OutPoint};

use rust_server::db;
use rust_server::services::chain_provider::{CellOutput, MockChainProvider};

/// Create an in-memory SQLite database with all migrations applied.
pub fn test_db() -> rust_server::db::DbPool {
    db::init_test_db()
}

/// A deterministic secp256k1 test private key (32 bytes).
/// This is the hex encoding of a known test key.
pub fn test_private_key_hex() -> String {
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
}

/// Build a mock OutPoint from a byte seed and index.
fn mock_outpoint(seed: &[u8], index: u32) -> OutPoint {
    let mut hash = [0u8; 32];
    let len = seed.len().min(32);
    hash[..len].copy_from_slice(&seed[..len]);
    OutPoint::new(hash, index)
}

/// Create a mock chain provider pre-loaded with one match.
pub fn mock_with_match() -> MockChainProvider {
    MockChainProvider::with_matches(vec![MatchInfo {
        match_args: MatchArgs::new(
            OrderArgs::new(CompressedPubkey::new([0u8; 33]), [0xabu8; 32]),
            mock_outpoint(b"channel_001______________________", 0),
            [0xcdu8; 32],
        ),
        match_data: {
            let mut md = MatchData::new(0, 100);
            md.last_extraction_block = 500;
            md
        },
        xudt: None,
        ckb_capacity: 50_000_000_000,
        match_outpoint: mock_outpoint(b"match_tx_001_____________________", 0),
        match_current_block: 0,
    }])
}

/// Create a test cell output (fake channel cell).
pub fn test_cell(capacity: u64) -> CellOutput {
    CellOutput {
        capacity,
        lock_hash: [0u8; 32],
        type_hash: None,
        data: vec![],
    }
}

/// Helper to create an AppState for API tests (with in-memory DB + Config).
pub fn test_app_state() -> rust_server::api::AppState {
    let config = rust_server::config::Config {
        config_file: None,
        port: 8080,
        database_url: ":memory:".to_string(),
        ckb_rpc_url: "http://localhost:8114".to_string(),
        ckb_indexer_url: "http://localhost:8116".to_string(),
        fiber_rpc_url: "http://localhost:8227".to_string(),
        bind_address: "0.0.0.0".to_string(),
        scheduler_interval_secs: 60,
        min_extraction_amount_shannons: 100_000_000,
        fee_rate: 1000,
        log_level: "info".to_string(),
        auto_match_enabled: false,
        auto_match_min_capacity: 10_000_000_000,
        auto_match_max_escrow_blocks: 432_000,
        auto_match_interval_secs: 120,
        keystore_path: "data/keystore.json".to_string(),
    };

    let scheduler_state = std::sync::Arc::new(std::sync::RwLock::new(
        rust_server::services::console::scheduler_state::SchedulerState::new(),
    ));

    rust_server::api::AppState {
        db: test_db(),
        config,
        chain_provider: std::sync::Arc::new(rust_server::services::MockChainProvider::new()),
        signer: std::sync::Arc::new(rust_server::services::external_signer::ExternalSigner::new(
            "testnet",
        )),
        tx_assembler: None,
        keystore_path: "data/keystore.json".to_string(),
        scheduler_state,
    }
}
