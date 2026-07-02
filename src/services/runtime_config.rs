//! Runtime-configurable settings backed by an `Arc<RwLock<>>`.
//!
//! These fields can be changed at runtime via the console API without a
//! server restart. URL fields take effect after restart (the chain provider
//! is initialized once at startup).

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Subset of `Config` that can be mutated at runtime.
/// URL fields are editable but require a restart to take effect.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeConfig {
    pub fee_rate: u64,
    pub rent_extraction_enabled: bool,
    pub scheduler_interval_secs: u64,
    pub min_extraction_amount_shannons: u64,
    pub auto_match_enabled: bool,
    pub auto_match_min_capacity: u64,
    pub auto_match_max_escrow_blocks: u64,
    pub auto_match_interval_secs: u64,
    /// CKB RPC URL (requires restart to take effect).
    pub ckb_rpc_url: String,
    /// CKB Indexer URL (requires restart to take effect).
    pub ckb_indexer_url: String,
    /// Fiber Network RPC URL (requires restart to take effect).
    pub fiber_rpc_url: String,
}

impl RuntimeConfig {
    /// Initial values from the startup `Config`.
    pub fn from_config(config: &Config) -> Self {
        Self {
            fee_rate: config.fee_rate,
            rent_extraction_enabled: config.rent_extraction_enabled,
            scheduler_interval_secs: config.scheduler_interval_secs,
            min_extraction_amount_shannons: config.min_extraction_amount_shannons,
            auto_match_enabled: config.auto_match_enabled,
            auto_match_min_capacity: config.auto_match_min_capacity,
            auto_match_max_escrow_blocks: config.auto_match_max_escrow_blocks,
            auto_match_interval_secs: config.auto_match_interval_secs,
            ckb_rpc_url: config.ckb_rpc_url.clone(),
            ckb_indexer_url: config.ckb_indexer_url.clone(),
            fiber_rpc_url: config.fiber_rpc_url.clone(),
        }
    }

    /// Reset all fields back to config.toml values.
    pub fn reset_from_config(&mut self, config: &Config) {
        *self = Self::from_config(config);
    }

    /// Apply a partial update — only the supplied fields are changed.
    pub fn apply_partial(&mut self, partial: &RuntimeConfigPartial) {
        if let Some(v) = partial.fee_rate {
            self.fee_rate = v;
        }
        if let Some(v) = partial.rent_extraction_enabled {
            self.rent_extraction_enabled = v;
        }
        if let Some(v) = partial.scheduler_interval_secs {
            self.scheduler_interval_secs = v;
        }
        if let Some(v) = partial.min_extraction_amount_shannons {
            self.min_extraction_amount_shannons = v;
        }
        if let Some(v) = partial.auto_match_enabled {
            self.auto_match_enabled = v;
        }
        if let Some(v) = partial.auto_match_min_capacity {
            self.auto_match_min_capacity = v;
        }
        if let Some(v) = partial.auto_match_max_escrow_blocks {
            self.auto_match_max_escrow_blocks = v;
        }
        if let Some(v) = partial.auto_match_interval_secs {
            self.auto_match_interval_secs = v;
        }
        if let Some(v) = &partial.ckb_rpc_url {
            self.ckb_rpc_url = v.clone();
        }
        if let Some(v) = &partial.ckb_indexer_url {
            self.ckb_indexer_url = v.clone();
        }
        if let Some(v) = &partial.fiber_rpc_url {
            self.fiber_rpc_url = v.clone();
        }
    }
}

/// All fields optional — used for `PUT /api/console/runtime-config`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RuntimeConfigPartial {
    pub fee_rate: Option<u64>,
    pub rent_extraction_enabled: Option<bool>,
    pub scheduler_interval_secs: Option<u64>,
    pub min_extraction_amount_shannons: Option<u64>,
    pub auto_match_enabled: Option<bool>,
    pub auto_match_min_capacity: Option<u64>,
    pub auto_match_max_escrow_blocks: Option<u64>,
    pub auto_match_interval_secs: Option<u64>,
    /// CKB RPC URL (requires restart to take effect).
    pub ckb_rpc_url: Option<String>,
    /// CKB Indexer URL (requires restart to take effect).
    pub ckb_indexer_url: Option<String>,
    /// Fiber Network RPC URL (requires restart to take effect).
    pub fiber_rpc_url: Option<String>,
}
