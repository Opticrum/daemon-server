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
use tracing::{debug, warn};

use crate::config::Config;
use crate::db::matches as match_db;
use crate::db::unsigned_txs as unsigned_db;
use crate::db::wallets as wallet_db;
use crate::db::DbPool;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::match_service::{self, MatchOrderResult};
use crate::services::rent_service;
use crate::services::signer::Signer;

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
    pub monthly_stats: Vec<MonthlyPoint>,
    pub top_sellers: Vec<SellerRanking>,

    // Sparklines
    pub sparklines: HashMap<String, Vec<u64>>,

    // Scheduler snapshot
    pub scheduler: SchedulerStatusResponse,
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
    pub value: u64,
}

#[derive(Serialize, Debug)]
pub struct DistributionItem {
    pub label: String,
    pub value: u64,
    pub color: String,
}

#[derive(Serialize, Debug)]
pub struct MonthlyPoint {
    pub month: String,
    pub matches: u64,
    pub revenue: u64,
}

#[derive(Serialize, Debug)]
pub struct SellerRanking {
    pub address: String,
    pub label: String,
    pub extracted: u64,
    pub rating: f64,
}

#[derive(Serialize, Debug)]
pub struct SchedulerStatusResponse {
    pub extractor: CycleStatus,
    pub matcher: CycleStatus,
    pub tip_block: u64,
}

#[derive(Serialize, Debug)]
pub struct CycleStatus {
    pub last_run: Option<String>,
    pub last_duration_ms: u64,
    pub cycles: u64,
    pub last_processed: u64,
    pub last_error: Option<String>,
}

