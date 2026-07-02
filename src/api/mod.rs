//! API module — actix-web route configuration and shared state.
//!
//! Defines `AppState` (injected into all handlers via `web::Data`),
//! request logging middleware, and `configure_routes`.

use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    web,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, warn};

use crate::config::Config;
use crate::db::DbPool;
use crate::services::chain_provider::ChainProvider;
use crate::services::console::scheduler_state::SharedSchedulerState;
use crate::services::hd_wallet_signer::HdWalletSigner;
use crate::services::transaction_assembler::TransactionAssembler;
use crate::services::wallet_session::WalletSessionManager;
use crate::services::RuntimeConfig;
use std::sync::RwLock;

mod admin;
pub mod console;
mod fiber;
mod health;
mod matches;
mod orders;
mod wallet;

/// Application state shared across all handlers.
pub struct AppState {
    /// SQLite connection pool (Diesel-backed).
    pub db: DbPool,
    /// Server configuration (immutable — restart required for changes).
    pub config: Config,
    /// Runtime-configurable settings (fee rate, extraction, auto-match, etc.).
    /// Backed by `Arc<RwLock<>>` so changes take effect immediately.
    pub runtime_config: Arc<RwLock<RuntimeConfig>>,
    /// Chain provider for CKB RPC and indexer access.
    pub chain_provider: Arc<dyn ChainProvider>,
    /// HD wallet signing provider (unlock via admin panel).
    pub signer: Arc<HdWalletSigner>,
    /// In-memory unlock session (1-hour HttpOnly cookie).
    pub wallet_session: Arc<WalletSessionManager>,
    /// Real transaction assembler (None for MockChainProvider test mode).
    pub tx_assembler: Option<TransactionAssembler>,
    /// Shared scheduler state for console observability.
    pub scheduler_state: SharedSchedulerState,
    /// HD wallet keystore file path.
    pub keystore_path: String,
    /// Own Fiber node pubkey (cached at startup) — used to filter out
    /// self-owned orders from chain scan results.
    pub own_fiber_pubkey: Option<String>,
}

/// Mount all API routes on the given `ServiceConfig`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/health", web::get().to(health::check))
            .route("/wallets", web::get().to(wallet::list))
            .route("/wallets", web::post().to(wallet::import_key))
            .route("/wallets/{id}", web::delete().to(wallet::delete))
            .route("/orders/scan", web::get().to(orders::scan_chain))
            .route("/orders/{tx_hash}/match", web::post().to(orders::do_match))
            .route("/matches/scan", web::get().to(matches::scan_chain))
            .route("/matches", web::get().to(matches::list))
            .route(
                "/matches/{tx_hash}/{output_index}/extract",
                web::post().to(matches::extract),
            )
            .route(
                "/matches/{tx_hash}/{output_index}/destroy",
                web::post().to(matches::destroy),
            )
            .route("/fiber/channels", web::get().to(fiber::list_channels))
            .route("/admin/stats", web::get().to(admin::stats))
            .route(
                "/admin/auto-match/config",
                web::get().to(admin::get_auto_match_config),
            )
            .route(
                "/admin/auto-match/config",
                web::put().to(admin::update_auto_match_config),
            )
            // Console gateway
            .route("/console/dashboard", web::get().to(console::dashboard))
            .route("/console/wallets", web::get().to(console::list_wallets))
            .route("/console/wallets", web::post().to(console::import_wallet))
            // Named routes must come BEFORE the catch-all {id} route
            .route(
                "/console/wallets/create-hd",
                web::post().to(console::create_hd_wallet),
            )
            .route(
                "/console/wallets/unlock",
                web::post().to(console::unlock_keystore),
            )
            .route(
                "/console/wallets/session",
                web::get().to(console::wallet_session),
            )
            .route(
                "/console/wallets/lock",
                web::post().to(console::lock_wallet),
            )
            .route(
                "/console/wallets/derive-more",
                web::post().to(console::derive_more_addresses),
            )
            .route(
                "/console/wallets/import-mnemonic",
                web::post().to(console::import_mnemonic),
            )
            .route(
                "/console/wallets/hd-status",
                web::get().to(console::hd_status),
            )
            .route(
                "/console/wallets/balance",
                web::get().to(console::hd_balance),
            )
            .route(
                "/console/wallets/balances",
                web::get().to(console::hd_address_balances),
            )
            .route(
                "/console/wallets/refresh-hd",
                web::post().to(console::refresh_hd_wallet),
            )
            .route(
                "/console/wallets/delete-hd",
                web::delete().to(console::delete_hd_wallet),
            )
            .route(
                "/console/wallets/reveal-mnemonic",
                web::post().to(console::reveal_mnemonic),
            )
            .route(
                "/console/signer/wallets",
                web::get().to(console::signer_wallets),
            )
            // Catch-all: delete individual wallet by id
            .route(
                "/console/wallets/{id}",
                web::delete().to(console::delete_wallet),
            )
            .route("/console/orders", web::get().to(console::scan_orders))
            .route(
                "/console/orders/{tx_hash}/match-readiness",
                web::get().to(console::match_readiness),
            )
            .route(
                "/console/orders/{tx_hash}/create-channel",
                web::post().to(console::create_order_channel),
            )
            .route(
                "/console/orders/{tx_hash}/match",
                web::post().to(console::match_order),
            )
            .route("/console/matches", web::get().to(console::list_matches))
            .route(
                "/console/matches/{tx_hash}/{output_index}",
                web::get().to(console::match_detail),
            )
            .route(
                "/console/matches/{tx_hash}/{output_index}/extract",
                web::post().to(console::extract_rent),
            )
            .route(
                "/console/matches/{tx_hash}/{output_index}/destroy",
                web::post().to(console::destroy_match),
            )
            .route("/console/channels", web::get().to(console::scan_channels))
            .route(
                "/console/channels/{channel_id}/close",
                web::post().to(console::close_channel),
            )
            .route(
                "/console/channels/{channel_id}",
                web::delete().to(console::delete_channel),
            )
            .route(
                "/console/scheduler/status",
                web::get().to(console::scheduler_status),
            )
            .route("/console/server-info", web::get().to(console::server_info))
            // Runtime config (mutable at runtime)
            .route(
                "/console/runtime-config",
                web::get().to(console::get_runtime_config),
            )
            .route(
                "/console/runtime-config",
                web::put().to(console::update_runtime_config),
            )
            .route(
                "/console/runtime-config/reset",
                web::post().to(console::reset_runtime_config),
            )
            .route(
                "/console/fiber-node-info",
                web::get().to(console::fiber_node_info),
            )
            .route(
                "/console/peers/check/{pubkey}",
                web::get().to(console::check_peer_connection),
            )
            .route(
                "/console/peers/connect",
                web::post().to(console::connect_to_peer),
            ),
    );
}

