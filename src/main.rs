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

    info!("Opticrum Server v{} starting up", env!("CARGO_PKG_VERSION"));

    // Parse configuration
    let config = Config::load();
    info!(
        config = ?config.config_file,
        port = config.port,
        db = %config.database_url,
        ckb_rpc = %config.ckb_rpc_url,
        idx = %config.ckb_indexer_url,
        fiber = %config.fiber_rpc_url,
        fee_rate = config.fee_rate,
        auto_match = config.auto_match_enabled,
        log_level = %config.log_level,
        "Configuration loaded"
    );

    // Initialize database
    let pool = match db::init_db(&config.database_url) {
        Ok(p) => {
            info!(path = %config.database_url, "Database initialized");
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
    match real_provider.get_tip_block_number().await {
        Ok(tip) => info!(tip_block = tip, network = real_provider.network(), "CKB chain connection verified"),
        Err(e) => warn!(error = %e, "CKB chain connectivity check failed — server will start anyway"),
    }

    // Build transaction assembler
    let tx_assembler = Some(TransactionAssembler::new(
        real_provider.rpc_client().clone(),
        config.fee_rate,
    ));
    info!("Transaction assembler initialized (fee_rate={})", config.fee_rate);

    let chain_provider: Arc<dyn ChainProvider> = Arc::new(real_provider);

    // Signing: external by default
    let signer: Arc<dyn Signer> = Arc::new(ExternalSigner::new());
    info!("Signer initialized: mode=external");

    let signer_bg = signer.clone();

    // Build application state
    let state = api::AppState {
        db: pool.clone(),
        config: config.clone(),
        chain_provider: chain_provider.clone(),
        signer,
        tx_assembler,
    };
    let state = web::Data::new(state);

    // Spawn background tasks
    scheduler::spawn_schedulers(pool, config.clone(), chain_provider, signer_bg);
    info!("Background schedulers spawned");

    // Start HTTP server
    let bind_addr = (config.bind_address.as_str(), config.port);
    let admin_url = format!("http://{}:{}/admin", config.bind_address, config.port);
    info!(address = %config.bind_address, port = config.port, admin_url = %admin_url, "HTTP server starting");

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
            info!("Server listening — ready for requests");
            server.run().await
        }
        Err(e) => {
            error!(error = %e, address = %config.bind_address, port = config.port, "Failed to bind");
            Err(e)
        }
    }
}