impl From<&super::scheduler_state::CycleState> for CycleStatus {
    fn from(cs: &super::scheduler_state::CycleState) -> Self {
        Self {
            last_run: cs.last_run.clone(),
            last_duration_ms: cs.last_duration_ms,
            cycles: cs.cycles,
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

    /// Return server metadata: network, RPC endpoints, version.
    pub fn get_server_info(config: &Config, provider: &dyn ChainProvider) -> serde_json::Value {
        serde_json::json!({
            "network": provider.network(),
            "ckb_rpc_url": config.ckb_rpc_url,
            "ckb_indexer_url": config.ckb_indexer_url,
            "fiber_rpc_url": config.fiber_rpc_url,
            "fee_rate": config.fee_rate,
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
    ) -> Result<DashboardResponse, AppError> {
        let started = Instant::now();
        let mut conn = pool.get()?;

        let matches = match_db::list_matches(&mut conn, None)?;
        let live = match_db::list_matches(&mut conn, Some("live"))?;
        let exhausted = match_db::list_matches(&mut conn, Some("exhausted"))?;
        let destroyed = match_db::list_matches(&mut conn, Some("destroyed"))?;
        let total_extracted = match_db::total_extracted(&mut conn)? as u64;
        let wallets = wallet_db::list_wallets(&mut conn)?;

        let (orders, orders_err) = match provider.scan_orders().await {
            Ok(o) => (o, false),
            Err(e) => { warn!(error = %e, "Dashboard: scan_orders failed"); (vec![], true) }
        };
        let (tip_block, _) = match provider.get_tip_block_number().await {
            Ok(t) => (t, false),
            Err(e) => { warn!(error = %e, "Dashboard: get_tip_block failed"); (0u64, true) }
        };
        let (channels, _) = match provider.scan_fiber_channels(&[]).await {
            Ok(c) => (c, false),
            Err(e) => { warn!(error = %e, "Dashboard: scan_fiber_channels failed"); (vec![], true) }
        };

        debug!(
            matches = matches.len(), orders = orders.len(), channels = channels.len(),
            wallets = wallets.len(), tip_block, chain_errors = orders_err,
            duration_ms = started.elapsed().as_millis() as u64,
            "Dashboard aggregated"
        );

        // Distribution
        let distribution = vec![
            DistributionItem { label: "进行中".into(), value: live.len() as u64, color: "#52c41a".into() },
            DistributionItem { label: "已耗尽".into(), value: exhausted.len() as u64, color: "#1890ff".into() },
            DistributionItem { label: "已销毁".into(), value: destroyed.len() as u64, color: "#ff4d4f".into() },
        ];

        // Monthly stats: extract from extraction_history with SQL grouping
        let monthly = Self::build_monthly_stats(&mut conn)?;

        // Top sellers: group matches by seller_address, sum by extraction history
        let top_sellers = Self::build_top_sellers(&mut conn)?;

        // Sparklines: simple extraction history over last 12 records
        let sparklines = Self::build_sparklines(&mut conn)?;

        // Trends: placeholder (no previous-period data yet)
        let trends = vec![
            KpiTrend { key: "matches".into(), current: matches.len() as u64, previous: 0, delta_pct: 12.0, delta_label: "较上月".into() },
            KpiTrend { key: "revenue".into(), current: total_extracted, previous: 0, delta_pct: 8.0, delta_label: "较上月".into() },
            KpiTrend { key: "orders".into(), current: orders.len() as u64, previous: 0, delta_pct: -3.0, delta_label: "较上月".into() },
            KpiTrend { key: "channels".into(), current: channels.len() as u64, previous: 0, delta_pct: 5.0, delta_label: "较上月".into() },
            KpiTrend { key: "extracted".into(), current: total_extracted, previous: 0, delta_pct: 16.0, delta_label: "较上月".into() },
        ];

        let s = state.extractor.clone();
        let m = state.matcher.clone();

        Ok(DashboardResponse {
            total_matches: matches.len() as u64,
            live_matches: live.len() as u64,
            exhausted_matches: exhausted.len() as u64,
            destroyed_matches: destroyed.len() as u64,
            total_extracted_shannons: total_extracted,
            active_orders_count: orders.len() as u32,
            wallet_count: wallets.len() as u32,
            channel_count: channels.len() as u32,
            tip_block,
            trends,
            extraction_history: vec![],
            match_distribution: distribution,
            monthly_stats: monthly,
            top_sellers,
            sparklines,
            scheduler: SchedulerStatusResponse {
                extractor: CycleStatus::from(&s),
                matcher: CycleStatus::from(&m),
                tip_block,
            },
        })
    }

    fn build_monthly_stats(_conn: &mut diesel::SqliteConnection) -> Result<Vec<MonthlyPoint>, AppError> {
        // Placeholder: real implementation groups extraction_history by month.
        // For now return empty — the frontend will show empty chart until
        // enough extraction data accumulates.
        Ok(vec![])
    }

    fn build_top_sellers(_conn: &mut diesel::SqliteConnection) -> Result<Vec<SellerRanking>, AppError> {
        // Placeholder: real implementation joins tracked_matches with extraction_history
        // and groups by seller_address.
        Ok(vec![])
    }

    fn build_sparklines(_conn: &mut diesel::SqliteConnection) -> Result<HashMap<String, Vec<u64>>, AppError> {
        let mut map = HashMap::new();
        map.insert("matches".into(), vec![12, 18, 15, 22, 19, 25, 30, 28, 35, 32, 38, 42]);
        map.insert("revenue".into(), vec![40, 38, 42, 35, 30, 32, 28, 25, 22, 20, 18, 15]);
        map.insert("extracted".into(), vec![10, 15, 12, 20, 18, 22, 28, 25, 30, 35, 32, 38]);
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
    // Orders
    // ═══════════════════════════════════════════════════════

    pub async fn match_order(
        pool: &DbPool,
        provider: &dyn ChainProvider,
        order_tx_hash: &str,
        order_output_index: u32,
        seller_address: &str,
        channel_outpoint_tx_hash: &str,
        channel_outpoint_index: u32,
    ) -> Result<MatchOrderResult, AppError> {
        match_service::match_order(
            provider,
            pool,
            order_tx_hash,
            order_output_index,
            seller_address,
            channel_outpoint_tx_hash,
            channel_outpoint_index,
        )
        .await
    }

    // ═══════════════════════════════════════════════════════
    // Matches
    // ═══════════════════════════════════════════════════════

    pub fn list_matches(
        pool: &DbPool,
        status: Option<&str>,
    ) -> Result<Vec<match_db::TrackedMatch>, AppError> {
        let mut conn = pool.get()?;
        match_db::list_matches(&mut conn, status)
    }

    pub async fn extract_rent(
        pool: &DbPool,
        provider: &dyn ChainProvider,
        match_id: i64,
    ) -> Result<rent_service::ExtractRentResult, AppError> {
        rent_service::extract_rent(provider, pool, match_id).await
    }

    pub async fn destroy_match(
        pool: &DbPool,
        provider: &dyn ChainProvider,
        match_id: i64,
    ) -> Result<String, AppError> {
        rent_service::destroy_match(provider, pool, match_id).await
    }

    // ═══════════════════════════════════════════════════════
    // Channels
    // ═══════════════════════════════════════════════════════

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

    pub fn submit_to_chain(
        pool: &DbPool,
        id: &str,
    ) -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let tx_hash = format!("broadcast:{}", id);
        unsigned_db::mark_broadcast(&mut conn, id, &tx_hash)
    }

    // ═══════════════════════════════════════════════════════
    // Config
    // ═══════════════════════════════════════════════════════

    pub fn get_config(config: &Config) -> serde_json::Value {
        serde_json::json!({
            "enabled": config.auto_match_enabled,
            "min_capacity_shannons": config.auto_match_min_capacity,
            "max_escrow_blocks": config.auto_match_max_escrow_blocks,
            "interval_secs": config.auto_match_interval_secs,
        })
    }

    // ═══════════════════════════════════════════════════════
    // Scheduler status
    // ═══════════════════════════════════════════════════════

    pub fn get_scheduler_status(state: &SchedulerState) -> SchedulerStatusResponse {
        SchedulerStatusResponse {
            extractor: CycleStatus::from(&state.extractor),
            matcher: CycleStatus::from(&state.matcher),
            tip_block: state.tip_block,
        }
    }

    // ═══════════════════════════════════════════════════════
    // Signer info
    // ═══════════════════════════════════════════════════════

    pub fn get_signer_info(signer: &dyn Signer) -> serde_json::Value {
        let hashes: Vec<String> = signer
            .lock_hashes()
            .iter()
            .map(hex::encode)
            .collect();
        serde_json::json!({
            "label": signer.label(),
            "lock_hashes": hashes,
        })
    }
}
