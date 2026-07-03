//! Console gateway API — single unified surface for the Web Console SPA.
//!
//! Every handler delegates to `GatewayService` methods.
//! All routes are mounted under `/api/console`.

use actix_web::cookie::{Cookie, SameSite};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::api::AppState;
use crate::db::wallets as wallet_db;
use crate::error::AppError;
use crate::services::console::gateway_service::GatewayService;
use crate::services::runtime_config::RuntimeConfigPartial;
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
            .route(
                "/matches/{tx_hash}/{output_index}",
                web::get().to(match_detail),
            )
            .route(
                "/matches/{tx_hash}/{output_index}/extract",
                web::post().to(extract_rent),
            )
            .route(
                "/matches/{tx_hash}/{output_index}/destroy",
                web::post().to(destroy_match),
            )
            // Channels
            .route("/channels", web::get().to(scan_channels))
            .route("/channels-only", web::get().to(scan_channels_only))
            .route("/channel-matches", web::get().to(scan_channel_matches))
            // Fiber node info
            .route("/fiber-node-info", web::get().to(fiber_node_info))
            // Runtime config (mutable at runtime)
            .route("/runtime-config", web::get().to(get_runtime_config))
            .route("/runtime-config", web::put().to(update_runtime_config))
            .route(
                "/runtime-config/reset",
                web::post().to(reset_runtime_config),
            )
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
    let rc = state.runtime_config.read().unwrap();
    let info = GatewayService::get_server_info(&state.config, &rc, state.chain_provider.as_ref());
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
    let mut status = session_status(&state, &req);
    if status.active {
        ensure_signer_from_session(&state, &req)?;
    }
    // The signer may hold loaded keys even when the session cookie is
    // absent or expired (e.g. after explicit unlock with no session).
    // Report the true unlock state so the frontend doesn't mistakenly
    // show a "wallet locked" warning.
    if !status.active && state.signer.is_unlocked() {
        status.active = true;
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

/// Reveal the HD wallet mnemonic phrase. Requires password verification.
/// Unlike unlock_keystore, this does NOT start a wallet session or load
/// the signer — it only decrypts and returns the mnemonic for display.
#[derive(Deserialize)]
pub struct RevealMnemonicBody {
    pub password: String,
}

pub async fn reveal_mnemonic(
    state: web::Data<AppState>,
    body: web::Json<RevealMnemonicBody>,
) -> Result<HttpResponse, AppError> {
    let mnemonic = GatewayService::reveal_mnemonic(
        std::path::Path::new(&state.keystore_path),
        &body.password,
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "mnemonic": mnemonic,
    })))
}

/// Lighter wallet info for the address selector in the match dialog.
/// Omits `encrypted_key` and other internal fields.
#[derive(serde::Serialize)]
pub struct SignerWalletItem {
    pub id: i64,
    pub label: String,
    pub ckb_address: String,
    pub lock_hash: String,
    pub derivation_index: Option<i32>,
    pub derivation_path: Option<String>,
    pub balance_shannons: u64,
}

/// Return the HD wallet addresses with CKB balances for the address selector.
///
/// Auto-unlocks the signer from an active wallet session. Falls back to DB
/// records when the signer is locked so the admin can still browse addresses
/// and unlock directly from the match dialog.
pub async fn signer_wallets(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    // Auto-unlock from session cookie if available (no-op when already unlocked).
    let _ = ensure_signer_from_session(&state, &req);

    // Prefer in-memory signer records; fall back to DB when locked.
    let records = state.signer.wallet_records();
    let records: Vec<wallet_db::WalletRecord> = if records.is_empty() {
        let mut conn = state.db.get()?;
        wallet_db::list_wallets(&mut conn)?
            .into_iter()
            .filter(|w| w.wallet_type == "hd_child")
            .collect()
    } else {
        records
    };

    let mut wallets = Vec::with_capacity(records.len());
    for wr in records {
        let balance = state
            .chain_provider
            .get_balance_by_address(&wr.ckb_address)
            .await
            .unwrap_or(0);
        wallets.push(SignerWalletItem {
            id: wr.id,
            label: wr.label,
            ckb_address: wr.ckb_address.clone(),
            lock_hash: hex::encode(&wr.lock_hash),
            derivation_index: wr.derivation_index,
            derivation_path: wr.derivation_path.clone(),
            balance_shannons: balance,
        });
    }
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
            own_pubkey
                .as_ref()
                .is_none_or(|pk| hex::encode(o.order_args.fiber_pubkey.to_bytes()) != *pk)
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
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    // Try cookie-based session unlock first, then fall back to the
    // configured password (--hd-wallet-password). If the signer is already
    // unlocked both are no-ops.
    if let Err(e) = ensure_signer_from_session(&state, &req) {
        if let Some(ref password) = state.config.hd_wallet_password {
            state.signer.load_keys(&state.db, password)?;
        } else {
            return Err(e);
        }
    }

    let tx_hash = path.into_inner();
    let tx_assembler = state
        .tx_assembler
        .as_ref()
        .ok_or_else(|| AppError::ChainError("Transaction assembler not configured".into()))?;
    let result = GatewayService::match_order(
        &state.db,
        state.chain_provider.as_ref(),
        &tx_hash,
        body.order_output_index,
        &body.seller_address,
        &state.signer,
        tx_assembler,
    )
    .await?;
    Ok(HttpResponse::Ok().json(result))
}

