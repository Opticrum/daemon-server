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
use rust_server::services::cached_chain_provider::CachedChainProvider;
use rust_server::services::chain_cache::ChainCache;
use rust_server::services::console::scheduler_state::{SchedulerState, SharedSchedulerState};
use rust_server::services::hd_wallet_signer::HdWalletSigner;
use rust_server::services::transaction_assembler::TransactionAssembler;
use rust_server::services::wallet_session::WalletSessionManager;
use rust_server::services::RealChainProvider;
use rust_server::services::RuntimeConfig;
use rust_server::{api, db, scheduler};
use std::sync::RwLock;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging — respect RUST_LOG env var, default to info.
    // Set RUST_LOG=debug for verbose output; use --log-level or OPTICRUM_LOG_LEVEL
    // in the config to document the desired level (logged below for visibility).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    // Parse configuration
    let config = Config::load();

    // Initialize database
    let pool = match db::init_db(&config.database_url) {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, path = %config.database_url, "Failed to initialize database");
            std::process::exit(1);
        }
    };

    // Build chain provider (network is auto-detected from RPC URL)
    let mut real_provider = RealChainProvider::new(
        &config.ckb_rpc_url,
        &config.ckb_indexer_url,
        &config.fiber_rpc_url,
    );

    // Resolve Network::Custom(url) → Testnet/Mainnet from on-chain chain_info.
    if let Err(e) = real_provider.update_network().await {
        warn!(error = %e, "Failed to auto-detect network from chain — using URL-based fallback");
    }

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

    let inner_provider: Arc<dyn ChainProvider> = Arc::new(real_provider);

    // Fetch own Fiber node pubkey once at startup (before wrapping with cache).
    let own_fiber_pubkey = inner_provider
        .get_fiber_node_info()
        .await
        .ok()
        .flatten()
        .map(|info| info.pubkey);

    // Runtime-configurable settings — changes take effect immediately
    // without a server restart.
    let runtime_config = Arc::new(RwLock::new(RuntimeConfig::from_config(&config)));

    // Wrap inner provider with transparent cache layer.
    let chain_cache: Arc<ChainCache> = Arc::new(ChainCache::new());
    let cached_chain = Arc::new(CachedChainProvider::new(
        inner_provider.clone(),
        chain_cache.clone(),
        runtime_config.clone(),
    ));
    let chain_provider: Arc<dyn ChainProvider> = cached_chain.clone();

    // Signing: built-in HD wallet (unlock via admin panel)
    let signer: Arc<HdWalletSigner> = Arc::new(HdWalletSigner::new());
    let wallet_session: Arc<WalletSessionManager> = Arc::new(WalletSessionManager::default());

    // Auto-unlock the HD wallet on startup when a password is configured.
    if let Some(ref password) = config.hd_wallet_password {
        match signer.load_keys(&pool, password) {
            Ok(()) => {
                info!(
                    "HD wallet auto-unlocked ({} keys loaded)",
                    signer.wallet_records().len()
                );
            }
            Err(e) => {
                error!(
                    "HD wallet auto-unlock failed: {} — start the server with the \
                     correct --hd-wallet-password or unlock via the admin panel",
                    e
                );
            }
        }
    }

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
    let scheduler_state: SharedSchedulerState =
        Arc::new(std::sync::RwLock::new(SchedulerState::new()));

    // Resolve keystore path to absolute so restarts from a different
    // working directory don't silently create a new keystore elsewhere.
    let keystore_path = {
        let p = std::path::Path::new(&config.keystore_path);
        if p.is_relative() {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(p)
        } else {
            p.to_path_buf()
        }
    };
    // Ensure the parent directory exists (the keystore file itself is
    // created on first use, but create_dir_all is idempotent at startup).
    if let Some(parent) = keystore_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let keystore_path = keystore_path.display().to_string();

    let tx_assembler_for_scheduler = tx_assembler.clone();

    let state = api::AppState {
        db: pool.clone(),
        config: config.clone(),
        runtime_config: runtime_config.clone(),
        chain_provider: chain_provider.clone(),
        cached_chain: cached_chain.clone(),
        signer,
        wallet_session: wallet_session.clone(),
        tx_assembler,
        scheduler_state: scheduler_state.clone(),
        chain_cache: chain_cache.clone(),
        keystore_path,
        own_fiber_pubkey,
    };
    let state = web::Data::new(state);

    // Spawn background tasks
    scheduler::spawn_schedulers(
        pool,
        runtime_config,
        chain_provider,
        inner_provider,
        chain_cache,
        signer_bg,
        tx_assembler_for_scheduler,
        scheduler_state,
    );

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
