//! In-memory snapshot store for on-chain Opticrum cell scans.
//!
//! Populated by the chain indexer and manual refresh. Read paths go through
//! [`CachedChainProvider`](crate::services::cached_chain_provider::CachedChainProvider).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use opticrum_calculator::types::{MatchInfo, OrderInfo};
use serde::Serialize;
use tracing::warn;

use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::rent_service::{walk_extraction_chain, ExtractionChain};

/// Outpoint key for extraction-chain lookup: `(tx_hash_hex, output_index)`.
pub type MatchOutpointKey = (String, u32);

/// Snapshot of the latest chain scan results.
#[derive(Debug, Clone, Default)]
pub struct ChainCacheSnapshot {
    pub orders: Vec<OrderInfo>,
    pub matches: Vec<MatchInfo>,
    pub tip_block: u64,
    /// On-chain extraction history per match outpoint (built during refresh).
    pub extraction_chains: HashMap<MatchOutpointKey, ExtractionChain>,
    pub updated_at_ms: u64,
    pub last_error: Option<String>,
}

/// Metadata returned by cache status/refresh endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct ChainCacheStatusResponse {
    pub updated_at_ms: u64,
    pub order_count: u64,
    pub match_count: u64,
    pub channel_count: u64,
    pub tip_block: u64,
    pub extraction_chain_count: u64,
    pub refreshing: bool,
    pub last_error: Option<String>,
}

pub struct ChainCache {
    snapshot: RwLock<ChainCacheSnapshot>,
    refreshing: AtomicBool,
    refresh_lock: tokio::sync::Mutex<()>,
}

pub type SharedChainCache = Arc<ChainCache>;

impl Default for ChainCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ChainCache {
    pub fn new() -> Self {
        Self {
            snapshot: RwLock::new(ChainCacheSnapshot::default()),
            refreshing: AtomicBool::new(false),
            refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Read the snapshot, logging and recovering on RwLock poison.
    fn read_snap(&self) -> std::sync::RwLockReadGuard<'_, ChainCacheSnapshot> {
        self.snapshot.read().unwrap_or_else(|e| {
            tracing::error!(
                "ChainCache lock poisoned on read: {} — recovering with potentially stale data",
                e
            );
            e.into_inner()
        })
    }

    /// Write the snapshot, logging and recovering on RwLock poison.
    fn write_snap(&self) -> std::sync::RwLockWriteGuard<'_, ChainCacheSnapshot> {
        self.snapshot.write().unwrap_or_else(|e| {
            tracing::error!(
                "ChainCache lock poisoned on write: {} — recovering with potentially stale data",
                e
            );
            e.into_inner()
        })
    }

    pub fn is_populated(&self) -> bool {
        self.read_snap().updated_at_ms > 0
    }

    pub fn updated_at_ms(&self) -> u64 {
        self.read_snap().updated_at_ms
    }

    pub fn status(&self) -> ChainCacheStatusResponse {
        let snap = self.read_snap();
        ChainCacheStatusResponse {
            updated_at_ms: snap.updated_at_ms,
            order_count: snap.orders.len() as u64,
            match_count: snap.matches.len() as u64,
            // Fiber channels are fetched live on demand (e.g. Channels page), not cached here.
            channel_count: 0,
            tip_block: snap.tip_block,
            extraction_chain_count: snap.extraction_chains.len() as u64,
            refreshing: self.refreshing.load(Ordering::SeqCst),
            last_error: snap.last_error.clone(),
        }
    }

    pub(crate) fn snapshot_orders(&self) -> Vec<OrderInfo> {
        self.read_snap().orders.clone()
    }

    pub(crate) fn snapshot_matches(&self) -> Vec<MatchInfo> {
        self.read_snap().matches.clone()
    }

    pub(crate) fn snapshot_tip_block(&self) -> u64 {
        self.read_snap().tip_block
    }

    pub(crate) fn snapshot_extraction_chain(
        &self,
        tx_hash: &str,
        output_index: u32,
    ) -> Option<ExtractionChain> {
        let key = (tx_hash.to_string(), output_index);
        self.read_snap().extraction_chains.get(&key).cloned()
    }

    fn match_outpoint_key(m: &MatchInfo) -> MatchOutpointKey {
        (
            hex::encode(m.match_outpoint.tx_hash),
            m.match_outpoint.index,
        )
    }

    /// Refresh snapshot from the **inner** chain provider (never the cached wrapper).
    pub async fn refresh(&self, inner: &dyn ChainProvider) -> Result<(), AppError> {
        let _guard = self.refresh_lock.lock().await;
        self.refreshing.store(true, Ordering::SeqCst);
        let result = self.do_refresh(inner).await;
        self.refreshing.store(false, Ordering::SeqCst);
        result
    }

    async fn do_refresh(&self, inner: &dyn ChainProvider) -> Result<(), AppError> {
        let (orders_result, matches_result, tip_result) = tokio::join!(
            inner.scan_orders(),
            inner.scan_matches(),
            inner.get_tip_block_number(),
        );

        let mut last_error: Option<String> = None;

        let orders = match orders_result {
            Ok(o) => o,
            Err(e) => {
                warn!(error = %e, "Chain cache: scan_orders failed");
                last_error = Some(e.to_string());
                vec![]
            }
        };

        let matches = match matches_result {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "Chain cache: scan_matches failed");
                last_error.get_or_insert_with(|| e.to_string());
                vec![]
            }
        };

        let tip_block = match tip_result {
            Ok(tip) => tip,
            Err(e) => {
                warn!(error = %e, "Chain cache: get_tip_block_number failed");
                last_error.get_or_insert_with(|| e.to_string());
                0
            }
        };

        if last_error.is_some() && orders.is_empty() && matches.is_empty() {
            let mut snap = self.write_snap();
            snap.last_error = last_error;
            return Err(AppError::Internal(
                snap.last_error
                    .clone()
                    .unwrap_or_else(|| "chain cache refresh failed".into()),
            ));
        }

        // Publish scan results immediately so list/dashboard reads are fast;
        // extraction history is filled in a second pass below.
        {
            let mut snap = self.write_snap();
            snap.orders = orders.clone();
            snap.matches = matches.clone();
            snap.tip_block = tip_block;
            snap.updated_at_ms = Self::now_ms();
            snap.last_error = last_error.clone();
        }

        // Walk on-chain extraction history for every live match (background refresh cost).
        let mut extraction_chains = HashMap::with_capacity(matches.len());
        for m in &matches {
            let key = Self::match_outpoint_key(m);
            let chain = walk_extraction_chain(inner, m).await.unwrap_or_default();
            extraction_chains.insert(key, chain);
        }

        let mut snap = self.write_snap();
        snap.extraction_chains = extraction_chains;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::chain_provider::MockChainProvider;

    #[actix_rt::test]
    async fn refresh_populates_snapshot() {
        let cache = ChainCache::new();
        let provider = MockChainProvider::new();
        assert!(!cache.is_populated());

        cache.refresh(&provider).await.unwrap();
        assert!(cache.is_populated());

        let status = cache.status();
        assert!(status.updated_at_ms > 0);
        assert!(!status.refreshing);
    }

    #[actix_rt::test]
    async fn refresh_builds_extraction_chain_index() {
        let cache = ChainCache::new();
        let provider = MockChainProvider::with_matches(vec![]);
        cache.refresh(&provider).await.unwrap();
        assert!(cache.snapshot_extraction_chain("any", 0).is_none());
    }
}
