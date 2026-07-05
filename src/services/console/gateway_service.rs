//! Gateway service — unified aggregation hub for the Web Console.
//!
//! Every method here is a thin facade that delegates to existing services
//! or DB functions. The gateway never duplicates business logic.
//!
//! # Design rules
//! 1. Orchestration — call multiple services for a single response
//! 2. Aggregation — combine data from multiple sources
//! 3. Formatting — compute percentages, trends server-side
//! 4. Consistent errors — all errors go through `AppError`

use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;
use tracing::debug;

use crate::config::Config;
use crate::db::matches as match_db;
use crate::db::unsigned_txs as unsigned_db;
use crate::db::wallets as wallet_db;
use crate::db::DbPool;
use crate::error::AppError;
use crate::services::cached_chain_provider::CachedChainProvider;
use crate::services::chain_provider::{
    ChainProvider, ChannelMatchInfo, ChannelWithMatch, FiberChannelInfo,
};
use crate::services::hd_wallet_signer::HdWalletSigner;
use crate::services::match_service::{self, get_used_channel_outpoints, MatchOrderResult};
use crate::services::rent_service;
use crate::services::runtime_config::{RuntimeConfig, RuntimeConfigPartial};
use crate::services::signer::Signer;
use crate::services::transaction_assembler::TransactionAssembler;
use opticrum_calculator::types::MatchInfo;
use opticrum_protocol::CompressedPubkey;
use std::sync::{Arc, RwLock};

use super::scheduler_state::SchedulerState;

/// Response shape for the aggregated dashboard endpoint.
/// One API call returns everything the Dashboard needs.
#[derive(Serialize, Debug)]
pub struct DashboardResponse {
    // KPI values
    pub total_matches: u64,
    pub live_matches: u64,
    pub exhausted_matches: u64,
    pub destroyed_matches: u64,
    pub total_extracted_shannons: u64,
    pub active_orders_count: u32,
    pub wallet_count: u32,
    pub channel_count: u32,
    pub tip_block: u64,

    // KPI trends (placeholder — backend can extend later with prev-month comparison)
    pub trends: Vec<KpiTrend>,

    // Chart data
    pub extraction_history: Vec<ExtractionPoint>,
    pub match_distribution: Vec<DistributionItem>,

    /// Sparklines (values in CKB for monetary keys).
    pub sparklines: HashMap<String, Vec<f64>>,

    // Scheduler snapshot
    pub scheduler: SchedulerStatusResponse,

    /// Unix ms when the chain cache was last refreshed.
    pub cache_updated_at_ms: u64,
}

#[derive(Serialize, Debug)]
pub struct KpiTrend {
    pub key: String,
    pub current: u64,
    pub previous: u64,
    pub delta_pct: f64,
    pub delta_label: String,
}

#[derive(Serialize, Debug)]
pub struct ExtractionPoint {
    pub date: String,
    /// Extracted amount in CKB (normalized from shannons).
    pub value: f64,
}

#[derive(Serialize, Debug)]
pub struct DistributionItem {
    pub label: String,
    pub value: u64,
    pub color: String,
}

#[derive(Serialize, Debug)]
pub struct SchedulerEventResponse {
    pub id: u64,
    pub ts_ms: u64,
    pub source: String,
    pub level: String,
    pub message: String,
}

#[derive(Serialize, Debug)]
pub struct SchedulerStatusResponse {
    pub extractor: CycleStatus,
    pub matcher: CycleStatus,
    pub indexer: CycleStatus,
    pub tip_block: u64,
    pub latest_event_id: u64,
    pub events: Vec<SchedulerEventResponse>,
}

#[derive(Serialize, Debug)]
pub struct CycleStatus {
    pub last_run: Option<String>,
    pub last_duration_ms: u64,
    pub cycles: u64,
    pub total_processed: u64,
    pub last_processed: u64,
    pub last_error: Option<String>,
}

impl From<&super::scheduler_state::CycleState> for CycleStatus {
    fn from(cs: &super::scheduler_state::CycleState) -> Self {
        Self {
            last_run: cs.last_run.clone(),
            last_duration_ms: cs.last_duration_ms,
            cycles: cs.cycles,
            total_processed: cs.total_processed,
            last_processed: cs.last_processed,
            last_error: cs.last_error.clone(),
        }
    }
}

/// Stateless gateway service.
pub struct GatewayService;

impl GatewayService {
    // ═══════════════════════════════════════════════════════
    // Server info
    // ═══════════════════════════════════════════════════════

    /// Return server metadata: network, RPC endpoints, version, plus
    /// current runtime-config values so the frontend always sees the
    /// effective settings (not just config.toml defaults).
    pub fn get_server_info(
        _config: &Config,
        runtime_config: &RuntimeConfig,
        provider: &dyn ChainProvider,
    ) -> serde_json::Value {
        serde_json::json!({
            "network": provider.network(),
            "ckb_rpc_url": runtime_config.ckb_rpc_url,
            "ckb_indexer_url": runtime_config.ckb_indexer_url,
            "fiber_rpc_url": runtime_config.fiber_rpc_url,
            "fee_rate": runtime_config.fee_rate,
            "version": env!("CARGO_PKG_VERSION"),
        })
    }

    // ═══════════════════════════════════════════════════════
    // Dashboard aggregation
    // ═══════════════════════════════════════════════════════

