//! Background wallet-transaction sync — periodically pulls each managed
//! wallet's latest tx window from the indexer into the SQLite DB.
//!
//! The console reads `wallet_transactions` directly (no live chain query);
//! this loop keeps that table fresh. `POST /api/console/wallets/transactions/sync`
//! runs the same `wallet_tx::sync` on demand for the manual refresh button.

use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::db::DbPool;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::console::scheduler_state::{push_event, SharedSchedulerState};
use crate::services::wallet_tx;
use crate::services::RuntimeConfig;

/// Run one wallet-tx sync cycle. Returns rows synced.
pub async fn run_wallet_tx_sync_cycle(
    pool: &DbPool,
    provider: &(dyn ChainProvider + Send + Sync),
    scheduler_state: Option<&SharedSchedulerState>,
) -> Result<u64, AppError> {
    push_event(
        scheduler_state,
        "wallet_tx",
        "info",
        "Cycle started — syncing wallet transactions",
    );
    let started = Instant::now();
    let stats = wallet_tx::sync(pool, provider).await?;
    push_event(
        scheduler_state,
        "wallet_tx",
        "info",
        format!(
            "Synced {} wallets, {} rows, pruned {} ({} ms)",
            stats.wallets,
            stats.rows_synced,
            stats.pruned,
            started.elapsed().as_millis() as u64
        ),
    );
    Ok(stats.rows_synced as u64)
}

/// Spawn the background loop. Reads enabled/interval live from `RuntimeConfig`
/// each iteration (mirrors `chain_indexer.rs`).
pub fn spawn_wallet_tx_sync(
    pool: DbPool,
    runtime_config: Arc<RwLock<RuntimeConfig>>,
    chain_provider: Arc<dyn ChainProvider>,
    scheduler_state: SharedSchedulerState,
) {
    actix_rt::spawn(async move {
        tracing::info!("Wallet tx sync started");
        loop {
            let (enabled, interval) = {
                let rc = match runtime_config.read() {
                    Ok(rc) => rc,
                    Err(e) => {
                        tracing::error!(
                            "RuntimeConfig lock poisoned: {} — wallet tx sync exiting",
                            e
                        );
                        break;
                    }
                };
                (rc.wallet_tx_sync_enabled, rc.wallet_tx_sync_interval_secs)
            };
            if !enabled {
                tracing::debug!("Wallet tx sync disabled, sleeping");
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                continue;
            }
            match run_wallet_tx_sync_cycle(&pool, chain_provider.as_ref(), Some(&scheduler_state))
                .await
            {
                Ok(_n) => {}
                Err(e) => {
                    let msg = e.to_string();
                    push_event(
                        Some(&scheduler_state),
                        "wallet_tx",
                        "error",
                        format!("Cycle failed — {msg}"),
                    );
                    tracing::error!(error = %e, "Wallet tx sync cycle failed");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });
}
