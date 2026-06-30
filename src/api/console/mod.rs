//! Console gateway API — single unified surface for the Web Console SPA.
//!
//! Every handler delegates to `GatewayService` methods.
//! All routes are mounted under `/api/console`.

use actix_web::cookie::{Cookie, SameSite};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::api::AppState;
use crate::error::AppError;
use crate::services::console::gateway_service::GatewayService;
use crate::services::wallet_session::{SessionStatus, SESSION_COOKIE, SESSION_TTL_SECS};

fn session_token(req: &HttpRequest) -> Option<String> {
    req.cookie(SESSION_COOKIE)
        .map(|cookie| cookie.value().to_string())
}

fn session_cookie(token: &str) -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE, token.to_string())
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(actix_web::cookie::time::Duration::seconds(
            SESSION_TTL_SECS as i64,
        ))
        .finish()
}

fn clear_session_cookie() -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE, "")
        .path("/")
        .http_only(true)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .finish()
}

fn unlock_signer(state: &web::Data<AppState>, password: &str) -> Result<(), AppError> {
    state.signer.load_keys(&state.db, password)
}

fn start_wallet_session(state: &web::Data<AppState>, password: &str) -> Result<String, AppError> {
    unlock_signer(state, password)?;
    Ok(state.wallet_session.create(password.to_string()))
}

/// Clear both the wallet session and the in-memory signer keys.
/// Called on explicit lock or HD wallet deletion.
fn end_wallet_session(state: &web::Data<AppState>) {
    state.wallet_session.clear();
    state.signer.clear();
}

/// Clear only the wallet session — leaves the signer keys in memory so the
/// background auto-matcher can keep signing without re-unlock.
fn clear_session_only(state: &web::Data<AppState>) {
    state.wallet_session.clear();
}

fn resolve_password(
    state: &web::Data<AppState>,
    req: &HttpRequest,
    body_password: Option<&str>,
) -> Result<String, AppError> {
    if let Some(password) = body_password {
        if !password.is_empty() {
            return Ok(password.to_string());
        }
    }
    if let Some(token) = session_token(req) {
        if let Some(password) = state.wallet_session.password_for(&token) {
            return Ok(password);
        }
    }
    Err(AppError::WalletError(
        "Wallet locked — unlock required".into(),
    ))
}

/// Check session status without destructive side effects.
/// The `WalletSessionManager::status` internally clears expired sessions;
/// we do NOT clear the signer here — that only happens on explicit lock/delete.
fn session_status(state: &web::Data<AppState>, req: &HttpRequest) -> SessionStatus {
    let token = session_token(req);
    state.wallet_session.status(token.as_deref())
}

/// Ensure the HD wallet signer is loaded into memory.
///
/// If the session is active but the signer happened to be cleared (e.g. server
/// restart while session cookie is still valid), this auto-restores the keys
/// using the stored password.
///
/// If the session is inactive, the signer is left untouched — it may still hold
/// keys from a previous unlock so the background auto-matcher keeps working.
fn ensure_signer_from_session(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<(), AppError> {
    // Always touch the session to extend its TTL (sliding expiration).
    // This keeps the session alive as long as the user is actively browsing,
    // even when the signer is already loaded from a previous unlock.
    if let Some(token) = session_token(req) {
        let _ = state.wallet_session.touch(&token);
    }

    // Already unlocked — nothing to do.
    if state.signer.is_unlocked() {
        return Ok(());
    }

    // Re-unlock from session: retrieve the stored password, decrypt keys.
    let token = session_token(req)
        .ok_or_else(|| AppError::WalletError("Wallet locked — unlock required".into()))?;

    let password = state.wallet_session.password_for(&token).ok_or_else(|| {
        // Session expired or invalid — clear only the session, not the signer.
        clear_session_only(state);
        AppError::WalletError("Wallet session expired — unlock required".into())
    })?;

    unlock_signer(state, &password)
}

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
            // Fiber node info
            .route("/fiber-node-info", web::get().to(fiber_node_info))
            // Config
            .route("/config", web::get().to(get_config))
            // Scheduler
            .route("/scheduler/status", web::get().to(scheduler_status))
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
        let guard = state
            .scheduler_state
            .read()
            .map_err(|e| AppError::Internal(format!("Scheduler state lock: {}", e)))?;
        guard.clone()
    };
    let dash = GatewayService::get_dashboard(&state.db, state.chain_provider.as_ref(), &s).await?;
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
// HD Wallet
// ═══════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct CreateHdWalletBody {
    pub label: String,
    pub password: String,
    pub address_count: Option<u32>,
}