    pub async fn get_dashboard(
        pool: &DbPool,
        provider: &dyn ChainProvider,
        state: &SchedulerState,
        cache_updated_at_ms: u64,
    ) -> Result<DashboardResponse, AppError> {
        let started = Instant::now();
        let mut conn = pool.get()?;

        let (on_chain_matches, orders, channels, tip_block) = tokio::join!(
            provider.scan_matches(),
            provider.scan_orders(),
            provider.scan_fiber_channels(&[0u8; 32]),
            provider.get_tip_block_number(),
        );
        let on_chain_matches = on_chain_matches.unwrap_or_default();
        let orders = orders.unwrap_or_default();
        let channels = channels.unwrap_or_default();
        let tip_block = tip_block.unwrap_or(0);
        let orders_err =
            on_chain_matches.is_empty() && orders.is_empty() && cache_updated_at_ms == 0;

        let live = on_chain_matches.len() as u64;
        // "exhausted" = match cells with zero capacity remaining
        let exhausted = on_chain_matches
            .iter()
            .filter(|m| m.ckb_capacity == 0)
            .count() as u64;
        let total_matches = on_chain_matches.len() as u64;
        let destroyed: u64 = 0; // destroyed cells are consumed and not in scan_matches()

        let total_extracted = match_db::total_extracted(&mut conn)? as u64;
        let wallets = wallet_db::list_wallets(&mut conn)?;

        debug!(
            matches = total_matches,
            orders = orders.len(),
            channels = channels.len(),
            wallets = wallets.len(),
            tip_block,
            chain_errors = orders_err,
            duration_ms = started.elapsed().as_millis() as u64,
            "Dashboard aggregated"
        );

        // Distribution
        let distribution = vec![
            DistributionItem {
                label: "进行中".into(),
                value: live,
                color: "#52c41a".into(),
            },
            DistributionItem {
                label: "已耗尽".into(),
                value: exhausted,
                color: "#1890ff".into(),
            },
            DistributionItem {
                label: "已销毁".into(),
                value: destroyed,
                color: "#ff4d4f".into(),
            },
        ];

        // Sparklines: real daily-aggregated data from extraction_history
        let sparklines = Self::build_sparklines(&mut conn)?;

        // Trends: current values from live data; deltas show 0 until
        // previous-period tracking is added.
        let trends = vec![
            KpiTrend {
                key: "matches".into(),
                current: total_matches,
                previous: 0,
                delta_pct: 0.0,
                delta_label: "较上月".into(),
            },
            KpiTrend {
                key: "revenue".into(),
                current: total_extracted,
                previous: 0,
                delta_pct: 0.0,
                delta_label: "较上月".into(),
            },
            KpiTrend {
                key: "orders".into(),
                current: orders.len() as u64,
                previous: 0,
                delta_pct: 0.0,
                delta_label: "较上月".into(),
            },
            KpiTrend {
                key: "channels".into(),
                current: channels.len() as u64,
                previous: 0,
                delta_pct: 0.0,
                delta_label: "较上月".into(),
            },
            KpiTrend {
                key: "extracted".into(),
                current: total_extracted,
                previous: 0,
                delta_pct: 0.0,
                delta_label: "较上月".into(),
            },
        ];

        let s = state.extractor.clone();
        let m = state.matcher.clone();
        let idx = state.indexer.clone();

        Ok(DashboardResponse {
            total_matches,
            live_matches: live,
            exhausted_matches: exhausted,
            destroyed_matches: destroyed,
            total_extracted_shannons: total_extracted,
            active_orders_count: orders.len() as u32,
            wallet_count: wallets.len() as u32,
            channel_count: channels.len() as u32,
            tip_block,
            trends,
            extraction_history: Self::build_extraction_history(&mut conn)?,
            match_distribution: distribution,
            sparklines,
            scheduler: SchedulerStatusResponse {
                extractor: CycleStatus::from(&s),
                matcher: CycleStatus::from(&m),
                indexer: CycleStatus::from(&idx),
                tip_block,
                latest_event_id: state.latest_event_id(),
                events: vec![],
            },
            cache_updated_at_ms,
        })
    }

    /// Shannons → CKB conversion factor (1 CKB = 100,000,000 shannons).
    const SHANNONS_PER_CKB: f64 = 100_000_000.0;

    fn build_extraction_history(
        conn: &mut diesel::SqliteConnection,
    ) -> Result<Vec<ExtractionPoint>, AppError> {
        let daily = match_db::get_daily_extractions(conn, 30)?;
        // Reverse to chronological order (oldest first) for the trend chart.
        let mut points: Vec<ExtractionPoint> = daily
            .into_iter()
            .map(|d| ExtractionPoint {
                date: d.date,
                value: d.total_extracted as f64 / Self::SHANNONS_PER_CKB,
            })
            .collect();
        points.reverse();
        Ok(points)
    }

    fn build_sparklines(
        conn: &mut diesel::SqliteConnection,
    ) -> Result<HashMap<String, Vec<f64>>, AppError> {
        let mut map = HashMap::new();

        // Match count sparkline: daily extraction event counts (not monetary).
        let counts = match_db::get_daily_extraction_counts(conn, 12)?;
        map.insert(
            "matches".into(),
            counts.into_iter().map(|c| c as f64).collect(),
        );

        // Revenue sparkline: daily extraction amounts in CKB.
        let revenues = match_db::get_daily_extraction_revenue(conn, 12)?;
        map.insert(
            "revenue".into(),
            revenues
                .into_iter()
                .map(|v| v as f64 / Self::SHANNONS_PER_CKB)
                .collect(),
        );

        // Extracted sparkline: recent individual extraction amounts in CKB (chronological).
        let amounts = match_db::get_recent_extraction_amounts(conn, 12)?;
        map.insert(
            "extracted".into(),
            amounts
                .into_iter()
                .rev()
                .map(|v| v as f64 / Self::SHANNONS_PER_CKB)
                .collect(),
        );

        // Orders and channels sparklines: no historical data available yet,
        // use empty arrays so the frontend renders gracefully.
        map.insert("orders".into(), vec![]);
        map.insert("channels".into(), vec![]);

        Ok(map)
    }

