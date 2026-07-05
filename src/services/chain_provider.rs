//! Chain provider — abstraction over CKB RPC and indexer.
//!
//! The `ChainProvider` trait allows the service layer to interact with
//! the CKB chain without depending on a specific RPC implementation.
//! `MockChainProvider` is used in tests; `RealChainProvider` in production.

use async_trait::async_trait;
use std::collections::HashMap;
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

    /// Get the CKB network this provider is connected to ("testnet" or "mainnet").
    /// Defaults to "testnet" — production implementations should override this.
    fn network(&self) -> &str {
        "testnet"
    }

    /// Query the Fiber node for its metadata (version, pubkey, peers, etc.).
    /// Calls the `node_info` JSON-RPC method on the Fiber RPC endpoint.
    async fn get_fiber_node_info(&self) -> Result<Option<FiberNodeInfo>, AppError> {
        // Default: no-op. RealChainProvider overrides this.
        Ok(None)
    }

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

    /// Shut down a Fiber channel by its channel ID.
    /// `force=false` attempts a cooperative close; `force=true` does a
    /// unilateral close. Default: no-op (MockChainProvider records the call).
    async fn shutdown_channel(&self, channel_id: &str, force: bool) -> Result<(), AppError> {
        let _ = (channel_id, force);
        Ok(())
    }

    /// Open a new Fiber channel to a peer.
    /// Returns the temporary channel ID (hex-encoded Hash256).
    /// `address` is an optional multiaddr for direct peer dialing.
    /// Default: returns a mock ID (MockChainProvider records the call).
    async fn open_channel(
        &self,
        peer_pubkey: &str,
        funding_amount: u64,
        address: Option<&str>,
    ) -> Result<String, AppError> {
        let _ = (peer_pubkey, funding_amount, address);
        Ok("mock_temporary_channel_id".into())
    }

    /// List all connected Fiber peers.
    async fn list_peers(&self) -> Result<Vec<PeerInfo>, AppError> {
        Ok(vec![])
    }

    /// Connect to a Fiber peer.
    /// `address` is an optional multiaddr for direct peer dialing — when
    /// provided, the Fiber node can dial the peer directly instead of
    /// relying on DHT discovery.
    async fn connect_peer(&self, pubkey: &str, address: Option<&str>) -> Result<(), AppError> {
        let _ = (pubkey, address);
        Ok(())
    }

    /// Get the block number where a transaction was confirmed.
    /// Returns 0 if the tx is not found or not yet confirmed.
    async fn get_tx_block_number(&self, _tx_hash: &str) -> Result<u64, AppError> {
        Ok(0)
    }

    /// Get the timestamp (Unix milliseconds) of a block by its number.
    /// Returns 0 if the block is not found.
    async fn get_block_timestamp(&self, _block_number: u64) -> Result<u64, AppError> {
        Ok(0)
    }

    /// Get full transaction data from CKB RPC.
    ///
    /// Returns the transaction hex, block number, and metadata.
    /// Used for extraction backtracking — walking the transaction graph
    /// to reconstruct extraction history from on-chain data.
    async fn get_transaction(&self, _tx_hash: &str) -> Result<TransactionInfo, AppError> {
        Err(AppError::ChainError(
            "get_transaction not implemented".into(),
        ))
    }

    /// Query live cells locked by a given lock hash.
    /// Returns the cell outputs with their capacities.
    /// Default: no-op (MockChainProvider overrides with in-memory filter,
    /// RealChainProvider queries the CKB indexer).
    async fn get_cells_by_lock(&self, _lock_hash: &[u8; 32]) -> Result<Vec<CellOutput>, AppError> {
        Ok(Vec::new())
    }

    /// Query live cells for a secp256k1_blake160 lock script (by lock args).
    async fn get_cells_by_lock_arg(
        &self,
        _lock_arg: &[u8; 20],
    ) -> Result<Vec<CellOutput>, AppError> {
        Ok(Vec::new())
    }

    /// Get total CKB balance for a lock hash (sum of live cell capacities).
    async fn get_balance(&self, lock_hash: &[u8; 32]) -> Result<u64, AppError> {
        let cells = self.get_cells_by_lock(lock_hash).await?;
        Ok(cells.iter().map(|c| c.capacity).sum())
    }

    /// Get total CKB balance for a CKB address (preferred — queries indexer by lock args).
    async fn get_balance_by_address(&self, address: &str) -> Result<u64, AppError> {
        use crate::services::address::{lock_arg_from_address, script_lock_hash};
        let lock_arg = lock_arg_from_address(address)?;
        let lock_hash = script_lock_hash(&lock_arg);
        let cells = self.get_cells_by_lock_arg(&lock_arg).await?;
        if cells.is_empty() {
            // Fallback for providers that only implement lock_hash lookup.
            let fallback = self.get_cells_by_lock(&lock_hash).await?;
            Ok(fallback.iter().map(|c| c.capacity).sum())
        } else {
            Ok(cells.iter().map(|c| c.capacity).sum())
        }
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
///
/// Fields are extracted from the Fiber node's `list_channels` JSON-RPC response
/// (deserialized via `fiber_json_types::channel::Channel`).
#[derive(Clone, Debug, serde::Serialize)]
pub struct FiberChannelInfo {
    /// Channel ID (hex, 32 bytes).
    pub channel_id: String,
    /// Counterparty's Fiber public key (hex, 33 bytes compressed pubkey).
    pub counterparty_fiber_key: String,
    /// Channel funding outpoint tx_hash (hex).
    pub tx_hash: String,
    /// Channel funding outpoint output index.
    pub output_index: u32,
    /// Total channel capacity in shannons (local_balance + remote_balance).
    pub capacity: u64,
    /// Local balance in shannons.
    pub local_balance: u64,
    /// Remote balance in shannons.
    pub remote_balance: u64,
    /// Channel state name (e.g. "ChannelReady", "ShuttingDown", "Closed").
    pub state_name: String,
    /// Whether the channel is public (announced to network).
    pub is_public: bool,
    /// Whether the channel is enabled for routing.
    pub enabled: bool,
    /// Channel creation time in Unix milliseconds from the Fiber node.
    pub created_at: u64,
}

/// Summary of an on-chain opticrum Match cell linked to a Fiber channel.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ChannelMatchInfo {
    /// Match cell tx_hash (hex).
    pub match_tx_hash: String,
    /// Match cell output index.
    pub match_output_index: u32,
    /// Locked xUDT amount.
    pub xudt_amount: u128,
    /// Per-block rent rate in shannons.
    pub shannons_per_block: u64,
    /// Last extraction block number.
    pub last_extraction_block: u64,
    /// CKB capacity of the match cell in shannons.
    pub ckb_capacity: u64,
    /// Seller lock hash (hex).
    pub seller_lock_hash: String,
}