pub async fn create_hd_wallet(
    state: web::Data<AppState>,
    body: web::Json<CreateHdWalletBody>,
) -> Result<HttpResponse, AppError> {
    let result = GatewayService::create_hd_wallet(
        &state.db,
        std::path::Path::new(&state.keystore_path),
        &body.label,
        &body.password,
        body.address_count.unwrap_or(5),
    )?;
    let token = start_wallet_session(&state, &body.password)?;
    Ok(HttpResponse::Created()
        .cookie(session_cookie(&token))
        .json(result))
}

#[derive(Deserialize)]
pub struct UnlockWalletBody {
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshWalletBody {
    pub password: Option<String>,
}

pub async fn unlock_keystore(
    state: web::Data<AppState>,
    body: web::Json<UnlockWalletBody>,
) -> Result<HttpResponse, AppError> {
    let result = GatewayService::unlock_keystore(
        &state.db,
        std::path::Path::new(&state.keystore_path),
        &body.password,
    )?;
    let token = start_wallet_session(&state, &body.password)?;
    Ok(HttpResponse::Ok()
        .cookie(session_cookie(&token))
        .json(result))
}

pub async fn wallet_session(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let status = session_status(&state, &req);
    if status.active {
        ensure_signer_from_session(&state, &req)?;
    }
    Ok(HttpResponse::Ok().json(status))
}

pub async fn lock_wallet(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    end_wallet_session(&state);
    Ok(HttpResponse::Ok()
        .cookie(clear_session_cookie())
        .json(serde_json::json!({ "locked": true })))
}

#[derive(Deserialize)]
pub struct DeriveMoreBody {
    pub password: Option<String>,
    pub count: Option<u32>,
}

pub async fn derive_more_addresses(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<DeriveMoreBody>,
) -> Result<HttpResponse, AppError> {
    let password = resolve_password(&state, &req, body.password.as_deref())?;
    let records = GatewayService::derive_more_addresses(
        &state.db,
        std::path::Path::new(&state.keystore_path),
        &password,
        body.count.unwrap_or(5),
    )?;
    unlock_signer(&state, &password)?;
    if session_token(&req).is_none() {
        let token = start_wallet_session(&state, &password)?;
        return Ok(HttpResponse::Ok()
            .cookie(session_cookie(&token))
            .json(records));
    }
    Ok(HttpResponse::Ok().json(records))
}

pub async fn hd_status(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let status = GatewayService::get_hd_status(std::path::Path::new(&state.keystore_path));
    Ok(HttpResponse::Ok().json(status))
}

pub async fn hd_balance(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let balance = GatewayService::get_hd_balance(&state.db, state.chain_provider.as_ref()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "total_balance_shannons": balance,
    })))
}

pub async fn hd_address_balances(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let balances =
        GatewayService::get_hd_address_balances(&state.db, state.chain_provider.as_ref()).await?;
    Ok(HttpResponse::Ok().json(balances))
}

pub async fn refresh_hd_wallet(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<RefreshWalletBody>,
) -> Result<HttpResponse, AppError> {
    let password = resolve_password(&state, &req, body.password.as_deref())?;
    let result = GatewayService::refresh_hd_wallet(
        &state.db,
        std::path::Path::new(&state.keystore_path),
        &password,
        state.chain_provider.as_ref(),
    )
    .await?;
    unlock_signer(&state, &password)?;
    if session_token(&req).is_none() {
        let token = start_wallet_session(&state, &password)?;
        return Ok(HttpResponse::Ok()
            .cookie(session_cookie(&token))
            .json(result));
    }
    Ok(HttpResponse::Ok().json(result))
}

#[derive(Deserialize)]
pub struct ImportMnemonicBody {
    pub mnemonic: String,
    pub label: String,
    pub password: String,
    pub address_count: Option<u32>,
}