    // ═══════════════════════════════════════════════════════
    // Wallets
    // ═══════════════════════════════════════════════════════

    pub fn list_wallets(pool: &DbPool) -> Result<Vec<wallet_db::WalletRecord>, AppError> {
        let mut conn = pool.get()?;
        wallet_db::list_wallets(&mut conn)
    }

    pub fn delete_wallet(pool: &DbPool, id: i64) -> Result<bool, AppError> {
        let mut conn = pool.get()?;
        wallet_db::delete_wallet(&mut conn, id)
    }

    // ═══════════════════════════════════════════════════════
    // HD Wallet
    // ═══════════════════════════════════════════════════════

    /// Create a new HD wallet. Returns keystore + mnemonic + child records.
    pub fn create_hd_wallet(
        pool: &DbPool,
        keystore_path: &std::path::Path,
        label: &str,
        password: &str,
        address_count: u32,
    ) -> Result<serde_json::Value, AppError> {
        let (keystore, mnemonic, children) = crate::services::wallet_service::create_hd_wallet(
            pool,
            keystore_path,
            label,
            password,
            address_count,
        )?;
        Ok(serde_json::json!({
            "keystore": keystore,
            "mnemonic": mnemonic,
            "children": children,
            "address_count": keystore.address_count,
        }))
    }

    /// Unlock an existing keystore.
    pub fn unlock_keystore(
        pool: &DbPool,
        keystore_path: &std::path::Path,
        password: &str,
    ) -> Result<serde_json::Value, AppError> {
        let (keystore, children) =
            crate::services::wallet_service::unlock_keystore(pool, keystore_path, password)?;
        Ok(serde_json::json!({
            "keystore": keystore,
            "children": children,
        }))
    }

    /// Derive additional addresses for an HD wallet.
    pub fn derive_more_addresses(
        pool: &DbPool,
        keystore_path: &std::path::Path,
        password: &str,
        count: u32,
    ) -> Result<Vec<wallet_db::WalletRecord>, AppError> {
        crate::services::wallet_service::derive_more_addresses(pool, keystore_path, password, count)
    }

    /// Get HD wallet status.
    pub fn get_hd_status(keystore_path: &std::path::Path) -> serde_json::Value {
        let exists = crate::services::wallet_service::hd_wallet_exists(keystore_path);
        let label = if exists {
            crate::services::keystore::load_keystore(keystore_path)
                .map(|k| k.label)
                .ok()
        } else {
            None
        };
        let address_count = if exists {
            crate::services::keystore::load_keystore(keystore_path)
                .map(|k| k.address_count)
                .unwrap_or(0)
        } else {
            0
        };
        serde_json::json!({
            "keystore_exists": exists,
            "label": label,
            "address_count": address_count,
        })
    }

    /// Get total balance for all HD child wallets.
    pub async fn get_hd_balance(
        pool: &DbPool,
        provider: &dyn ChainProvider,
    ) -> Result<u64, AppError> {
        crate::services::wallet_service::get_hd_wallet_balance(pool, provider).await
    }

    /// Get per-address balances for all HD child wallets.
    pub async fn get_hd_address_balances(
        pool: &DbPool,
        provider: &dyn ChainProvider,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let balances =
            crate::services::wallet_service::get_hd_wallet_address_balances(pool, provider).await?;
        Ok(balances
            .into_iter()
            .map(|(w, bal)| {
                serde_json::json!({
                    "wallet": w,
                    "balance_shannons": bal,
                })
            })
            .collect())
    }

    /// Re-sync HD addresses from keystore and refresh on-chain balances.
    pub async fn refresh_hd_wallet(
        pool: &DbPool,
        keystore_path: &std::path::Path,
        password: &str,
        provider: &dyn ChainProvider,
    ) -> Result<serde_json::Value, AppError> {
        let (keystore, children, total, balances) =
            crate::services::wallet_service::refresh_hd_wallet(
                pool,
                keystore_path,
                password,
                provider,
            )
            .await?;
        Ok(serde_json::json!({
            "keystore": keystore,
            "children": children,
            "total_balance_shannons": total,
            "address_balances": balances.into_iter().map(|(w, bal)| {
                serde_json::json!({
                    "wallet": w,
                    "balance_shannons": bal,
                })
            }).collect::<Vec<_>>(),
        }))
    }

    /// Import/recover HD wallet from a mnemonic phrase.
    pub fn import_mnemonic(
        pool: &DbPool,
        keystore_path: &std::path::Path,
        mnemonic_phrase: &str,
        label: &str,
        password: &str,
        address_count: u32,
    ) -> Result<serde_json::Value, AppError> {
        let (keystore, children) = crate::services::wallet_service::import_hd_from_mnemonic(
            pool,
            keystore_path,
            mnemonic_phrase,
            label,
            password,
            address_count,
        )?;
        Ok(serde_json::json!({
            "keystore": keystore,
            "children": children,
            "address_count": keystore.address_count,
        }))
    }

    /// Delete the HD wallet: remove keystore file and all hd_child rows.
    pub fn delete_hd_wallet(
        pool: &DbPool,
        keystore_path: &std::path::Path,
    ) -> Result<(), AppError> {
        crate::services::wallet_service::delete_hd_wallet(pool, keystore_path)
    }

    /// Reveal the mnemonic phrase from the keystore. Requires password verification.
    /// This is a read-only operation — no session is started, no signer is loaded.
    pub fn reveal_mnemonic(
        keystore_path: &std::path::Path,
        password: &str,
    ) -> Result<String, AppError> {
        let keystore = crate::services::keystore::load_keystore(keystore_path)?;
        let mnemonic = crate::services::keystore::decrypt_mnemonic(&keystore, password)?;
        Ok(mnemonic.to_string())
    }

