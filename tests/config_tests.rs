//! Configuration tests — CLI argument parsing and defaults.

use clap::Parser;
use rust_server::config::Config;

/// Clear all OPTICRUM_ env vars to prevent test leakage.
fn clear_opticrum_env() {
    for (key, _) in std::env::vars() {
        if key.starts_with("OPTICRUM_") {
            std::env::remove_var(&key);
        }
    }
}

#[test]
fn test_default_values() {
    clear_opticrum_env();
    let config = Config::try_parse_from(["opticrum-server"]).expect("should parse with defaults");

    assert_eq!(config.port, 8080);
    assert_eq!(config.database_url, "data/opticrum.db");
    assert_eq!(config.ckb_rpc_url, "http://localhost:8114");
    assert_eq!(config.ckb_indexer_url, "http://localhost:8116");
    assert_eq!(config.log_level, "info");
    assert_eq!(config.scheduler_interval_secs, 60);
    assert_eq!(config.min_extraction_amount_shannons, 100_000_000);
    assert_eq!(config.fee_rate, 1000);
}

#[test]
fn test_custom_port() {
    clear_opticrum_env();
    let config = Config::try_parse_from(["opticrum-server", "--port", "9090"])
        .expect("should parse custom port");
    assert_eq!(config.port, 9090);
}

#[test]
fn test_custom_database_url() {
    clear_opticrum_env();
    let config = Config::try_parse_from([
        "opticrum-server",
        "--database-url",
        "/tmp/custom.db",
    ])
    .expect("should parse custom DB path");
    assert_eq!(config.database_url, "/tmp/custom.db");
}

#[test]
fn test_all_custom_values() {
    clear_opticrum_env();
    let config = Config::try_parse_from([
        "opticrum-server",
        "--port", "3000",
        "--database-url", "custom.db",
        "--ckb-rpc-url", "http://ckb:8114",
        "--ckb-indexer-url", "http://indexer:8116",
        "--log-level", "debug",
        "--scheduler-interval-secs", "120",
        "--min-extraction-amount-shannons", "500000",
        "--fee-rate", "5000",
        "--auto-match-enabled",
        "--auto-match-min-capacity", "5000000000",
    ])
    .expect("should parse all custom values");

    assert_eq!(config.port, 3000);
    assert_eq!(config.database_url, "custom.db");
    assert_eq!(config.ckb_rpc_url, "http://ckb:8114");
    assert_eq!(config.ckb_indexer_url, "http://indexer:8116");
    assert_eq!(config.log_level, "debug");
    assert_eq!(config.scheduler_interval_secs, 120);
    assert_eq!(config.min_extraction_amount_shannons, 500_000);
    assert_eq!(config.fee_rate, 5000);
    assert!(config.auto_match_enabled);
    assert_eq!(config.auto_match_min_capacity, 5_000_000_000);
}
