//! Real chain provider — production implementation of `ChainProvider`.
//!
//! Wraps `ckb_cinnabar_calculator::rpc::RpcClient` to provide real CKB RPC
//! and indexer access. Delegates order/match scanning to the
//! `opticrum_calculator::reader` functions.
//!
//! Fiber node communication uses the vendored `crate::fiber::rpc_client::RpcClient`
//! (from fiber-cli) with request/response types from `fiber-json-types`.

use async_trait::async_trait;
use ckb_cinnabar_calculator::rpc::{RpcClient, RPC};
use molecule::prelude::Entity;
use opticrum_calculator::reader::{scan_matches, scan_orders};
use opticrum_calculator::types::{MatchInfo, OrderInfo};
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::fiber::rpc_client::FiberRpcExt;
use crate::services::chain_provider::{
    CellOutput, ChainProvider, FiberChannelInfo, FiberNodeInfo, PeerInfo, TransactionInfo,
};
use fiber_json_types::channel::{ListChannelsResult, OpenChannelResult};
use opticrum_protocol::OutPoint as ProtocolOutPoint;

/// Production chain provider backed by a real CKB RPC node and indexer.
pub struct RealChainProvider {
    rpc: RpcClient,
    fiber_rpc: crate::fiber::rpc_client::RpcClient,
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
    /// RPC URL. The resolved `Network` is passed to `opticrum_calculator`
    /// reader functions so they never encounter `Network::Custom(url)`.
    pub fn new(ckb_rpc_url: &str, ckb_indexer_url: &str, fiber_rpc_url: &str) -> Self {
        let rpc = RpcClient::new(ckb_rpc_url, Some(ckb_indexer_url));
        let network = Self::detect_network(ckb_rpc_url);

        let fiber_rpc = crate::fiber::rpc_client::RpcClient::new(fiber_rpc_url, false, None)
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to create Fiber RPC client for '{}': {} — using fallback",
                    fiber_rpc_url,
                    e
                );
                crate::fiber::rpc_client::RpcClient::new("http://localhost:8227", false, None)
                    .expect("localhost fallback RPC client")
            });

        tracing::info!(
            "RealChainProvider: rpc={}, idx={}, fiber={}, network={}",
            ckb_rpc_url,
            ckb_indexer_url,
            fiber_rpc.url(),
            network
        );

        Self {
            rpc,
            fiber_rpc,
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

    /// Update the RPC client's network from on-chain `get_blockchain_info`.
    /// Resolves `Network::Custom(url)` to `Mainnet` or `Testnet`.
    pub async fn update_network(&mut self) -> Result<(), AppError> {
        self.rpc
            .update_network()
            .await
            .map_err(|e| AppError::ChainError(format!("update_network: {}", e)))
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

    /// Extract the state name string from a `ChannelState` variant.
    fn channel_state_name(state: &fiber_json_types::channel::ChannelState) -> String {
        use fiber_json_types::channel::ChannelState;
        match state {
            ChannelState::NegotiatingFunding(_) => "NegotiatingFunding",
            ChannelState::CollaboratingFundingTx(_) => "CollaboratingFundingTx",
            ChannelState::SigningCommitment(_) => "SigningCommitment",
            ChannelState::AwaitingTxSignatures(_) => "AwaitingTxSignatures",
            ChannelState::AwaitingChannelReady(_) => "AwaitingChannelReady",
            ChannelState::ChannelReady => "ChannelReady",
            ChannelState::ShuttingDown(_) => "ShuttingDown",
            ChannelState::Closed(_) => "Closed",
        }
        .to_string()
    }
}

#[async_trait]
impl ChainProvider for RealChainProvider {
    fn network(&self) -> &str {
        &self.network
    }

    async fn get_tip_block_number(&self) -> Result<u64, AppError> {
        self.rpc
            .get_tip_block_number()
            .await
            .map(u64::from)
            .map_err(Self::map_err)
    }

    async fn scan_orders(&self) -> Result<Vec<OrderInfo>, AppError> {
        scan_orders(&self.rpc, None).await.map_err(Self::map_err)
    }

    async fn scan_matches(&self) -> Result<Vec<MatchInfo>, AppError> {
        scan_matches(&self.rpc, None).await.map_err(Self::map_err)
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
            || tx_hex.starts_with("auto_match:")
        {
            tracing::debug!(
                "Placeholder tx (Phase 6): {}",
                &tx_hex[..tx_hex.len().min(80)]
            );
            return Ok(hex::encode(Self::hash_bytes(tx_hex.as_bytes())));
        }

        // Real transaction path (Phase 6+): decode hex and broadcast via RPC.
        Err(AppError::ChainError(
            "Real transaction broadcast requires Phase 6 assembly wiring. \
             Currently only placeholder transactions are supported."
                .into(),
        ))
    }

    async fn get_cell(&self, tx_hash: &str, index: u32) -> Result<CellOutput, AppError> {
        tracing::debug!("get_cell({tx_hash}, {index}) — RPC query deferred to Phase 6");
        Err(AppError::ChainError(
            "Cell query not yet wired for RPC (Phase 6). \
             Use MockChainProvider::add_cell() for test setups."
                .to_string(),
        ))
    }

    async fn get_cells_by_lock_arg(
        &self,
        lock_arg: &[u8; 20],
    ) -> Result<Vec<CellOutput>, AppError> {
        use crate::services::address::{script_lock_hash, secp256k1_blake160_lock_script};
        use ckb_cinnabar_calculator::indexer::{ScriptType, SearchKey};
        use ckb_cinnabar_calculator::re_exports::ckb_jsonrpc_types::JsonBytes;
        use ckb_cinnabar_calculator::rpc::RPC;

        let script: ckb_cinnabar_calculator::re_exports::ckb_jsonrpc_types::Script =
            serde_json::from_value(secp256k1_blake160_lock_script(lock_arg))
                .map_err(|e| AppError::ChainError(format!("Build lock script for indexer: {e}")))?;

        let search_key = SearchKey {
            script,
            script_type: ScriptType::Lock,
            script_search_mode: None,
            filter: None,
            with_data: None,
            group_by_transaction: None,
        };

        let lock_hash = script_lock_hash(lock_arg);
        let mut cells = Vec::new();
        let mut cursor: Option<JsonBytes> = None;

        loop {
            let page = self
                .rpc
                .get_cells(search_key.clone(), 1000, cursor.clone())
                .await
                .map_err(Self::map_err)?;

            if page.objects.is_empty() {
                break;
            }

            for cell in page.objects {
                let capacity = cell.output.capacity.value();
                cells.push(CellOutput {
                    capacity,
                    lock_hash,
                    type_hash: None,
                    data: vec![],
                });
            }

            let next = page.last_cursor;
            if next.as_bytes().is_empty() || Some(next.clone()) == cursor {
                break;
            }
            cursor = Some(next);
        }

        tracing::debug!(
            lock_arg = %hex::encode(lock_arg),
            count = cells.len(),
            total = cells.iter().map(|c| c.capacity).sum::<u64>(),
            "Indexer cells fetched"
        );

        Ok(cells)
    }

    async fn get_cells_by_lock(&self, lock_hash: &[u8; 32]) -> Result<Vec<CellOutput>, AppError> {
        // Without lock args we cannot query the indexer efficiently; callers should
        // prefer get_cells_by_lock_arg / get_balance_by_address.
        tracing::debug!(
            lock_hash = %hex::encode(lock_hash),
            "get_cells_by_lock called without lock args — returning empty"
        );
        Ok(Vec::new())
    }

    async fn get_fiber_node_info(&self) -> Result<Option<FiberNodeInfo>, AppError> {
        tracing::debug!("Fetching Fiber node_info from {}", self.fiber_rpc.url());

        // Use fiber-json-types for deserialization (no type duplication).
        match self
            .fiber_rpc
            .call_fiber_no_params::<fiber_json_types::info::NodeInfoResult>("node_info")
            .await
        {
            Ok(info) => {
                // Map to our API DTO (frontend contract, not protocol duplication).
                Ok(Some(FiberNodeInfo {
                    version: info.version,
                    commit_hash: info.commit_hash,
                    pubkey: info.pubkey.to_string(),
                    node_name: info.node_name,
                    addresses: info.addresses,
                    chain_hash: info.chain_hash.to_string(),
                    channel_count: format!("0x{:x}", info.channel_count),
                    pending_channel_count: format!("0x{:x}", info.pending_channel_count),
                    peers_count: format!("0x{:x}", info.peers_count),
                    tlc_expiry_delta: format!("0x{:x}", info.tlc_expiry_delta),
                    tlc_min_value: format!("0x{:x}", info.tlc_min_value),
                    udt_cfg_infos: serde_json::to_value(&info.udt_cfg_infos)
                        .unwrap_or_default()
                        .as_array()
                        .cloned()
                        .unwrap_or_default(),
                }))
            }
            Err(e) => {
                tracing::warn!("Fiber node_info failed: {}", e);
                Ok(None)
            }
        }
    }

    async fn scan_fiber_channels(
        &self,
        _owner_lock_hash: &[u8],
    ) -> Result<Vec<FiberChannelInfo>, AppError> {
        let url = self.fiber_rpc.url();
        tracing::info!("Calling Fiber list_channels at {}", url);

        // Build params manually as raw JSON to avoid sending null fields
        // (serde serializes Option::None as null, which some Fiber node
        // versions reject).
        let params = serde_json::json!({
            "include_closed": true
        });

        // Use call_typed_with_values for raw JSON params, then deserialize
        // result into fiber_json_types types.
        let value = self
            .fiber_rpc
            .call("list_channels", vec![params])
            .await
            .map_err(|e| AppError::ChainError(format!("Fiber RPC list_channels: {}", e)))?;

        let result: ListChannelsResult = serde_json::from_value(value)
            .map_err(|e| AppError::ChainError(format!("Fiber RPC list_channels parse: {}", e)))?;

        let channels: Vec<FiberChannelInfo> = result
            .channels
            .into_iter()
            .map(|ch| {
                // Parse channel_outpoint from EntityHex (36-byte molecule OutPoint).
                // If missing or unparseable, use empty/default values.
                let (tx_hash, output_index) = ch
                    .channel_outpoint
                    .and_then(|op| match ProtocolOutPoint::from_slice(op.as_slice()) {
                        Ok(outpoint) => Some((hex::encode(outpoint.tx_hash), outpoint.index)),
                        Err(e) => {
                            tracing::debug!("Failed to parse channel outpoint: {}", e);
                            None
                        }
                    })
                    .unwrap_or_default();

                FiberChannelInfo {
                    channel_id: ch
                        .channel_id
                        .to_string()
                        .trim_start_matches("0x")
                        .to_string(),
                    counterparty_fiber_key: ch.pubkey.to_string(),
                    tx_hash,
                    output_index,
                    capacity: (ch.local_balance + ch.remote_balance) as u64,
                    local_balance: ch.local_balance as u64,
                    remote_balance: ch.remote_balance as u64,
                    state_name: Self::channel_state_name(&ch.state),
                    is_public: ch.is_public,
                    enabled: ch.enabled,
                    created_at: ch.created_at,
                }
            })
            .collect();

        tracing::info!(count = channels.len(), "Fiber channels listed");
        Ok(channels)
    }

    async fn shutdown_channel(&self, channel_id: &str, force: bool) -> Result<(), AppError> {
        tracing::info!(
            channel_id = %channel_id,
            force,
            "Calling Fiber shutdown_channel"
        );
        let params = serde_json::json!({
            "channel_id": channel_id,
            "force": force,
        });
        self.fiber_rpc
            .call("shutdown_channel", vec![params])
            .await
            .map_err(|e| AppError::ChainError(format!("Fiber RPC shutdown_channel: {}", e)))?;
        tracing::info!(channel_id = %channel_id, "Fiber channel shutdown initiated");
        Ok(())
    }

    async fn open_channel(
        &self,
        peer_pubkey: &str,
        funding_amount: u64,
    ) -> Result<String, AppError> {
        tracing::info!(
            peer = %peer_pubkey,
            amount = funding_amount,
            "Calling Fiber open_channel"
        );
        let params = serde_json::json!({
            "pubkey": peer_pubkey,
            "funding_amount": format!("0x{:x}", funding_amount),
        });
        let value = self
            .fiber_rpc
            .call("open_channel", vec![params])
            .await
            .map_err(|e| AppError::ChainError(format!("Fiber RPC open_channel: {}", e)))?;
        let result: OpenChannelResult = serde_json::from_value(value)
            .map_err(|e| AppError::ChainError(format!("open_channel parse: {}", e)))?;
        let id = hex::encode(result.temporary_channel_id.as_bytes());
        tracing::info!(temp_id = %id, "Fiber channel open initiated");
        Ok(id)
    }

    async fn list_peers(&self) -> Result<Vec<PeerInfo>, AppError> {
        let result: fiber_json_types::peer::ListPeersResult =
            self.fiber_rpc.call_fiber_no_params("list_peers").await?;
        Ok(result
            .peers
            .into_iter()
            .map(|p| PeerInfo {
                pubkey: p.pubkey.to_string(),
                address: p.address,
            })
            .collect())
    }

    async fn get_tx_block_number(&self, tx_hash: &str) -> Result<u64, AppError> {
        use ckb_cinnabar_calculator::rpc::RPC;
        let hash: ckb_cinnabar_calculator::re_exports::ckb_types::H256 = tx_hash
            .parse()
            .map_err(|_| AppError::ChainError("invalid tx hash".into()))?;
        match self.rpc.get_transaction(&hash).await {
            Ok(Some(tx)) => {
                let block = tx.tx_status.block_number.unwrap_or_default();
                Ok(block.value())
            }
            _ => Ok(0),
        }
    }

    async fn get_block_timestamp(&self, block_number: u64) -> Result<u64, AppError> {
        use ckb_cinnabar_calculator::rpc::RPC;
        let number: ckb_cinnabar_calculator::re_exports::ckb_jsonrpc_types::Uint64 =
            block_number.into();
        match self.rpc.get_block_by_number(number).await {
            Ok(Some(block)) => Ok(block.header.inner.timestamp.value()),
            _ => Ok(0),
        }
    }

    async fn get_transaction(&self, tx_hash: &str) -> Result<TransactionInfo, AppError> {
        use crate::services::chain_provider::{TxInputInfo, TxOutputInfo};
        use ckb_cinnabar_calculator::re_exports::ckb_jsonrpc_types::Either;
        use ckb_cinnabar_calculator::rpc::RPC;

        let hash: ckb_cinnabar_calculator::re_exports::ckb_types::H256 = tx_hash
            .parse()
            .map_err(|_| AppError::ChainError("invalid tx hash".into()))?;
        match self
            .rpc
            .get_transaction(&hash)
            .await
            .map_err(Self::map_err)?
        {
            Some(tx) => {
                let block = tx.tx_status.block_number.unwrap_or_default();

                // Extract structured I/O from the transaction view
                let (inputs, outputs) = tx
                    .transaction
                    .as_ref()
                    .and_then(|rf| match &rf.inner {
                        Either::Left(txv) => Some(&txv.inner),
                        Either::Right(_) => None,
                    })
                    .map(|inner| {
                        let inputs: Vec<TxInputInfo> = inner
                            .inputs
                            .iter()
                            .map(|input| TxInputInfo {
                                previous_tx_hash: hex::encode(
                                    input.previous_output.tx_hash.as_bytes(),
                                ),
                                previous_index: input.previous_output.index.value(),
                            })
                            .collect();

                        let outputs: Vec<TxOutputInfo> = inner
                            .outputs
                            .iter()
                            .enumerate()
                            .map(|(i, output)| {
                                let data = inner
                                    .outputs_data
                                    .get(i)
                                    .map(|d| hex::encode(d.as_bytes()))
                                    .unwrap_or_default();
                                let args_bytes = output.lock.args.as_bytes();
                                TxOutputInfo {
                                    capacity: output.capacity.value(),
                                    lock_code_hash: hex::encode(output.lock.code_hash.as_bytes()),
                                    lock_hash_type: format!("{:?}", output.lock.hash_type),
                                    lock_args_hex: hex::encode(args_bytes),
                                    lock_args_len: args_bytes.len(),
                                    data_hex: data,
                                }
                            })
                            .collect();

                        (inputs, outputs)
                    })
                    .unwrap_or_default();

                Ok(TransactionInfo {
                    tx_hash: tx_hash.to_string(),
                    block_number: block.value(),
                    inputs,
                    outputs,
                })
            }
            None => Err(AppError::NotFound(format!(
                "Transaction {tx_hash} not found"
            ))),
        }
    }

    async fn connect_peer(&self, pubkey: &str) -> Result<(), AppError> {
        tracing::info!(peer = %pubkey, "Calling Fiber connect_peer");
        let params = serde_json::json!({ "pubkey": pubkey });
        self.fiber_rpc
            .call("connect_peer", vec![params])
            .await
            .map_err(|e| AppError::ChainError(format!("Fiber RPC connect_peer: {}", e)))?;
        Ok(())
    }
}