/// A Fiber channel with its associated on-chain opticrum match cell (if found).
#[derive(Clone, Debug, serde::Serialize)]
pub struct ChannelWithMatch {
    #[serde(flatten)]
    pub channel: FiberChannelInfo,
    /// The matched opticrum Match cell, if found on chain.
    pub match_info: Option<ChannelMatchInfo>,
    /// "matched" or "not_found".
    pub match_status: String,
    /// Fiber network address of the counterparty, from the on-chain order (if any).
    pub fiber_address: Option<String>,
}

/// Lightweight connected peer info returned by `list_peers`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct PeerInfo {
    /// Peer identity pubkey (hex, 33 bytes).
    pub pubkey: String,
    /// Multiaddr used for the connection.
    pub address: String,
}

/// Fiber node metadata returned by the `node_info` JSON-RPC method.
/// Fields mirror the Fiber node's snake_case response.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FiberNodeInfo {
    pub version: String,
    pub commit_hash: String,
    pub pubkey: String,
    #[serde(default)]
    pub node_name: Option<String>,
    pub addresses: Vec<String>,
    pub chain_hash: String,
    pub channel_count: String,
    pub pending_channel_count: String,
    pub peers_count: String,
    pub tlc_expiry_delta: String,
    pub tlc_min_value: String,
    #[serde(default)]
    pub udt_cfg_infos: Vec<serde_json::Value>,
}

/// An input reference in a CKB transaction — points to a previous output.
#[derive(Clone, Debug)]
pub struct TxInputInfo {
    /// Hex-encoded tx_hash of the previous transaction.
    pub previous_tx_hash: String,
    /// Output index in the previous transaction.
    pub previous_index: u32,
}

/// An output in a CKB transaction with its lock script and data.
#[derive(Clone, Debug)]
pub struct TxOutputInfo {
    /// Capacity in shannons.
    pub capacity: u64,
    /// Hex-encoded lock script code_hash (H256).
    pub lock_code_hash: String,
    /// Lock script hash type: "Type", "Data", "Data1", or "Data2".
    pub lock_hash_type: String,
    /// Hex-encoded lock script args bytes.
    pub lock_args_hex: String,
    /// Byte length of lock args (65 = Order cell, 133 = Match cell).
    pub lock_args_len: usize,
    /// Hex-encoded cell data (e.g. MatchData for match cells).
    pub data_hex: String,
}

