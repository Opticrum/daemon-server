//! Configuration — CLI arguments, environment variables, and TOML file.
//!
//! Configuration is loaded in priority order (highest wins):
//! 1. CLI flags
//! 2. Environment variables (`OPTICRUM_*`)
//! 3. TOML config file (`config.toml` or `--config <path>`)
//! 4. Built-in defaults
//!
//! Transaction signing uses the built-in HD wallet. Unlock the keystore
//! in the admin panel to load signing keys into memory.

use clap::Parser;
use serde::Deserialize;

/// Opticrum Rust Server — REST API and background rent extraction service.
#[derive(Parser, Clone, Debug, Deserialize)]
#[command(name = "opticrum-server", version, about)]
#[serde(default)]
pub struct Config {
    /// TOML config file path. When present, this file is loaded first,
    /// then CLI flags and env vars override its values.
    #[arg(long, env = "OPTICRUM_CONFIG")]
    pub config_file: Option<String>,

    /// Port to listen on
    #[arg(long, env = "OPTICRUM_PORT", default_value = "8080")]
    pub port: u16,

    /// SQLite database path
    #[arg(
        long,
        env = "OPTICRUM_DATABASE_URL",
        default_value = "data/opticrum.db"
    )]
    pub database_url: String,

    /// HD wallet keystore file path
    #[arg(
        long,
        env = "OPTICRUM_KEYSTORE_PATH",
        default_value = "data/keystore.json"
    )]
    pub keystore_path: String,

    /// Password to auto-unlock the HD wallet on startup.
    /// When set and the keystore file exists, the wallet is unlocked
    /// automatically — no manual unlock step needed in the admin panel.
    #[arg(long, env = "OPTICRUM_HD_WALLET_PASSWORD")]
    pub hd_wallet_password: Option<String>,

    /// CKB RPC URL
    #[arg(
        long,
        env = "OPTICRUM_CKB_RPC_URL",
        default_value = "http://localhost:8114"
    )]
    pub ckb_rpc_url: String,

    /// CKB Indexer URL
    #[arg(
        long,
        env = "OPTICRUM_CKB_INDEXER_URL",
        default_value = "http://localhost:8116"
    )]
    pub ckb_indexer_url: String,

    /// Fiber network node RPC URL (for channel querying)
    #[arg(
        long,
        env = "OPTICRUM_FIBER_RPC_URL",
        default_value = "http://localhost:8227"
    )]
    pub fiber_rpc_url: String,

    /// Bind address (network interface)
    #[arg(long, env = "OPTICRUM_BIND_ADDRESS", default_value = "0.0.0.0")]
    pub bind_address: String,

    /// Scheduler interval in seconds
    #[arg(long, env = "OPTICRUM_SCHEDULER_INTERVAL_SECS", default_value = "60")]
    pub scheduler_interval_secs: u64,

    /// Minimum extraction amount in shannons (1 CKB = 100_000_000 shannons)
    #[arg(
        long,
        env = "OPTICRUM_MIN_EXTRACTION_SHANNONS",
        default_value = "100000000"
    )]
    pub min_extraction_amount_shannons: u64,

    /// Log level: trace, debug, info, warn, error.
    /// Overrides RUST_LOG for the opticrum_server crate only.
    #[arg(long, env = "OPTICRUM_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Transaction fee rate in shannons per KB
    #[arg(long, env = "OPTICRUM_FEE_RATE", default_value = "1000")]
    pub fee_rate: u64,

    /// Enable automatic rent extraction (background task)
    #[arg(long, env = "OPTICRUM_RENT_EXTRACTION_ENABLED", default_value = "true")]
    pub rent_extraction_enabled: bool,

    // -----------------------------------------------------------------------
    // Auto-match configuration
    // -----------------------------------------------------------------------
    /// Enable automatic order matching (background task)
    #[arg(long, env = "OPTICRUM_AUTO_MATCH_ENABLED", default_value = "false")]
    pub auto_match_enabled: bool,

    /// Minimum CKB capacity (shannons) for auto-match eligibility
    #[arg(
        long,
        env = "OPTICRUM_AUTO_MATCH_MIN_CAPACITY",
        default_value = "10000000000"
    )]
    pub auto_match_min_capacity: u64,

    /// Maximum escrow blocks for auto-match eligibility
    #[arg(
        long,
        env = "OPTICRUM_AUTO_MATCH_MAX_ESCROW_BLOCKS",
        default_value = "432000"
    )]
    pub auto_match_max_escrow_blocks: u64,

    /// Auto-match cycle interval in seconds
    #[arg(long, env = "OPTICRUM_AUTO_MATCH_INTERVAL_SECS", default_value = "120")]
    pub auto_match_interval_secs: u64,

    /// Enable background chain cache indexer (orders/matches/channels)
    #[arg(long, env = "OPTICRUM_CHAIN_CACHE_ENABLED", default_value = "true")]
    pub chain_cache_enabled: bool,

    /// Chain cache refresh interval in seconds
    #[arg(long, env = "OPTICRUM_CHAIN_CACHE_INTERVAL_SECS", default_value = "30")]
    pub chain_cache_interval_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_file: None,
            port: 8080,
            database_url: "data/opticrum.db".into(),
            ckb_rpc_url: "http://localhost:8114".into(),
            ckb_indexer_url: "http://localhost:8116".into(),
            fiber_rpc_url: "http://localhost:8227".into(),
            bind_address: "0.0.0.0".into(),
            scheduler_interval_secs: 60,
            min_extraction_amount_shannons: 100_000_000,
            fee_rate: 1000,
            rent_extraction_enabled: true,
            log_level: "info".into(),
            auto_match_enabled: false,
            auto_match_min_capacity: 10_000_000_000,
            auto_match_max_escrow_blocks: 432_000,
            auto_match_interval_secs: 120,
            chain_cache_enabled: true,
            chain_cache_interval_secs: 30,
            keystore_path: "data/keystore.json".into(),
            hd_wallet_password: None,
        }
    }
}

