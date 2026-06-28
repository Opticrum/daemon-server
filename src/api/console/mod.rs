//! Console gateway API — single unified surface for the Web Console SPA.
//!
//! Every handler delegates to `GatewayService` methods.
//! All routes are mounted under `/api/console`.

use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::api::AppState;
use crate::error::AppError;
use crate::services::console::gateway_service::GatewayService;

/// Mount all console routes under `/api/console`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    tracing::info!("Registering console gateway routes");
    cfg.service(
        web::scope("/api/console")
            // Dashboard
            .route("/dashboard", web::get().to(dashboard))
            // Wallets
            .route("/wallets", web::get().to(list_wallets))
            .route("/wallets", web::post().to(import_wallet))
            .route("/wallets/{id}", web::delete().to(delete_wallet))
            // Orders
            .route("/orders", web::get().to(scan_orders))
            .route("/orders/{tx_hash}/match", web::post().to(match_order))
            // Matches
            .route("/matches", web::get().to(list_matches))
            .route("/matches/{id}/extract", web::post().to(extract_rent))
            .route("/matches/{id}/destroy", web::post().to(destroy_match))
            // Channels
            .route("/channels", web::get().to(scan_channels))
            // Signing
            .route("/signing", web::get().to(list_unsigned))
            .route("/signing/{id}", web::get().to(get_unsigned))
            .route("/signing/{id}/witnesses", web::post().to(submit_witnesses))
            .route("/signing/{id}/submit", web::post().to(submit_to_chain))
            // Config
            .route("/config", web::get().to(get_config))
            // Scheduler
            .route("/scheduler/status", web::get().to(scheduler_status))
            // Signer
            .route("/signer-info", web::get().to(signer_info))
            // Server info
            .route("/server-info", web::get().to(server_info)),
    );
}

// ═══════════════════════════════════════════════════════
// Server info
// ═══════════════════════════════════════════════════════

pub async fn server_info(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let info = GatewayService::get_server_info(&state.config, state.chain_provider.as_ref());
    Ok(HttpResponse::Ok().json(info))
}

// ═══════════════════════════════════════════════════════
// Dashboard
// ═══════════════════════════════════════════════════════

pub async fn dashboard(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let s = {
        let guard = state.scheduler_state.read().map_err(|e| {
            AppError::Internal(format!("Scheduler state lock: {}", e))
        })?;
        guard.clone()
    };
    let dash = GatewayService::get_dashboard(
        &state.db,
        state.chain_provider.as_ref(),
        &s,
    )
    .await?;
    Ok(HttpResponse::Ok().json(dash))
}

// ═══════════════════════════════════════════════════════
// Wallets
// ═══════════════════════════════════════════════════════

pub async fn list_wallets(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let wallets = GatewayService::list_wallets(&state.db)?;
    Ok(HttpResponse::Ok().json(wallets))
}

#[derive(Deserialize)]
pub struct ImportWalletBody {
    label: String,
    private_key_hex: String,
    password: Option<String>,
}

pub async fn import_wallet(
    state: web::Data<AppState>,
    body: web::Json<ImportWalletBody>,
) -> Result<HttpResponse, AppError> {
    let w = crate::services::wallet_service::import_wallet(
        &state.db,
        &body.label,
        &body.private_key_hex,
        body.password.as_deref(),
    )?;
    Ok(HttpResponse::Created().json(w))
}

pub async fn delete_wallet(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let deleted = GatewayService::delete_wallet(&state.db, id)?;
    if deleted {
        Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
    } else {
        Err(AppError::NotFound(format!("Wallet id={}", id)))
    }
}

// ═══════════════════════════════════════════════════════
// Orders
// ═══════════════════════════════════════════════════════

#[derive(serde::Serialize)]
pub struct OrderScanItem {
    tx_hash: String,
    output_index: u32,
    fiber_pubkey: String,
    buyer_lock_hash: String,
    xudt_amount: u128,
    channel_capacity: u64,
    shannons_per_block: u64,
    ckb_capacity: u64,
}