/// Full transaction data retrieved from the CKB chain.
///
/// Used for extraction backtracking — walking the transaction graph
/// to trace how a match cell evolved through multiple rent extractions.
#[derive(Clone, Debug)]
pub struct TransactionInfo {
    /// Transaction hash (hex-encoded, 64 chars).
    pub tx_hash: String,
    /// Block number where the transaction was confirmed, or 0 if pending.
    pub block_number: u64,
    /// Consumed cell references (inputs).
    pub inputs: Vec<TxInputInfo>,
    /// Created cells with lock scripts and data (outputs).
    pub outputs: Vec<TxOutputInfo>,
}

impl TransactionInfo {
    /// Construct a minimal TransactionInfo from tx_hex for backward
    /// compatibility with callers that only have raw hex.
    pub fn from_hex(tx_hash: String, block_number: u64, _tx_hex: &str) -> Self {
        Self {
            tx_hash,
            block_number,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
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
    pub channel_matches: Mutex<Vec<ChannelWithMatch>>,
    pub fiber_node_info: Mutex<Option<FiberNodeInfo>>,
    pub shutdown_channels: Mutex<Vec<(String, bool)>>,
    pub open_channels: Mutex<Vec<(String, u64, Option<String>)>>,
    pub peer_list: Mutex<Vec<PeerInfo>>,
    pub peer_connections: Mutex<Vec<(String, Option<String>)>>,
    pub transactions: Mutex<HashMap<String, TransactionInfo>>,
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
            channel_matches: Mutex::new(Vec::new()),
            fiber_node_info: Mutex::new(None),
            shutdown_channels: Mutex::new(Vec::new()),
            open_channels: Mutex::new(Vec::new()),
            peer_list: Mutex::new(Vec::new()),
            peer_connections: Mutex::new(Vec::new()),
            transactions: Mutex::new(HashMap::new()),
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

    pub fn add_channel_with_match(&self, cwm: ChannelWithMatch) {
        self.channel_matches.lock().unwrap().push(cwm);
    }

    pub fn with_fiber_channels(channels: Vec<FiberChannelInfo>) -> Self {
        Self {
            fiber_channels: Mutex::new(channels),
            ..Self::new()
        }
    }

    pub fn add_transaction(&self, tx_hash: &str, info: TransactionInfo) {
        self.transactions
            .lock()
            .unwrap()
            .insert(tx_hash.to_string(), info);
    }

    pub fn with_channel_matches(cwms: Vec<ChannelWithMatch>) -> Self {
        Self {
            channel_matches: Mutex::new(cwms),
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

    async fn get_fiber_node_info(&self) -> Result<Option<FiberNodeInfo>, AppError> {
        Ok(self.fiber_node_info.lock().unwrap().clone())
    }

    async fn scan_fiber_channels(
        &self,
        _owner_lock_hash: &[u8],
    ) -> Result<Vec<FiberChannelInfo>, AppError> {
        Ok(self.fiber_channels.lock().unwrap().clone())
    }

    async fn shutdown_channel(&self, channel_id: &str, force: bool) -> Result<(), AppError> {
        self.shutdown_channels
            .lock()
            .unwrap()
            .push((channel_id.to_string(), force));
        Ok(())
    }

    async fn open_channel(
        &self,
        peer_pubkey: &str,
        funding_amount: u64,
        address: Option<&str>,
    ) -> Result<String, AppError> {
        self.open_channels
            .lock()
            .unwrap()
            .push((peer_pubkey.to_string(), funding_amount, address.map(|a| a.to_string())));
        Ok("mock_temporary_channel_id".into())
    }

    async fn list_peers(&self) -> Result<Vec<PeerInfo>, AppError> {
        Ok(self.peer_list.lock().unwrap().clone())
    }

    async fn connect_peer(&self, pubkey: &str, address: Option<&str>) -> Result<(), AppError> {
        self.peer_connections
            .lock()
            .unwrap()
            .push((pubkey.to_string(), address.map(|a| a.to_string())));
        Ok(())
    }

    async fn get_cells_by_lock(&self, lock_hash: &[u8; 32]) -> Result<Vec<CellOutput>, AppError> {
        Ok(self
            .cells
            .lock()
            .unwrap()
            .values()
            .filter(|c| &c.lock_hash == lock_hash)
            .cloned()
            .collect())
    }

    async fn get_cells_by_lock_arg(
        &self,
        lock_arg: &[u8; 20],
    ) -> Result<Vec<CellOutput>, AppError> {
        use crate::services::address::script_lock_hash;
        let lock_hash = script_lock_hash(lock_arg);
        self.get_cells_by_lock(&lock_hash).await
    }

    async fn get_transaction(&self, tx_hash: &str) -> Result<TransactionInfo, AppError> {
        self.transactions
            .lock()
            .unwrap()
            .get(tx_hash)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Transaction {tx_hash} not found")))
    }
}