impl Config {
    /// Load configuration from TOML file, env vars, and CLI args.
    ///
    /// Priority (highest wins):
    /// 1. CLI flags
    /// 2. Environment variables
    /// 3. TOML file values
    /// 4. Built-in defaults
    pub fn load() -> Self {
        // Step 1: find the config file path from CLI --config or env OPTICRUM_CONFIG.
        let args: Vec<String> = std::env::args().collect();
        let config_path = args
            .iter()
            .position(|a| a == "--config")
            .and_then(|i| args.get(i + 1).cloned())
            .or_else(|| std::env::var("OPTICRUM_CONFIG").ok())
            .unwrap_or_else(|| "config.toml".into());

        // Step 2: try to load TOML config file
        let from_file: Option<Self> = match std::fs::read_to_string(&config_path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(cfg) => {
                    tracing::info!("Loaded config from {}", config_path);
                    Some(cfg)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse config file '{}': {} — using defaults",
                        config_path,
                        e
                    );
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                tracing::warn!(
                    "Failed to read config file '{}': {} — using defaults",
                    config_path,
                    e
                );
                None
            }
        };

        // Step 3: parse full CLI args (with env var overrides)
        let from_cli = Self::parse();

        // Step 4: merge — file values as base, CLI (which includes env vars) on top.
        match from_file {
            Some(file_cfg) => Self::merge(file_cfg, from_cli),
            None => from_cli,
        }
    }

    /// Merge two configs: `file` as base, `cli` overrides non-default values.
    fn merge(file: Self, cli: Self) -> Self {
        let defaults = Self::default();
        Self {
            config_file: cli.config_file,
            port: if cli.port != defaults.port {
                cli.port
            } else {
                file.port
            },
            database_url: if cli.database_url != defaults.database_url {
                cli.database_url
            } else {
                file.database_url
            },
            ckb_rpc_url: if cli.ckb_rpc_url != defaults.ckb_rpc_url {
                cli.ckb_rpc_url
            } else {
                file.ckb_rpc_url
            },
            ckb_indexer_url: if cli.ckb_indexer_url != defaults.ckb_indexer_url {
                cli.ckb_indexer_url
            } else {
                file.ckb_indexer_url
            },
            fiber_rpc_url: if cli.fiber_rpc_url != defaults.fiber_rpc_url {
                cli.fiber_rpc_url
            } else {
                file.fiber_rpc_url
            },
            bind_address: if cli.bind_address != defaults.bind_address {
                cli.bind_address
            } else {
                file.bind_address
            },
            scheduler_interval_secs: if cli.scheduler_interval_secs
                != defaults.scheduler_interval_secs
            {
                cli.scheduler_interval_secs
            } else {
                file.scheduler_interval_secs
            },
            min_extraction_amount_shannons: if cli.min_extraction_amount_shannons
                != defaults.min_extraction_amount_shannons
            {
                cli.min_extraction_amount_shannons
            } else {
                file.min_extraction_amount_shannons
            },
            fee_rate: if cli.fee_rate != defaults.fee_rate {
                cli.fee_rate
            } else {
                file.fee_rate
            },
            rent_extraction_enabled: if cli.rent_extraction_enabled
                != defaults.rent_extraction_enabled
            {
                cli.rent_extraction_enabled
            } else {
                file.rent_extraction_enabled
            },
            log_level: if cli.log_level != defaults.log_level {
                cli.log_level
            } else {
                file.log_level
            },
            auto_match_enabled: if cli.auto_match_enabled != defaults.auto_match_enabled {
                cli.auto_match_enabled
            } else {
                file.auto_match_enabled
            },
            auto_match_min_capacity: if cli.auto_match_min_capacity
                != defaults.auto_match_min_capacity
            {
                cli.auto_match_min_capacity
            } else {
                file.auto_match_min_capacity
            },
            auto_match_max_escrow_blocks: if cli.auto_match_max_escrow_blocks
                != defaults.auto_match_max_escrow_blocks
            {
                cli.auto_match_max_escrow_blocks
            } else {
                file.auto_match_max_escrow_blocks
            },
            auto_match_interval_secs: if cli.auto_match_interval_secs
                != defaults.auto_match_interval_secs
            {
                cli.auto_match_interval_secs
            } else {
                file.auto_match_interval_secs
            },
            chain_cache_enabled: if cli.chain_cache_enabled != defaults.chain_cache_enabled {
                cli.chain_cache_enabled
            } else {
                file.chain_cache_enabled
            },
            chain_cache_interval_secs: if cli.chain_cache_interval_secs
                != defaults.chain_cache_interval_secs
            {
                cli.chain_cache_interval_secs
            } else {
                file.chain_cache_interval_secs
            },
            keystore_path: if cli.keystore_path != defaults.keystore_path {
                cli.keystore_path
            } else {
                file.keystore_path
            },
            hd_wallet_password: cli.hd_wallet_password.or(file.hd_wallet_password),
        }
    }

    pub fn from_args() -> Self {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let config = Config::try_parse_from(["opticrum-server"]).expect("should parse");
        assert_eq!(config.port, 8080);
        assert_eq!(config.database_url, "data/opticrum.db");
        assert_eq!(config.bind_address, "0.0.0.0");
        assert_eq!(config.scheduler_interval_secs, 60);
        assert_eq!(config.fee_rate, 1000);
        assert!(!config.auto_match_enabled);
    }

    #[test]
    fn test_custom_port() {
        clear_opticrum_env();
        let config = Config::try_parse_from(["opticrum-server", "--port", "9090"])
            .expect("should parse custom port");
        assert_eq!(config.port, 9090);
    }

    #[test]
    fn test_load_from_toml_string() {
        let toml_content = r#"
port = 9090
fee_rate = 5000
database_url = "file.db"
ckb_rpc_url = "http://file-ckb:8114"
ckb_indexer_url = "http://file-idx:8116"
auto_match_enabled = true
auto_match_min_capacity = 5000000000
"#;
        let file_cfg: Config = toml::from_str(toml_content).expect("valid TOML");
        assert_eq!(file_cfg.port, 9090);
        assert_eq!(file_cfg.fee_rate, 5000);
        assert_eq!(file_cfg.database_url, "file.db");
        assert!(file_cfg.auto_match_enabled);
        assert_eq!(file_cfg.auto_match_min_capacity, 5_000_000_000);
    }

    #[test]
    fn test_toml_merge_cli_wins() {
        let file = Config {
            port: 9090,
            ..Default::default()
        };
        let cli = Config {
            port: 3000,
            ..Default::default()
        };
        let merged = Config::merge(file, cli);
        assert_eq!(merged.port, 3000, "CLI should override file port");
    }
}