    // ═══════════════════════════════════════════════════════
    // Orders
    // ═══════════════════════════════════════════════════════

    pub async fn match_order(
        _pool: &DbPool,
        provider: &dyn ChainProvider,
        order_tx_hash: &str,
        order_output_index: u32,
        seller_address: &str,
        signer: &crate::services::hd_wallet_signer::HdWalletSigner,
        tx_assembler: &crate::services::transaction_assembler::TransactionAssembler,
    ) -> Result<MatchOrderResult, AppError> {
        // Verify seller has on-chain CKB to pay tx fees
        let balance = provider
            .get_balance_by_address(seller_address)
            .await
            .unwrap_or(0);
        if balance < 10_000_000 {
            // Need at least ~0.1 CKB for fees
            return Err(AppError::BadRequest(format!(
                "Seller address has insufficient balance: {} CKB. Need at least 0.1 CKB for transaction fees.",
                balance as f64 / 100_000_000.0
            )));
        }

        // Look up the seller's secret key
        let secret_key = signer.find_key_by_address(seller_address).ok_or_else(|| {
            AppError::BadRequest(
                "Seller address not found in unlocked HD wallet. Unlock the wallet first.".into(),
            )
        })?;

        // Scan the order to get OrderInfo
        let orders = provider.scan_orders().await?;
        let order = orders
            .into_iter()
            .find(|o| {
                hex::encode(o.order_outpoint.tx_hash) == order_tx_hash
                    && o.order_outpoint.index == order_output_index
            })
            .ok_or_else(|| AppError::BadRequest("Order not found on chain".into()))?;

        // Find or create a compatible channel (exclude already-matched channels)
        let used_channel_outpoints =
            get_used_channel_outpoints(provider).await?;
        let channel = match_service::ensure_channel(
            provider,
            &hex::encode(order.order_args.fiber_pubkey.to_bytes()),
            order.order_data.channel_capacity,
            order_tx_hash,
            &used_channel_outpoints,
        )
        .await?;

        // Build match args with the channel outpoint
        let mut tx_hash_bytes = [0u8; 32];
        let tx_hash_decoded = hex::decode(&channel.tx_hash)
            .map_err(|_| AppError::BadRequest("Invalid channel tx_hash".into()))?;
        tx_hash_bytes.copy_from_slice(&tx_hash_decoded);
        let channel_outpoint = opticrum_protocol::OutPoint {
            tx_hash: tx_hash_bytes,
            index: channel.output_index,
        };
        let seller_lock_hash = {
            let lock_arg = crate::services::address::lock_arg_from_address(seller_address)?;
            crate::services::address::script_lock_hash(&lock_arg)
        };
        let order_args = order.order_args.clone();
        let match_args = opticrum_protocol::MatchArgs {
            order_args,
            channel_outpoint,
            seller_lock_hash,
        };

        let tx_hash = tx_assembler
            .match_order(seller_address, &secret_key, order, match_args)
            .await?;
        let output_index: i32 = 0;

        tracing::info!(
            tx_hash = %tx_hash,
            seller = %seller_address,
            channel_tx = %channel.tx_hash,
            "Order matched on-chain"
        );

        Ok(MatchOrderResult {
            tx_hash,
            output_index,
        })
    }

    /// Check whether an order is ready to match: peer is connected and a
    /// compatible channel exists. Used by the frontend to show per-order status.
    pub async fn get_match_readiness(
        provider: &dyn ChainProvider,
        _pool: &crate::db::DbPool,
        order_tx_hash: &str,
    ) -> Result<serde_json::Value, AppError> {
        let orders = provider.scan_orders().await?;
        let order = orders
            .into_iter()
            .find(|o| hex::encode(o.order_outpoint.tx_hash) == order_tx_hash)
            .ok_or_else(|| AppError::BadRequest("Order not found on chain".into()))?;

        let fiber_pubkey_hex = hex::encode(order.order_args.fiber_pubkey.to_bytes());
        let required_capacity = order.order_data.channel_capacity;

        // Run three independent queries in parallel: peer list, fiber channels,
        // and the order's own block number. None depend on each other.
        let (peers_result, channels_result, order_block_result) = tokio::join!(
            provider.list_peers(),
            provider.scan_fiber_channels(&[0u8; 32]),
            provider.get_tx_block_number(order_tx_hash),
        );
        let peers = peers_result?;
        let channels = channels_result?;
        let order_block = order_block_result.unwrap_or(0);

        // Check peer connectivity (tolerant of 0x prefix)
        let peer_connected = peers.iter().any(|p| {
            p.pubkey.trim_start_matches("0x") == fiber_pubkey_hex.trim_start_matches("0x")
        });

        // Check for compatible channel: matching peer + capacity + tx on-chain
        // + created after the order (contract requires channel_block > order_block).
        // + not already used in another match.
        // Only ChannelReady counts as usable; in-progress channels are reported
        // separately as pending_channel.
        // (channels already fetched in parallel above)
        let used_outpoints =
            get_used_channel_outpoints(provider).await?;
        let mut compatible = None;
        let mut pending = None;
        for ch in &channels {
            if ch.counterparty_fiber_key.trim_start_matches("0x")
                != fiber_pubkey_hex.trim_start_matches("0x")
                || ch.capacity < required_capacity
                || ch.tx_hash.is_empty()
            {
                continue;
            }
            // Exclude channels already used in another match (check against on-chain data)
            if used_outpoints.contains(&(ch.tx_hash.clone(), ch.output_index)) {
                continue;
            }
            let channel_block = provider.get_tx_block_number(&ch.tx_hash).await.unwrap_or(0);
            if channel_block == 0 || channel_block <= order_block {
                continue;
            }

            if match_service::is_channel_ready(&ch.state_name) {
                compatible = Some(ch);
                break;
            }
            if pending.is_none() && match_service::is_channel_pending(&ch.state_name) {
                pending = Some(ch);
            }
        }

        Ok(serde_json::json!({
            "peer_connected": peer_connected,
            "compatible_channel": compatible.map(|ch| serde_json::json!({
                "channel_id": ch.channel_id,
                "tx_hash": ch.tx_hash,
                "state_name": ch.state_name,
                "capacity": ch.capacity,
            })),
            "pending_channel": pending.map(|ch| serde_json::json!({
                "channel_id": ch.channel_id,
                "tx_hash": ch.tx_hash,
                "state_name": ch.state_name,
                "capacity": ch.capacity,
            })),
            "fiber_pubkey": fiber_pubkey_hex,
            "fiber_address": order.fiber_address,
            "required_capacity": required_capacity,
        }))
    }