// ---------------------------------------------------------------------------
// Request logging middleware
// ---------------------------------------------------------------------------

/// Logs every HTTP request with method, path, status, and duration.
///
/// Usage: `.wrap(api::RequestLogger)` in the actix `App` builder.
pub struct RequestLogger;

impl<S, B> actix_web::dev::Transform<S, ServiceRequest> for RequestLogger
where
    S: actix_web::dev::Service<ServiceRequest, Response = ServiceResponse<B>> + 'static,
    S::Error: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Transform = RequestLoggerMiddleware<S>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(RequestLoggerMiddleware {
            service: std::rc::Rc::new(service),
        }))
    }
}

pub struct RequestLoggerMiddleware<S> {
    service: std::rc::Rc<S>,
}

impl<S, B> actix_web::dev::Service<ServiceRequest> for RequestLoggerMiddleware<S>
where
    S: actix_web::dev::Service<ServiceRequest, Response = ServiceResponse<B>> + 'static,
    S::Error: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Future =
        futures_util::future::LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let started = Instant::now();
        let method = req.method().to_string();
        let path = req.path().to_string();
        let service = self.service.clone();

        Box::pin(async move {
            match service.call(req).await {
                Ok(res) => {
                    let elapsed = started.elapsed();
                    let status = res.status().as_u16();
                    match status {
                        200..=299 | 304 => debug!(
                            method = %method, path = %path,
                            status = status, duration_ms = elapsed.as_millis() as u64,
                            "OK"
                        ),
                        400..=499 => warn!(
                            method = %method, path = %path,
                            status = status, duration_ms = elapsed.as_millis() as u64,
                            "Client error"
                        ),
                        _ => error!(
                            method = %method, path = %path,
                            status = status, duration_ms = elapsed.as_millis() as u64,
                            "Server error"
                        ),
                    }
                    Ok(res)
                }
                Err(_e) => {
                    let elapsed = started.elapsed();
                    error!(
                        method = %method, path = %path,
                        duration_ms = elapsed.as_millis() as u64,
                        "Request pipeline error"
                    );
                    Err(_e)
                }
            }
        })
    }
}
