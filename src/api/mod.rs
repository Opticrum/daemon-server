//! API module — actix-web route configuration and shared state.
//!
//! Defines `AppState` (injected into all handlers via `web::Data`),
//! request logging middleware, and `configure_routes`.

use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    web,
};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::services::chain_provider::ChainProvider;
use crate::services::signer::Signer;
use crate::services::transaction_assembler::TransactionAssembler;

mod admin;
mod fiber;
mod health;
mod matches;
mod orders;
mod transactions;
mod wallet;

/// Application state shared across all handlers.
pub struct AppState {
    /// SQLite connection pool.
    pub db: Pool<SqliteConnectionManager>,
    /// Server configuration.
    pub config: Config,
    /// Chain provider for CKB RPC and indexer access.
    pub chain_provider: Arc<dyn ChainProvider>,
    /// Signing provider (internal or external).
    pub signer: Arc<dyn Signer>,
    /// Real transaction assembler (None for MockChainProvider test mode).
    pub tx_assembler: Option<TransactionAssembler>,
}

/// Mount all API routes on the given `ServiceConfig`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/health", web::get().to(health::check))
            // Wallet management
            .route("/wallets", web::get().to(wallet::list))
            .route("/wallets", web::post().to(wallet::import_key))
            .route("/wallets/{id}", web::delete().to(wallet::delete))
            // Orders
            .route("/orders/scan", web::get().to(orders::scan_chain))
            .route("/orders", web::get().to(orders::list))
            .route("/orders", web::post().to(orders::create))
            .route("/orders/{id}/cancel", web::post().to(orders::cancel))
            .route("/orders/{id}/match", web::post().to(orders::do_match))
            // Matches
            .route("/matches/scan", web::get().to(matches::scan_chain))
            .route("/matches", web::get().to(matches::list))
            .route("/matches/{id}/extract", web::post().to(matches::extract))
            .route("/matches/{id}/destroy", web::post().to(matches::destroy))
            // Fiber channels
            .route("/fiber/channels", web::get().to(fiber::list_channels))
            // External signing
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
            // Admin
            .route("/admin/stats", web::get().to(admin::stats))
            .route(
                "/admin/auto-match/config",
                web::get().to(admin::get_auto_match_config),
            )
            .route(
                "/admin/auto-match/config",
                web::put().to(admin::update_auto_match_config),
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
                        200..=299 => info!(
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