    /// Open a channel for a specific order (connect peer + open_channel).
    pub async fn create_order_channel(
        provider: &dyn ChainProvider,
        order_tx_hash: &str,
    ) -> Result<serde_json::Value, AppError> {
        let orders = provider.scan_orders().await?;
        let order = orders
            .into_iter()
            .find(|o| hex::encode(o.order_outpoint.tx_hash) == order_tx_hash)
            .ok_or_else(|| AppError::BadRequest("Order not found on chain".into()))?;

        let fiber_pubkey_hex = hex::encode(order.order_args.fiber_pubkey.to_bytes());
        let fiber_address = order.fiber_address.as_deref();
        let required_capacity =
            order.order_data.channel_capacity + match_service::CHANNEL_CELL_OCCUPIED_RESERVE;

        // Connect peer first (required by Fiber). Pass fiber_address when
        // available so the Fiber node can dial the peer directly instead of
        // relying on DHT discovery.
        let _ = provider.connect_peer(&fiber_pubkey_hex, fiber_address).await;

        let temp_id = provider
            .open_channel(&fiber_pubkey_hex, required_capacity, fiber_address)
            .await?;

        Ok(serde_json::json!({
            "temporary_channel_id": temp_id,
            "peer": fiber_pubkey_hex,
            "capacity": required_capacity,
        }))
    }

    // ═══════════════════════════════════════════════════════
    // Matches
    // ═══════════════════════════════════════════════════════

    /// List matches directly from on-chain data.
    /// No longer syncs to/from a database table — the chain is the source of truth.
    ///
    /// Scan results come from the chain cache when populated; per-row enrichment
    /// uses local DB (extraction totals) and a single tip timestamp estimate —
    /// no per-match chain RPC on the list path.
    pub async fn list_matches(
        pool: &DbPool,
        status: Option<&str>,
        chain: &CachedChainProvider,
        signer_lock_hashes: Option<&[String]>,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let provider = chain as &dyn ChainProvider;
        let tip_block = provider.get_tip_block_number().await.unwrap_or(0);
        let on_chain = provider.scan_matches().await?;

        let tip_timestamp = if tip_block > 0 {
            provider
                .get_block_timestamp(tip_block)
                .await
                .ok()
                .filter(|&ts| ts > 0)
        } else {
            None
        };

        let mut conn = pool.get()?;
        let mut results: Vec<serde_json::Value> = Vec::new();

        for m in &on_chain {
            let tx_hash = hex::encode(m.match_outpoint.tx_hash);
            let output_index = m.match_outpoint.index;

            // Status filter
            let is_exhausted = m.ckb_capacity == 0;
            if let Some(s) = status {
                match s {
                    "live" if is_exhausted => continue,
                    "exhausted" if !is_exhausted => continue,
                    "destroyed" => continue, // destroyed cells aren't in scan_matches
                    _ => {}
                }
            }

            // Signer lock hash filter
            let seller_lh = hex::encode(m.match_args.seller_lock_hash);
            if let Some(hashes) = signer_lock_hashes {
                if !hashes.is_empty() && !hashes.iter().any(|h| h.eq_ignore_ascii_case(&seller_lh))
                {
                    continue;
                }
            }

            // Resolve seller address from wallet DB
            let seller_address =
                Self::resolve_lock_hash_to_address(&mut conn, &format!("lock_hash:{seller_lh}"))
                    .unwrap_or_else(|| format!("lock_hash:{seller_lh}"));

            // Extraction totals from chain cache (never walk chain on the list path)
            let extracted_total = chain
                .extraction_chain(&tx_hash, output_index)
                .map(|c| c.total_extracted)
                .unwrap_or(0);

            // How much rent is currently withdrawable from this match cell
            let extractable = rent_service::preview_extractable_from_chain(m, tip_block);

            // Estimate remaining days before the match cell's rent is exhausted.
            // escrow_blocks = total blocks the current capacity can sustain at the given rate.
            // baseline = the block from which rent started accumulating (match_current_block
            //   for never-extracted matches, last_extraction_block after each extraction).
            // CKB block time ≈ 12 s → 7200 blocks/day.
            let remaining_days: f64 = m
                .ckb_capacity
                .checked_div(m.match_data.shannons_per_block)
                .map(|escrow_blocks| {
                    let baseline = if m.match_data.last_extraction_block == 0 {
                        m.match_current_block
                    } else {
                        m.match_data.last_extraction_block
                    };
                    let blocks_elapsed = tip_block.saturating_sub(baseline);
                    let remaining_blocks = escrow_blocks.saturating_sub(blocks_elapsed);
                    remaining_blocks as f64 / 7200.0
                })
                .unwrap_or(0.0);

            // Match time from cached match_current_block + tip anchor (one RPC for whole list)
            let created_at =
                Self::estimate_block_timestamp(m.match_current_block, tip_block, tip_timestamp);

            results.push(serde_json::json!({
                "tx_hash": tx_hash,
                "output_index": output_index,
                "order_tx_hash": hex::encode(m.match_args.order_args.fiber_pubkey.to_bytes()),
                "order_output_index": 0,
                "seller_address": seller_address,
                "shannons_per_block": m.match_data.shannons_per_block,
                "ckb_capacity": m.ckb_capacity,
                "last_extraction_block": m.match_data.last_extraction_block,
                "xudt_amount": m.match_data.xudt_amount,
                "status": if is_exhausted { "exhausted" } else { "live" },
                "extracted_total": extracted_total,
                "extractable_shannons": extractable,
                "created_at": created_at,
                "remaining_days": remaining_days,
                "tip_block": tip_block,
            }));
        }

        Ok(results)
    }

