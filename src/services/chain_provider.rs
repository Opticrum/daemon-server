//! Chain provider — abstraction over CKB RPC and indexer.
//!
//! The `ChainProvider` trait allows the service layer to interact with
//! the CKB chain without depending on a specific RPC implementation.
//! `MockChainProvider` is used in tests; `RealChainProvider` in production.

use async_trait::async_trait;
use std::sync::Mutex;

use opticrum_calculator::types::{MatchInfo, OrderInfo};

use crate::error::AppError;

/// Chain provider trait — abstracts CKB RPC calls.
///
/// All methods are async and return `Result<T, AppError>` so they
/// compose cleanly with the service layer's error handling.
#[async_trait]
pub trait ChainProvider: Send + Sync {
    /// Get current tip block number.
    async fn get_tip_block_number(&self) -> Result<u64, AppError>;

    /// Scan all live Order cells on chain.
    async fn scan_orders(&self) -> Result<Vec<OrderInfo>, AppError>;

    /// Scan all live Match cells on chain.
    async fn scan_matches(&self) -> Result<Vec<MatchInfo>, AppError>;

    /// Submit a signed transaction. Returns the tx_hash (32 bytes as hex).
    async fn send_transaction(&self, tx_hex: &str) -> Result<String, AppError>;

    /// Get a live cell's output by outpoint.
    async fn get_cell(&self, tx_hash: &str, index: u32) -> Result<CellOutput, AppError>;

    /// Scan Fiber network for channels owned by the given lock hash.
    /// Returns channel outpoints with capacities for the admin panel's
    /// channel browser and auto-match engine.
    async fn scan_fiber_channels(
        &self,
        owner_lock_hash: &[u8],
    ) -> Result<Vec<FiberChannelInfo>, AppError> {
        // Default: no-op. RealChainProvider overrides this.
        let _ = owner_lock_hash;
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Lightweight chain types (avoid heavy CKB type deps in trait)
// ---------------------------------------------------------------------------

/// Lightweight cell output info returned by the chain provider.
#[derive(Clone, Debug, PartialEq)]
pub struct CellOutput {
    pub capacity: u64,
    pub lock_hash: [u8; 32],
    pub type_hash: Option<[u8; 32]>,
    pub data: Vec<u8>,
}

/// Lightweight Fiber channel info returned by `scan_fiber_channels`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct FiberChannelInfo {
    /// Channel cell tx_hash (hex).
    pub tx_hash: String,
    /// Channel cell output index.
    pub output_index: u32,
    /// Channel capacity in shannons.
    pub capacity: u64,
    /// Channel status: "open", "closing", "closed".
    pub status: String,
    /// Counterparty lock hash (hex).
    pub counterparty_lock_hash: String,
}

// OrderInfo and MatchInfo are imported from opticrum_calculator —
// the contract kernel is the single source of truth for protocol types.

// ---------------------------------------------------------------------------
// Mock chain provider (for tests)
// ---------------------------------------------------------------------------

/// Mock chain provider for unit tests.
///
/// Holds in-memory state: configurable tip block, pre-loaded orders/matches,
/// and a record of submitted transactions.
pub struct MockChainProvider {
    pub tip_block: Mutex<u64>,
    pub orders: Mutex<Vec<OrderInfo>>,
    pub matches: Mutex<Vec<MatchInfo>>,
    pub submitted_txs: Mutex<Vec<String>>,
    pub cells: Mutex<std::collections::HashMap<(String, u32), CellOutput>>,
    pub fiber_channels: Mutex<Vec<FiberChannelInfo>>,
}

impl Default for MockChainProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockChainProvider {
    pub fn new() -> Self {
        Self {
            tip_block: Mutex::new(1000),
            orders: Mutex::new(Vec::new()),
            matches: Mutex::new(Vec::new()),
            submitted_txs: Mutex::new(Vec::new()),
            cells: Mutex::new(std::collections::HashMap::new()),
            fiber_channels: Mutex::new(Vec::new()),
        }
    }

    pub fn with_orders(orders: Vec<OrderInfo>) -> Self {
        Self {
            orders: Mutex::new(orders),
            ..Self::new()
        }
    }

    pub fn with_matches(matches: Vec<MatchInfo>) -> Self {
        Self {
            matches: Mutex::new(matches),
            ..Self::new()
        }
    }

    pub fn set_tip_block(&self, block: u64) {
        *self.tip_block.lock().unwrap() = block;
    }

    pub fn add_cell(&self, tx_hash: &str, index: u32, cell: CellOutput) {
        self.cells
            .lock()
            .unwrap()
            .insert((tx_hash.to_string(), index), cell);
    }

    pub fn add_fiber_channel(&self, channel: FiberChannelInfo) {
        self.fiber_channels.lock().unwrap().push(channel);
    }

    pub fn with_fiber_channels(channels: Vec<FiberChannelInfo>) -> Self {
        Self {
            fiber_channels: Mutex::new(channels),
            ..Self::new()
        }
    }
}

#[async_trait]
impl ChainProvider for MockChainProvider {
    async fn get_tip_block_number(&self) -> Result<u64, AppError> {
        Ok(*self.tip_block.lock().unwrap())
    }

    async fn scan_orders(&self) -> Result<Vec<OrderInfo>, AppError> {
        Ok(self.orders.lock().unwrap().clone())
    }

    async fn scan_matches(&self) -> Result<Vec<MatchInfo>, AppError> {
        Ok(self.matches.lock().unwrap().clone())
    }

    async fn send_transaction(&self, tx_hex: &str) -> Result<String, AppError> {
        use std::hash::{Hash, Hasher};
        self.submitted_txs.lock().unwrap().push(tx_hex.to_string());
        // Generate a deterministic 64-char hex tx hash from the input
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        tx_hex.hash(&mut hasher);
        let h = hasher.finish();
        Ok(format!("{:064x}", h))
    }

    async fn get_cell(&self, tx_hash: &str, index: u32) -> Result<CellOutput, AppError> {
        self.cells
            .lock()
            .unwrap()
            .get(&(tx_hash.to_string(), index))
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Cell {tx_hash}:{index} not found")))
    }

    async fn scan_fiber_channels(
        &self,
        _owner_lock_hash: &[u8],
    ) -> Result<Vec<FiberChannelInfo>, AppError> {
        Ok(self.fiber_channels.lock().unwrap().clone())
    }
}