pub async fn scan_orders(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let orders = state.chain_provider.scan_orders().await?;
    let items: Vec<OrderScanItem> = orders
        .into_iter()
        .map(|o| OrderScanItem {
            tx_hash: hex::encode(o.order_outpoint.tx_hash),
            output_index: o.order_outpoint.index,
            fiber_pubkey: hex::encode(o.order_args.fiber_pubkey.to_bytes()),
            buyer_lock_hash: hex::encode(o.order_args.buyer_lock_hash),
            xudt_amount: o.order_data.xudt_amount,
            channel_capacity: o.order_data.channel_capacity,
            shannons_per_block: o.order_data.shannons_per_block,
            ckb_capacity: o.ckb_capacity,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

#[derive(Deserialize)]
pub struct MatchOrderBody {
    order_output_index: u32,
    seller_address: String,
    channel_outpoint_tx_hash: String,
    channel_outpoint_index: u32,
}

pub async fn match_order(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<MatchOrderBody>,
) -> Result<HttpResponse, AppError> {
    let tx_hash = path.into_inner();
    let result = GatewayService::match_order(
        &state.db,
        state.chain_provider.as_ref(),
        &tx_hash,
        body.order_output_index,
        &body.seller_address,
        &body.channel_outpoint_tx_hash,
        body.channel_outpoint_index,
    )
    .await?;
    Ok(HttpResponse::Ok().json(result))
}

// ═══════════════════════════════════════════════════════
// Matches
// ═══════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct ListMatchesQuery {
    status: Option<String>,
}

pub async fn list_matches(
    state: web::Data<AppState>,
    query: web::Query<ListMatchesQuery>,
) -> Result<HttpResponse, AppError> {
    let matches = GatewayService::list_matches(&state.db, query.status.as_deref())?;
    Ok(HttpResponse::Ok().json(matches))
}

pub async fn extract_rent(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let result = GatewayService::extract_rent(&state.db, state.chain_provider.as_ref(), id).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn destroy_match(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let tx_hash = GatewayService::destroy_match(&state.db, state.chain_provider.as_ref(), id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"tx_hash": tx_hash, "status": "destroyed"})))
}

// ═══════════════════════════════════════════════════════
// Channels
// ═══════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct ChannelsQuery {
    owner: Option<String>,
}

pub async fn scan_channels(
    state: web::Data<AppState>,
    query: web::Query<ChannelsQuery>,
) -> Result<HttpResponse, AppError> {
    let channels = match &query.owner {
        Some(hex_str) if !hex_str.is_empty() => {
            let raw = hex::decode(hex_str)
                .map_err(|_| AppError::BadRequest("Invalid hex for owner lock hash".into()))?;
            state.chain_provider.scan_fiber_channels(&raw).await?
        }
        _ => state.chain_provider.scan_fiber_channels(&[]).await?,
    };
    Ok(HttpResponse::Ok().json(channels))
}

// ═══════════════════════════════════════════════════════
// Signing
// ═══════════════════════════════════════════════════════

pub async fn list_unsigned(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let txs = GatewayService::list_unsigned_txs(&state.db)?;
    Ok(HttpResponse::Ok().json(txs))
}

pub async fn get_unsigned(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let tx = GatewayService::get_unsigned_tx(&state.db, &id)?;
    Ok(HttpResponse::Ok().json(tx))
}

#[derive(Deserialize)]
pub struct WitnessBody {
    witnesses: serde_json::Value,
}

pub async fn submit_witnesses(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<WitnessBody>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    GatewayService::submit_witnesses(&state.db, &id, body.witnesses.clone())?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"id": id, "status": "signed"})))
}

pub async fn submit_to_chain(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    GatewayService::submit_to_chain(&state.db, &id)?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"id": id, "status": "broadcast"})))
}

// ═══════════════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════════════

pub async fn get_config(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let cfg = GatewayService::get_config(&state.config);
    Ok(HttpResponse::Ok().json(cfg))
}

#[derive(Deserialize)]
pub struct UpdateConfigBody {
    pub enabled: Option<bool>,
    pub min_capacity_shannons: Option<u64>,
    pub max_escrow_blocks: Option<u64>,
    pub interval_secs: Option<u64>,
}

pub async fn update_config(
    state: web::Data<AppState>,
    body: web::Json<UpdateConfigBody>,
) -> Result<HttpResponse, AppError> {
    let current = GatewayService::get_config(&state.config);
    // Note: config changes require restart to take effect in scheduler loops.
    // This endpoint acknowledges the request and returns the requested values.
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Config update received. Restart required for changes to take effect.",
        "current": current,
        "requested": {
            "enabled": body.enabled,
            "min_capacity_shannons": body.min_capacity_shannons,
            "max_escrow_blocks": body.max_escrow_blocks,
            "interval_secs": body.interval_secs,
        }
    })))
}

// ═══════════════════════════════════════════════════════
// Scheduler
// ═══════════════════════════════════════════════════════

pub async fn scheduler_status(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let s = {
        let guard = state.scheduler_state.read().map_err(|e| {
            AppError::Internal(format!("Scheduler state lock: {}", e))
        })?;
        guard.clone()
    };
    let status = GatewayService::get_scheduler_status(&s);
    Ok(HttpResponse::Ok().json(status))
}

// ═══════════════════════════════════════════════════════
// Signer
// ═══════════════════════════════════════════════════════

pub async fn signer_info(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let info = GatewayService::get_signer_info(state.signer.as_ref());
    Ok(HttpResponse::Ok().json(info))
}