    /// Estimate a block's Unix-ms timestamp from the tip block anchor.
    /// CKB average block interval ≈ 12 seconds.
    fn estimate_block_timestamp(
        block_number: u64,
        tip_block: u64,
        tip_timestamp: Option<u64>,
    ) -> Option<u64> {
        const AVG_BLOCK_MS: u64 = 12_000;
        tip_timestamp.map(|tip_ts| {
            let blocks_behind = tip_block.saturating_sub(block_number);
            tip_ts.saturating_sub(blocks_behind * AVG_BLOCK_MS)
        })
    }

    /// Try to resolve a `"lock_hash:<hex>"` placeholder to a proper CKB address
    /// by looking up the wallet with that lock_hash in the database.
    fn resolve_lock_hash_to_address(
        conn: &mut diesel::SqliteConnection,
        seller_address: &str,
    ) -> Option<String> {
        let hex_part = seller_address.strip_prefix("lock_hash:")?;
        let lock_hash_bytes = hex::decode(hex_part).ok()?;
        match wallet_db::get_wallet_by_lock_hash(conn, &lock_hash_bytes) {
            Ok(wallet) => Some(wallet.ckb_address),
            Err(_) => None,
        }
    }

    /// Get full match detail with extraction history from on-chain data.
    pub async fn get_match_detail(
        pool: &DbPool,
        tx_hash: &str,
        output_index: u32,
        chain: &CachedChainProvider,
    ) -> Result<serde_json::Value, AppError> {
        let provider = chain as &dyn ChainProvider;
        let mut conn = pool.get()?;

        let on_chain = provider.scan_matches().await?;
        let m = on_chain
            .iter()
            .find(|mi| {
                hex::encode(mi.match_outpoint.tx_hash) == tx_hash
                    && mi.match_outpoint.index == output_index
            })
            .ok_or_else(|| {
                AppError::NotFound(format!("Match {tx_hash}:{output_index} not found on chain"))
            })?;

        let extraction = chain.get_extraction_chain(m).await;
        let extracted_total = extraction.total_extracted;

        // Resolve timestamps for extraction events from on-chain block headers.
        // We use the current tip block timestamp as an anchor and compute
        // event timestamps via the CKB average block interval to avoid
        // N individual RPC calls that may fail independently.
        let tip_block = provider.get_tip_block_number().await.unwrap_or(0);
        let tip_timestamp = provider
            .get_block_timestamp(tip_block)
            .await
            .ok()
            .filter(|&ts| ts > 0);
        // CKB average block interval ≈ 12 seconds = 12_000 ms
        const AVG_BLOCK_MS: u64 = 12_000;

        let mut history: Vec<serde_json::Value> = Vec::new();
        for (i, e) in extraction.extractions.iter().enumerate() {
            let timestamp = tip_timestamp.map(|tip_ts| {
                let blocks_behind = tip_block.saturating_sub(e.block_number);
                tip_ts.saturating_sub(blocks_behind * AVG_BLOCK_MS)
            });
            history.push(serde_json::json!({
                "id": i,
                "tx_hash": e.tx_hash,
                "block_number": e.block_number,
                "tip_block": e.block_number,
                "extracted_amount": e.extracted_amount,
                "timestamp": timestamp,
            }));
        }

        let seller_lh = hex::encode(m.match_args.seller_lock_hash);
        let seller_address =
            Self::resolve_lock_hash_to_address(&mut conn, &format!("lock_hash:{seller_lh}"))
                .unwrap_or_else(|| format!("lock_hash:{seller_lh}"));

        let is_exhausted = m.ckb_capacity == 0;
        let status = if is_exhausted { "exhausted" } else { "live" };

        // Get match creation timestamp (Unix milliseconds) from the match tx's block
        let created_at: Option<u64> = match provider.get_tx_block_number(tx_hash).await {
            Ok(block_number) if block_number > 0 => provider
                .get_block_timestamp(block_number)
                .await
                .ok()
                .filter(|&ts| ts > 0),
            _ => None,
        };

        Ok(serde_json::json!({
            "tx_hash": tx_hash,
            "output_index": output_index,
            "order_tx_hash": hex::encode(m.match_args.order_args.fiber_pubkey.to_bytes()),
            "order_output_index": 0,
            "seller_address": seller_address,
            "shannons_per_block": m.match_data.shannons_per_block,
            "ckb_capacity": m.ckb_capacity,
            "last_extraction_block": m.match_data.last_extraction_block,
            "xudt_amount": m.match_data.xudt_amount,
            "status": status,
            "created_at": created_at,
            "extracted_total_shannons": extracted_total,
            "extraction_history": history,
        }))
    }

