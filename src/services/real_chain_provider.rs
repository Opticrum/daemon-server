//! Real chain provider — production implementation of `ChainProvider`.
//!
//! Wraps `ckb_cinnabar_calculator::rpc::RpcClient` to provide real CKB RPC
//! and indexer access. Delegates order/match scanning to the
//! `opticrum_calculator::reader` functions.

use async_trait::async_trait;
use ckb_cinnabar_calculator::rpc::{RpcClient, RPC};
use opticrum_calculator::reader::{scan_matches, scan_orders};
use opticrum_calculator::types::{MatchInfo, OrderInfo};
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::services::chain_provider::{CellOutput, ChainProvider, FiberChannelInfo};

/// Production chain provider backed by a real CKB RPC node and indexer.
pub struct RealChainProvider {
    rpc: RpcClient,
    fiber_rpc_url: String,
    network: String,
}

impl RealChainProvider {
    /// Create a new real chain provider.
    ///
    /// `ckb_rpc_url` — CKB JSON-RPC endpoint (e.g. `http://localhost:8114`).
    /// `ckb_indexer_url` — CKB indexer endpoint (e.g. `http://localhost:8116`).
    /// `fiber_rpc_url` — Fiber network node RPC endpoint.
    ///
    /// The CKB network ("testnet" or "mainnet") is auto-detected from the
    /// RPC URL using common naming conventions.
    pub fn new(ckb_rpc_url: &str, ckb_indexer_url: &str, fiber_rpc_url: &str) -> Self {
        let rpc = RpcClient::new(ckb_rpc_url, Some(ckb_indexer_url));
        let network = Self::detect_network(ckb_rpc_url);

        tracing::info!(
            "RealChainProvider: rpc={}, idx={}, fiber={}, network={}",
            ckb_rpc_url,
            ckb_indexer_url,
            fiber_rpc_url,
            network
        );

        Self {
            rpc,
            fiber_rpc_url: fiber_rpc_url.to_string(),
            network,
        }
    }

    /// Auto-detect the CKB network from the RPC URL.
    ///
    /// Heuristics (checked in order):
    /// - URL contains "testnet" or "aggron"         → testnet
    /// - Port is 28114 (standard CKB testnet port)  → testnet
    /// - URL contains "mainnet" or "lina"           → mainnet
    /// - Falls back to "testnet" (conservative default — port 8114 is
    ///   used by both mainnet and custom testnet setups)
    fn detect_network(rpc_url: &str) -> String {
        let lower = rpc_url.to_lowercase();

        // Explicit testnet indicators
        if lower.contains("testnet") || lower.contains("aggron") || lower.contains(":28114") {
            return "testnet".into();
        }

        // Explicit mainnet indicators
        if lower.contains("mainnet") || lower.contains("lina") {
            return "mainnet".into();
        }

        // Ambiguous — default to testnet for safety. Common case: localhost:8114
        // which could be either. Users with mainnet nodes should use a URL
        // containing "mainnet" (e.g. http://ckb-mainnet.local:8114).
        tracing::info!(
            "Network not obvious from RPC URL '{}' — defaulting to testnet. \
             Add 'mainnet' or 'testnet' to the URL host to disambiguate.",
            rpc_url
        );
        "testnet".into()
    }

    /// Get a reference to the underlying CKB RPC client.
    pub fn rpc_client(&self) -> &RpcClient {
        &self.rpc
    }

    /// The configured CKB network ("testnet" or "mainnet").
    pub fn network(&self) -> &str {
        &self.network
    }

fn map_err(e: impl std::fmt::Display) -> AppError {
        AppError::ChainError(format!("Chain RPC error: {}", e))
    }

    fn hash_bytes(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());
        hash
    }
}

#[async_trait]
impl ChainProvider for RealChainProvider {
    async fn get_tip_block_number(&self) -> Result<u64, AppError> {
        self.rpc
            .get_tip_block_number()
            .await
            .map(|n| u64::from(n))
            .map_err(Self::map_err)
    }

    async fn scan_orders(&self) -> Result<Vec<OrderInfo>, AppError> {
        scan_orders(&self.rpc).await.map_err(Self::map_err)
    }

    async fn scan_matches(&self) -> Result<Vec<MatchInfo>, AppError> {
        scan_matches(&self.rpc).await.map_err(Self::map_err)
    }

    async fn send_transaction(&self, tx_hex: &str) -> Result<String, AppError> {
        // Detect placeholder format strings (current service layer pattern).
        // These will be replaced with real transaction assembly in Phase 6.
        if tx_hex.starts_with("create_order:")
            || tx_hex.starts_with("cancel_order:")
            || tx_hex.starts_with("match_order:")
            || tx_hex.starts_with("extract_rent:")
            || tx_hex.starts_with("destroy_match:")
            || tx_hex.starts_with("auto_extract:")
        {
            tracing::warn!(
                "send_transaction called with placeholder tx (Phase 6 will wire real assembly): {}",
                &tx_hex[..tx_hex.len().min(80)]
            );
            return Ok(hex::encode(Self::hash_bytes(tx_hex.as_bytes())));
        }

        // Real transaction path (Phase 6+): decode hex and broadcast via RPC.
        // The CKB RPC send_transaction takes a JSON Transaction, not hex.
        // For now, return an error indicating this path requires Phase 6 wiring.
        Err(AppError::ChainError(
            "Real transaction broadcast requires Phase 6 assembly wiring. \
             Currently only placeholder transactions are supported."
                .into(),
        ))
    }

    async fn get_cell(&self, tx_hash: &str, index: u32) -> Result<CellOutput, AppError> {
        // In production, this queries the CKB RPC get_live_cell.
        // The RPC type conversion requires ckb_jsonrpc_types which will be
        // wired in Phase 6 alongside real transaction assembly.
        // For now, return a not-found error — the match_service currently
        // uses MockChainProvider-based cell verification in tests.
        tracing::debug!("get_cell({tx_hash}, {index}) — RPC query deferred to Phase 6");
        Err(AppError::ChainError(format!(
            "Cell query not yet wired for RPC (Phase 6). \
             Use MockChainProvider::add_cell() for test setups."
        )))
    }

    async fn scan_fiber_channels(
        &self,
        owner_lock_hash: &[u8],
    ) -> Result<Vec<FiberChannelInfo>, AppError> {
        let _ = owner_lock_hash;
        let _ = &self.fiber_rpc_url;
        tracing::debug!(
            "Fiber channel scan for lock_hash={} (Fiber RPC integration pending)",
            hex::encode(owner_lock_hash)
        );
        Ok(Vec::new())
    }
}