pub async fn import_mnemonic(
    state: web::Data<AppState>,
    body: web::Json<ImportMnemonicBody>,
) -> Result<HttpResponse, AppError> {
    let result = GatewayService::import_mnemonic(
        &state.db,
        std::path::Path::new(&state.keystore_path),
        &body.mnemonic,
        &body.label,
        &body.password,
        body.address_count.unwrap_or(5),
    )?;
    let token = start_wallet_session(&state, &body.password)?;
    Ok(HttpResponse::Created()
        .cookie(session_cookie(&token))
        .json(result))
}

pub async fn delete_hd_wallet(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    GatewayService::delete_hd_wallet(&state.db, std::path::Path::new(&state.keystore_path))?;
    end_wallet_session(&state);
    Ok(HttpResponse::Ok()
        .cookie(clear_session_cookie())
        .json(serde_json::json!({"deleted": true})))
}

/// Lighter wallet info for the address selector in the match dialog.
/// Omits `encrypted_key` and other internal fields.
#[derive(serde::Serialize)]
pub struct SignerWalletItem {
    pub id: i64,
    pub label: String,
    pub ckb_address: String,
    pub derivation_index: Option<i32>,
    pub derivation_path: Option<String>,
}

/// Return the currently loaded (unlocked) HD wallet addresses for the
/// match-order address selector. Returns an empty array when the signer
/// is locked or no HD wallet is configured.
pub async fn signer_wallets(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let wallets: Vec<SignerWalletItem> = state
        .signer
        .wallet_records()
        .into_iter()
        .map(|wr| SignerWalletItem {
            id: wr.id,
            label: wr.label,
            ckb_address: wr.ckb_address,
            derivation_index: wr.derivation_index,
            derivation_path: wr.derivation_path,
        })
        .collect();
    Ok(HttpResponse::Ok().json(wallets))
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
    let own_pubkey = &state.own_fiber_pubkey;
    let items: Vec<OrderScanItem> = orders
        .into_iter()
        .filter(|o| {
            // Filter out orders belonging to our own Fiber node.
            own_pubkey.as_ref().is_none_or(|pk| {
                hex::encode(o.order_args.fiber_pubkey.to_bytes()) != *pk
            })
        })
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
    pub order_output_index: u32,
    pub seller_address: String,
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
    let tx_hash =
        GatewayService::destroy_match(&state.db, state.chain_provider.as_ref(), id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"tx_hash": tx_hash, "status": "destroyed"})))
}

// ═══════════════════════════════════════════════════════
// Channels
// ═══════════════════════════════════════════════════════

pub async fn scan_channels(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let channels = GatewayService::get_channels_with_matches(state.chain_provider.as_ref()).await?;
    Ok(HttpResponse::Ok().json(channels))
}

#[derive(Deserialize)]
pub struct CloseChannelBody {
    pub force: Option<bool>,
}

pub async fn close_channel(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<CloseChannelBody>,
) -> Result<HttpResponse, AppError> {
    let channel_id = path.into_inner();
    GatewayService::close_channel(
        state.chain_provider.as_ref(),
        &channel_id,
        body.force.unwrap_or(false),
    )
    .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"closed": true})))
}

// ═══════════════════════════════════════════════════════
// Peer connection
// ═══════════════════════════════════════════════════════

pub async fn check_peer_connection(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let pubkey = path.into_inner();
    let peers = state.chain_provider.list_peers().await?;
    let connected = peers.iter().any(|p| p.pubkey == pubkey);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "connected": connected,
        "pubkey": pubkey,
    })))
}

#[derive(Deserialize)]
pub struct ConnectPeerBody {
    pub pubkey: String,
}

pub async fn connect_to_peer(
    state: web::Data<AppState>,
    body: web::Json<ConnectPeerBody>,
) -> Result<HttpResponse, AppError> {
    state.chain_provider.connect_peer(&body.pubkey).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"connected": true})))
}

// ═══════════════════════════════════════════════════════
// Fiber node info
// ═══════════════════════════════════════════════════════

pub async fn fiber_node_info(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let node_info = GatewayService::get_fiber_node_info(state.chain_provider.as_ref()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "rpc_url": state.config.fiber_rpc_url,
        "node_info": node_info,
    })))
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
        let guard = state
            .scheduler_state
            .read()
            .map_err(|e| AppError::Internal(format!("Scheduler state lock: {}", e)))?;
        guard.clone()
    };
    let status = GatewayService::get_scheduler_status(&s);
    Ok(HttpResponse::Ok().json(status))
}
