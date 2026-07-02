//! Runtime-configurable settings backed by an `Arc<RwLock<>>`.
//!
//! These fields can be changed at runtime via the console API without a
//! server restart. Immutable fields (URLs, port, etc.) stay in `Config`.

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Subset of `Config` that can be mutated at runtime and takes effect
/// immediately (read by schedulers, transaction assembler, etc.).
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
    }
}

/// All fields optional — used for `PUT /api/console/runtime-config`.
#[derive(Debug, Deserialize)]
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
}

impl Default for RuntimeConfigPartial {
    fn default() -> Self {
        Self {
            fee_rate: None,
            rent_extraction_enabled: None,
            scheduler_interval_secs: None,
            min_extraction_amount_shannons: None,
            auto_match_enabled: None,
            auto_match_min_capacity: None,
            auto_match_max_escrow_blocks: None,
            auto_match_interval_secs: None,
        }
    }
}