/// Check match readiness for an order (peer connected + compatible channel).
pub async fn match_readiness(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let tx_hash = path.into_inner();
    let status =
        GatewayService::get_match_readiness(state.chain_provider.as_ref(), &state.db, &tx_hash)
            .await?;
    Ok(HttpResponse::Ok().json(status))
}

/// Create a channel for a specific order.
pub async fn create_order_channel(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let tx_hash = path.into_inner();
    let result =
        GatewayService::create_order_channel(state.chain_provider.as_ref(), &tx_hash).await?;
    Ok(HttpResponse::Ok().json(result))
}

// ═══════════════════════════════════════════════════════
// Matches
// ═══════════════════════════════════════════════════════

/// Get a single match with full extraction history.
pub async fn match_detail(
    state: web::Data<AppState>,
    path: web::Path<(String, u32)>,
) -> Result<HttpResponse, AppError> {
    let (tx_hash, output_index) = path.into_inner();
    let detail = GatewayService::get_match_detail(
        &state.db,
        &tx_hash,
        output_index,
        state.chain_provider.as_ref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(detail))
}

#[derive(Deserialize)]
pub struct ListMatchesQuery {
    status: Option<String>,
    /// Comma-separated hex-encoded lock hashes. When provided, only matches
    /// whose seller address maps to one of these lock hashes are returned.
    lock_hashes: Option<String>,
}

pub async fn list_matches(
    state: web::Data<AppState>,
    query: web::Query<ListMatchesQuery>,
) -> Result<HttpResponse, AppError> {
    let signer_lock_hashes: Option<Vec<String>> = query
        .lock_hashes
        .as_ref()
        .map(|s| s.split(',').map(|h| h.trim().to_string()).collect());

    let matches = GatewayService::list_matches(
        &state.db,
        query.status.as_deref(),
        state.chain_provider.as_ref(),
        signer_lock_hashes.as_deref(),
    )
    .await?;

    Ok(HttpResponse::Ok().json(matches))
}

pub async fn extract_rent(
    state: web::Data<AppState>,
    path: web::Path<(String, u32)>,
) -> Result<HttpResponse, AppError> {
    let (tx_hash, output_index) = path.into_inner();
    let min_extraction = state
        .runtime_config
        .read()
        .map(|rc| rc.min_extraction_amount_shannons)
        .unwrap_or(0);
    let result = GatewayService::extract_rent(
        &state.db,
        state.chain_provider.as_ref(),
        &tx_hash,
        output_index,
        state.tx_assembler.as_ref(),
        state.signer.as_ref(),
        min_extraction,
    )
    .await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn destroy_match(
    state: web::Data<AppState>,
    path: web::Path<(String, u32)>,
) -> Result<HttpResponse, AppError> {
    let (tx_hash, output_index) = path.into_inner();
    let tx_hash_result = GatewayService::destroy_match(
        &state.db,
        state.chain_provider.as_ref(),
        &tx_hash,
        output_index,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .json(serde_json::json!({"tx_hash": tx_hash_result, "status": "destroyed"})))
}

// ═══════════════════════════════════════════════════════
// Channels
// ═══════════════════════════════════════════════════════

pub async fn scan_channels(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let channels =
        GatewayService::get_channels_with_matches(&state.db, state.chain_provider.as_ref()).await?;
    Ok(HttpResponse::Ok().json(channels))
}

/// GET /api/console/channels-only — fast path: channels without match cross-referencing.
/// The frontend calls this first to render the channel table immediately.
pub async fn scan_channels_only(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let channels =
        GatewayService::get_channels_only(state.chain_provider.as_ref()).await?;
    Ok(HttpResponse::Ok().json(channels))
}

/// GET /api/console/channel-matches — cross-reference channels with on-chain match cells.
/// The frontend calls this after channels-only to progressively fill in match status.
pub async fn scan_channel_matches(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let cwms =
        GatewayService::get_channel_matches(state.chain_provider.as_ref()).await?;
    Ok(HttpResponse::Ok().json(cwms))
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

pub async fn delete_channel(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let channel_id = path.into_inner();
    GatewayService::delete_channel(&state.db, state.chain_provider.as_ref(), &channel_id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
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
// Runtime config
// ═══════════════════════════════════════════════════════

pub async fn get_runtime_config(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let rc = state.runtime_config.read().unwrap();
    let cfg = GatewayService::get_runtime_config(&rc);
    Ok(HttpResponse::Ok().json(cfg))
}

pub async fn update_runtime_config(
    state: web::Data<AppState>,
    body: web::Json<RuntimeConfigPartial>,
) -> Result<HttpResponse, AppError> {
    let cfg = GatewayService::update_runtime_config(&state.runtime_config, body.into_inner());
    Ok(HttpResponse::Ok().json(cfg))
}

pub async fn reset_runtime_config(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let cfg = GatewayService::reset_runtime_config(&state.runtime_config, &state.config);
    Ok(HttpResponse::Ok().json(cfg))
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
