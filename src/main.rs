//! Opticrum Rust Server — REST API and background service for the
//! Opticrum decentralized liquidity marketplace.
//!
//! Provides HTTP endpoints for creating/canceling/matching orders,
//! extracting rent, and managing wallets. A background scheduler
//! automatically extracts rent from managed matches.

use actix_web::{web, App, HttpServer};
use std::sync::Arc;
use tracing::{error, info, warn};

use rust_server::config::Config;
use rust_server::services::chain_provider::ChainProvider;
use rust_server::services::console::scheduler_state::{SchedulerState, SharedSchedulerState};
use rust_server::services::external_signer::ExternalSigner;
use rust_server::services::signer::Signer;
use rust_server::services::transaction_assembler::TransactionAssembler;
use rust_server::services::RealChainProvider;
use rust_server::{api, db, scheduler};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging — respect RUST_LOG env var, default to info.
    // Set RUST_LOG=debug for verbose output; use --log-level or OPTICRUM_LOG_LEVEL
    // in the config to document the desired level (logged below for visibility).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    // Parse configuration
    let config = Config::load();

    // Initialize database
    let pool = match db::init_db(&config.database_url) {
        Ok(p) => {
            p
        }
        Err(e) => {
            error!(error = %e, path = %config.database_url, "Failed to initialize database");
            std::process::exit(1);
        }
    };

    // Build chain provider (network is auto-detected from RPC URL)
    let real_provider = RealChainProvider::new(
        &config.ckb_rpc_url,
        &config.ckb_indexer_url,
        &config.fiber_rpc_url,
    );

    // Verify chain connectivity
    let tip_block = match real_provider.get_tip_block_number().await {
        Ok(tip) => {
            info!(tip, network = real_provider.network(), "Chain connected");
            tip
        }
        Err(e) => {
            warn!(error = %e, "Chain connectivity check failed — starting anyway");
            0
        }
    };

    // Build transaction assembler (not logged — chain + signer cover the infra)
    let tx_assembler = Some(TransactionAssembler::new(
        real_provider.rpc_client().clone(),
        config.fee_rate,
    ));

    let chain_provider: Arc<dyn ChainProvider> = Arc::new(real_provider);

    // Signing: external by default, network-aware
    let signer: Arc<dyn Signer> = Arc::new(ExternalSigner::new(
        chain_provider.network(),
    ));

    // Consolidated startup summary
    info!(
        version = env!("CARGO_PKG_VERSION"),
        port = config.port,
        network = chain_provider.network(),
        db = %config.database_url,
        fiber = %config.fiber_rpc_url,
        auto_match = config.auto_match_enabled,
        tip_block,
        "Opticrum Server starting"
    );

    let signer_bg = signer.clone();

    // Shared scheduler state for console observability
    let scheduler_state: SharedSchedulerState = Arc::new(std::sync::RwLock::new(SchedulerState::new()));

    // Build application state
    let state = api::AppState {
        db: pool.clone(),
        config: config.clone(),
        chain_provider: chain_provider.clone(),
        signer,
        tx_assembler,
        scheduler_state: scheduler_state.clone(),
    };
    let state = web::Data::new(state);

    // Spawn background tasks
    scheduler::spawn_schedulers(pool, config.clone(), chain_provider, signer_bg, scheduler_state);

    // Start HTTP server
    let bind_addr = (config.bind_address.as_str(), config.port);

    match HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(api::RequestLogger)
            .configure(api::configure_routes)
            .service(actix_files::Files::new("/admin", "static").index_file("index.html"))
    })
    .bind(bind_addr)
    {
        Ok(server) => {
            info!(
                address = %format!("http://{}:{}/admin", config.bind_address, config.port),
                "Server ready"
            );
            server.run().await
        }
        Err(e) => {
            error!(error = %e, address = %config.bind_address, port = config.port, "Failed to bind");
            Err(e)
        }
    }
}