    pub async fn extract_rent(
        pool: &DbPool,
        provider: &dyn ChainProvider,
        tx_hash: &str,
        output_index: u32,
        tx_assembler: Option<&TransactionAssembler>,
        signer: &HdWalletSigner,
        min_extraction_shannons: u64,
    ) -> Result<rent_service::ExtractRentResult, AppError> {
        rent_service::extract_rent(
            provider,
            pool,
            tx_hash,
            output_index,
            &rent_service::ExtractRentOptions {
                tx_assembler,
                signer: Some(signer),
                min_extraction_shannons,
            },
        )
        .await
    }

    pub async fn destroy_match(
        _pool: &DbPool,
        provider: &dyn ChainProvider,
        tx_hash: &str,
        output_index: u32,
    ) -> Result<String, AppError> {
        rent_service::destroy_match(provider, tx_hash, output_index).await
    }

    // ═══════════════════════════════════════════════════════
    // Channels
    // ═══════════════════════════════════════════════════════

    /// List all Fiber channels with their associated on-chain opticrum match
    /// cells (if any). Uses two-step matching:
    /// 1. Filter match cells by counterparty fiber pubkey
    /// 2. Among filtered, match by channel outpoint
    pub async fn get_channels_with_matches(
        _pool: &DbPool,
        provider: &dyn ChainProvider,
    ) -> Result<Vec<ChannelWithMatch>, AppError> {
        let (channels_result, matches_result) = tokio::join!(
            provider.scan_fiber_channels(&[0u8; 32]),
            provider.scan_matches(),
        );
        let channels = channels_result?;

        // Scan on-chain matches — if it fails, still return channels without
        // match info rather than failing the whole request.
        let matches = match matches_result {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    "Failed to scan on-chain matches: {} — returning channels without match info",
                    e
                );
                vec![]
            }
        };

        Ok(channels
            .into_iter()
            .filter(|ch| !match_service::is_channel_terminal(&ch.state_name))
            .map(|ch| {
                let match_info = find_match_for_channel(&ch, &matches);
                let match_status = if match_info.is_some() {
                    "matched"
                } else {
                    "not_found"
                };
                ChannelWithMatch {
                    channel: ch,
                    match_info,
                    match_status: match_status.to_string(),
                    fiber_address: None,
                }
            })
            .collect())
    }

    /// List all Fiber channels only (no match cross-referencing).
    /// Fast path for progressive loading — the frontend calls this first to
    /// render the channel table immediately, then calls `get_channel_matches`
    /// to backfill match status.
    pub async fn get_channels_only(
        provider: &dyn ChainProvider,
    ) -> Result<Vec<FiberChannelInfo>, AppError> {
        let channels = provider.scan_fiber_channels(&[0u8; 32]).await?;
        Ok(channels
            .into_iter()
            .filter(|ch| !match_service::is_channel_terminal(&ch.state_name))
            .collect())
    }

    /// Cross-reference all Fiber channels with their on-chain match cells.
    /// Returns a map from channel_id → match_info so the frontend can
    /// progressively enrich an already-rendered channel table.
    pub async fn get_channel_matches(
        provider: &dyn ChainProvider,
    ) -> Result<Vec<ChannelWithMatch>, AppError> {
        let (channels_result, matches_result, orders_result) = tokio::join!(
            provider.scan_fiber_channels(&[0u8; 32]),
            provider.scan_matches(),
            provider.scan_orders(),
        );
        let channels = channels_result?;
        let matches = matches_result.unwrap_or_else(|e| {
            tracing::warn!("Failed to scan on-chain matches for channel-matches: {}", e);
            vec![]
        });
        let orders = orders_result.unwrap_or_else(|e| {
            tracing::warn!("Failed to scan on-chain orders for channel-matches: {}", e);
            vec![]
        });

        // Build a pubkey → fiber_address lookup from on-chain orders
        let addr_by_pubkey: HashMap<String, Option<String>> = orders
            .iter()
            .map(|o| {
                (
                    hex::encode(o.order_args.fiber_pubkey.as_ref()),
                    o.fiber_address.clone(),
                )
            })
            .collect();

        Ok(channels
            .into_iter()
            .filter(|ch| !match_service::is_channel_terminal(&ch.state_name))
            .map(|ch| {
                let match_info = find_match_for_channel(&ch, &matches);
                let match_status = if match_info.is_some() {
                    "matched"
                } else {
                    "not_found"
                };
                let fiber_address = addr_by_pubkey
                    .get(&ch.counterparty_fiber_key)
                    .and_then(|a| a.clone());
                ChannelWithMatch {
                    channel: ch,
                    match_info,
                    match_status: match_status.to_string(),
                    fiber_address,
                }
            })
            .collect())
    }

    /// Shut down a Fiber channel by its channel ID.
    /// Uses cooperative close by default (`force=false`).
    pub async fn close_channel(
        provider: &dyn ChainProvider,
        channel_id: &str,
        force: bool,
    ) -> Result<(), AppError> {
        provider.shutdown_channel(channel_id, force).await
    }

    /// Verify a closed Fiber channel exists and can be dismissed from the console.
    /// After the chain-first refactor, no DB records are deleted — closed channels
    /// are filtered out by `get_channels_with_matches` (terminal state check).
    pub async fn delete_channel(
        _pool: &DbPool,
        provider: &dyn ChainProvider,
        channel_id: &str,
    ) -> Result<(), AppError> {
        let channels = provider.scan_fiber_channels(&[0u8; 32]).await?;
        let channel = channels
            .into_iter()
            .find(|ch| ch.channel_id == channel_id)
            .ok_or_else(|| AppError::NotFound(format!("Channel {channel_id} not found")))?;

        if channel.state_name != "Closed" {
            return Err(AppError::BadRequest(
                "Only closed channels can be deleted".into(),
            ));
        }

        tracing::info!(channel_id = %channel_id, "Fiber channel dismissed from console");
        Ok(())
    }

    // ═══════════════════════════════════════════════════════
    // Fiber node info
    // ═══════════════════════════════════════════════════════

    pub async fn get_fiber_node_info(
        provider: &dyn ChainProvider,
    ) -> Result<Option<crate::services::chain_provider::FiberNodeInfo>, AppError> {
        provider.get_fiber_node_info().await
    }

    // ═══════════════════════════════════════════════════════
    // Signing
    // ═══════════════════════════════════════════════════════

    pub fn list_unsigned_txs(
        pool: &DbPool,
    ) -> Result<Vec<unsigned_db::UnsignedTransaction>, AppError> {
        let mut conn = pool.get()?;
        unsigned_db::list_unsigned_txs(&mut conn)
    }

    pub fn get_unsigned_tx(
        pool: &DbPool,
        id: &str,
    ) -> Result<unsigned_db::UnsignedTransaction, AppError> {
        let mut conn = pool.get()?;
        unsigned_db::get_unsigned_tx(&mut conn, id)
    }

    pub fn submit_witnesses(
        pool: &DbPool,
        id: &str,
        witnesses: serde_json::Value,
    ) -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let json = serde_json::to_string(&witnesses)
            .map_err(|e| AppError::BadRequest(format!("Invalid witnesses JSON: {}", e)))?;
        unsigned_db::set_witnesses(&mut conn, id, &json)
    }

    pub fn submit_to_chain(pool: &DbPool, id: &str) -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let tx_hash = format!("broadcast:{}", id);
        unsigned_db::mark_broadcast(&mut conn, id, &tx_hash)
    }

    // ═══════════════════════════════════════════════════════
    // Config
    // ═══════════════════════════════════════════════════════

    /// Snapshot of the runtime-configurable settings.
    pub fn get_runtime_config(rc: &RuntimeConfig) -> serde_json::Value {
        serde_json::to_value(rc).unwrap_or_default()
    }

    /// Apply partial updates to the runtime config.
    pub fn update_runtime_config(
        rc: &Arc<RwLock<RuntimeConfig>>,
        partial: RuntimeConfigPartial,
    ) -> serde_json::Value {
        let mut cfg = rc.write().unwrap();
        cfg.apply_partial(&partial);
        serde_json::to_value(&*cfg).unwrap_or_default()
    }

    /// Reset runtime config to config.toml values.
    pub fn reset_runtime_config(
        rc: &Arc<RwLock<RuntimeConfig>>,
        config: &Config,
    ) -> serde_json::Value {
        let mut cfg = rc.write().unwrap();
        cfg.reset_from_config(config);
        serde_json::to_value(&*cfg).unwrap_or_default()
    }

    // ═══════════════════════════════════════════════════════
    // Scheduler status
    // ═══════════════════════════════════════════════════════

    pub fn get_scheduler_status(state: &SchedulerState, since: u64) -> SchedulerStatusResponse {
        SchedulerStatusResponse {
            extractor: CycleStatus::from(&state.extractor),
            matcher: CycleStatus::from(&state.matcher),
            indexer: CycleStatus::from(&state.indexer),
            tip_block: state.tip_block,
            latest_event_id: state.latest_event_id(),
            events: state
                .events_since(since)
                .into_iter()
                .map(|e| SchedulerEventResponse {
                    id: e.id,
                    ts_ms: e.ts_ms,
                    source: e.source,
                    level: e.level,
                    message: e.message,
                })
                .collect(),
        }
    }

    // ═══════════════════════════════════════════════════════
    // Signer info
    // ═══════════════════════════════════════════════════════

    pub fn get_signer_info(signer: &dyn Signer) -> serde_json::Value {
        let hashes: Vec<String> = signer.lock_hashes().iter().map(hex::encode).collect();
        serde_json::json!({
            "label": signer.label(),
            "lock_hashes": hashes,
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Find the on-chain opticrum Match cell corresponding to a Fiber channel.
///
/// Two-step lookup:
/// 1. Filter match cells by counterparty fiber pubkey
///    (`counterparty_fiber_key == match_args.order_args.fiber_pubkey`)
/// 2. Among filtered, match by channel outpoint
///    (`(tx_hash, output_index) == match_args.channel_outpoint`)
///
/// Returns `None` if the counterparty's Fiber key is not a valid 33-byte
/// pubkey, or if no matching match cell is found.
fn find_match_for_channel(
    channel: &FiberChannelInfo,
    matches: &[MatchInfo],
) -> Option<ChannelMatchInfo> {
    // Decode counterparty Fiber key to 33-byte compressed pubkey
    let target_bytes = hex::decode(&channel.counterparty_fiber_key).ok()?;
    if target_bytes.len() != 33 {
        return None;
    }
    let mut pk_arr = [0u8; 33];
    pk_arr.copy_from_slice(&target_bytes);
    let target_pubkey = CompressedPubkey::new(pk_arr);

    for m in matches {
        // Step 1: fiber pubkey must match
        if m.match_args.order_args.fiber_pubkey != target_pubkey {
            continue;
        }
        // Step 2: channel outpoint must match
        let match_tx = hex::encode(m.match_args.channel_outpoint.tx_hash);
        if match_tx != channel.tx_hash
            || m.match_args.channel_outpoint.index != channel.output_index
        {
            continue;
        }
        // Both matched — build result
        return Some(ChannelMatchInfo {
            match_tx_hash: hex::encode(m.match_outpoint.tx_hash),
            match_output_index: m.match_outpoint.index,
            xudt_amount: m.match_data.xudt_amount,
            shannons_per_block: m.match_data.shannons_per_block,
            last_extraction_block: m.match_data.last_extraction_block,
            ckb_capacity: m.ckb_capacity,
            seller_lock_hash: hex::encode(m.match_args.seller_lock_hash),
        });
    }
    None
}
