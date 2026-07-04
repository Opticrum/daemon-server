//! Cached chain provider — transparent cache layer over [`ChainProvider`].
//!
//! Upper layers depend only on `ChainProvider`. When the cache is enabled and
//! populated, scan methods return snapshot data; otherwise they delegate to
//! the inner provider. Mutations and cache refresh always use the inner source.

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use opticrum_calculator::types::{MatchInfo, OrderInfo};
use tracing::warn;

use crate::error::AppError;
use crate::services::chain_cache::SharedChainCache;
use crate::services::chain_provider::{
    CellOutput, ChainProvider, FiberChannelInfo, FiberNodeInfo, PeerInfo, TransactionInfo,
};
use crate::services::rent_service::{walk_extraction_chain, ExtractionChain};
use crate::services::RuntimeConfig;

/// Chain provider that reads scan results from an in-memory cache when enabled.
pub struct CachedChainProvider {
    inner: Arc<dyn ChainProvider>,
    cache: SharedChainCache,
    runtime_config: Arc<RwLock<RuntimeConfig>>,
}

impl CachedChainProvider {
    pub fn new(
        inner: Arc<dyn ChainProvider>,
        cache: SharedChainCache,
        runtime_config: Arc<RwLock<RuntimeConfig>>,
    ) -> Self {
        Self {
            inner,
            cache,
            runtime_config,
        }
    }

    pub fn cache(&self) -> &SharedChainCache {
        &self.cache
    }

    pub fn inner(&self) -> Arc<dyn ChainProvider> {
        self.inner.clone()
    }

    fn cache_enabled(&self) -> bool {
        self.runtime_config.read().unwrap().chain_cache_enabled
    }

    fn use_cache(&self) -> bool {
        self.cache_enabled() && self.cache.is_populated()
    }

    /// Cached on-chain extraction history for a match outpoint (when cache is warm).
    pub fn extraction_chain(&self, tx_hash: &str, output_index: u32) -> Option<ExtractionChain> {
        if self.use_cache() {
            self.cache.snapshot_extraction_chain(tx_hash, output_index)
        } else {
            None
        }
    }

    /// Extraction history from cache when available, otherwise walk the chain live.
    pub async fn get_extraction_chain(&self, match_info: &MatchInfo) -> ExtractionChain {
        let tx_hash = hex::encode(match_info.match_outpoint.tx_hash);
        let output_index = match_info.match_outpoint.index;
        if let Some(chain) = self.extraction_chain(&tx_hash, output_index) {
            return chain;
        }
        walk_extraction_chain(self.inner.as_ref(), match_info)
            .await
            .unwrap_or_default()
    }

    /// Refresh the snapshot from the inner chain provider.
    pub async fn refresh_cache(&self) -> Result<(), AppError> {
        self.cache.refresh(self.inner.as_ref()).await
    }

    /// Fire-and-forget cache refresh after a successful mutation.
    pub fn spawn_cache_refresh(&self) {
        let cache = self.cache.clone();
        let inner = self.inner.clone();
        actix_rt::spawn(async move {
            if let Err(e) = cache.refresh(inner.as_ref()).await {
                warn!(error = %e, "Background chain cache refresh failed");
            }
        });
    }
}

#[async_trait]
impl ChainProvider for CachedChainProvider {
    async fn get_tip_block_number(&self) -> Result<u64, AppError> {
        if self.use_cache() {
            Ok(self.cache.snapshot_tip_block())
        } else {
            self.inner.get_tip_block_number().await
        }
    }

    async fn scan_orders(&self) -> Result<Vec<OrderInfo>, AppError> {
        if self.use_cache() {
            Ok(self.cache.snapshot_orders())
        } else {
            self.inner.scan_orders().await
        }
    }

    async fn scan_matches(&self) -> Result<Vec<MatchInfo>, AppError> {
        if self.use_cache() {
            Ok(self.cache.snapshot_matches())
        } else {
            self.inner.scan_matches().await
        }
    }

    async fn send_transaction(&self, tx_hex: &str) -> Result<String, AppError> {
        self.inner.send_transaction(tx_hex).await
    }

    async fn get_cell(&self, tx_hash: &str, index: u32) -> Result<CellOutput, AppError> {
        self.inner.get_cell(tx_hash, index).await
    }

    fn network(&self) -> &str {
        self.inner.network()
    }

    async fn get_fiber_node_info(&self) -> Result<Option<FiberNodeInfo>, AppError> {
        self.inner.get_fiber_node_info().await
    }

    async fn scan_fiber_channels(
        &self,
        owner_lock_hash: &[u8],
    ) -> Result<Vec<FiberChannelInfo>, AppError> {
        // Fiber channels are always fetched live — not part of the background chain cache.
        self.inner.scan_fiber_channels(owner_lock_hash).await
    }

    async fn shutdown_channel(&self, channel_id: &str, force: bool) -> Result<(), AppError> {
        self.inner.shutdown_channel(channel_id, force).await
    }

    async fn open_channel(
        &self,
        peer_pubkey: &str,
        funding_amount: u64,
    ) -> Result<String, AppError> {
        self.inner.open_channel(peer_pubkey, funding_amount).await
    }

    async fn connect_peer(&self, peer_pubkey: &str) -> Result<(), AppError> {
        self.inner.connect_peer(peer_pubkey).await
    }

    async fn list_peers(&self) -> Result<Vec<PeerInfo>, AppError> {
        self.inner.list_peers().await
    }

    async fn get_tx_block_number(&self, tx_hash: &str) -> Result<u64, AppError> {
        self.inner.get_tx_block_number(tx_hash).await
    }

    async fn get_block_timestamp(&self, block_number: u64) -> Result<u64, AppError> {
        self.inner.get_block_timestamp(block_number).await
    }

    async fn get_transaction(&self, tx_hash: &str) -> Result<TransactionInfo, AppError> {
        self.inner.get_transaction(tx_hash).await
    }

    async fn get_cells_by_lock(&self, lock_hash: &[u8; 32]) -> Result<Vec<CellOutput>, AppError> {
        self.inner.get_cells_by_lock(lock_hash).await
    }

    async fn get_cells_by_lock_arg(
        &self,
        lock_arg: &[u8; 20],
    ) -> Result<Vec<CellOutput>, AppError> {
        self.inner.get_cells_by_lock_arg(lock_arg).await
    }
}
