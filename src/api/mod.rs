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
use crate::services::signer::Signer;
use crate::services::transaction_assembler::TransactionAssembler;

mod admin;
pub mod console;
mod fiber;
mod health;
mod matches;
mod orders;
mod transactions;
mod wallet;

/// Application state shared across all handlers.
pub struct AppState {
    /// SQLite connection pool (Diesel-backed).
    pub db: DbPool,
    /// Server configuration.
    pub config: Config,
    /// Chain provider for CKB RPC and indexer access.
    pub chain_provider: Arc<dyn ChainProvider>,
    /// Signing provider (internal or external).
    pub signer: Arc<dyn Signer>,
    /// Real transaction assembler (None for MockChainProvider test mode).
    pub tx_assembler: Option<TransactionAssembler>,
    /// Shared scheduler state for console observability.
    pub scheduler_state: SharedSchedulerState,
    /// HD wallet keystore file path.
    pub keystore_path: String,
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
            .route("/matches/{id}/extract", web::post().to(matches::extract))
            .route("/matches/{id}/destroy", web::post().to(matches::destroy))
            .route("/fiber/channels", web::get().to(fiber::list_channels))
            .route(
                "/transactions/unsigned",
                web::get().to(transactions::list_unsigned),
            )
            .route(
                "/transactions/unsigned/{id}",
                web::get().to(transactions::get_unsigned),
            )
            .route(
                "/transactions/unsigned/{id}/witnesses",
                web::post().to(transactions::submit_witnesses),
            )
            .route(
                "/transactions/unsigned/{id}/submit",
                web::post().to(transactions::submit_to_chain),
            )
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
                "/console/wallets/derive-more",
                web::post().to(console::derive_more_addresses),
            )
            .route(
                "/console/wallets/import-mnemonic",
                web::post().to(console::import_mnemonic),
            )
            .route("/console/wallets/hd-status", web::get().to(console::hd_status))
            .route("/console/wallets/balance", web::get().to(console::hd_balance))
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
            // Catch-all: delete individual wallet by id
            .route(
                "/console/wallets/{id}",
                web::delete().to(console::delete_wallet),
            )
            .route("/console/orders", web::get().to(console::scan_orders))
            .route(
                "/console/orders/{tx_hash}/match",
                web::post().to(console::match_order),
            )
            .route("/console/matches", web::get().to(console::list_matches))
            .route(
                "/console/matches/{id}/extract",
                web::post().to(console::extract_rent),
            )
            .route(
                "/console/matches/{id}/destroy",
                web::post().to(console::destroy_match),
            )
            .route("/console/channels", web::get().to(console::scan_channels))
            .route("/console/signing", web::get().to(console::list_unsigned))
            .route(
                "/console/signing/{id}",
                web::get().to(console::get_unsigned),
            )
            .route(
                "/console/signing/{id}/witnesses",
                web::post().to(console::submit_witnesses),
            )
            .route(
                "/console/signing/{id}/submit",
                web::post().to(console::submit_to_chain),
            )
            .route("/console/config", web::get().to(console::get_config))
            .route("/console/config", web::put().to(console::update_config))
            .route(
                "/console/scheduler/status",
                web::get().to(console::scheduler_status),
            )
            .route("/console/signer-info", web::get().to(console::signer_info))
            .route("/console/server-info", web::get().to(console::server_info))
            .route(
                "/console/fiber-node-info",
                web::get().to(console::fiber_node_info),
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
