//! Background chain indexer — keeps the in-memory chain cache warm.
//!
//! Runs on a configurable interval (default 30s), performing a parallel
//! scan of orders, matches, and tip block. Fiber channels are fetched live
//! on demand (e.g. when the Channels page loads), not by this indexer.

use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::error::AppError;
use crate::services::chain_cache::SharedChainCache;
use crate::services::chain_provider::ChainProvider;
use crate::services::console::scheduler_state::{
    push_event, record_error, record_success, set_tip_block, SharedSchedulerState,
};
use crate::services::RuntimeConfig;

/// Run one chain cache refresh cycle.
pub async fn run_indexer_cycle(
    cache: &SharedChainCache,
    inner: &(dyn ChainProvider + Send + Sync),
    scheduler_state: Option<&SharedSchedulerState>,
) -> Result<u64, AppError> {
    push_event(
        scheduler_state,
        "indexer",
        "info",
        "Cycle started — refreshing chain cache",
    );

    let started = Instant::now();
    cache.refresh(inner).await?;

    let status = cache.status();
    push_event(
        scheduler_state,
        "indexer",
        "info",
        format!(
            "Cache updated — {} orders, {} matches ({} extraction histories), tip {}",
            status.order_count, status.match_count, status.extraction_chain_count, status.tip_block
        ),
    );

    if status.tip_block > 0 {
        if let Some(state) = scheduler_state {
            set_tip_block(state, status.tip_block);
        }
    }

    let total = status.order_count + status.match_count;
    let _elapsed = started.elapsed();
    Ok(total)
}

/// Spawn the chain indexer background loop.
pub fn spawn_chain_indexer(
    cache: SharedChainCache,
    runtime_config: Arc<RwLock<RuntimeConfig>>,
    inner_provider: Arc<dyn ChainProvider>,
    scheduler_state: SharedSchedulerState,
) {
    actix_rt::spawn(async move {
        tracing::info!("Chain indexer started");

        loop {
            let (enabled, interval) = {
                let rc = match runtime_config.read() {
                    Ok(rc) => rc,
                    Err(e) => {
                        tracing::error!(
                            "RuntimeConfig lock poisoned: {} — chain indexer exiting",
                            e
                        );
                        break;
                    }
                };
                (rc.chain_cache_enabled, rc.chain_cache_interval_secs)
            };

            if !enabled {
                tracing::debug!("Chain cache disabled, sleeping");
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                continue;
            }

            let started = Instant::now();
            match run_indexer_cycle(&cache, inner_provider.as_ref(), Some(&scheduler_state)).await {
                Ok(processed) => {
                    let elapsed = started.elapsed();
                    record_success(&scheduler_state, |s| &mut s.indexer, elapsed, processed);
                }
                Err(e) => {
                    let msg = e.to_string();
                    record_error(&scheduler_state, |s| &mut s.indexer, &msg);
                    push_event(
                        Some(&scheduler_state),
                        "indexer",
                        "error",
                        format!("Cycle failed — {msg}"),
                    );
                    tracing::error!(error = %e, "Chain indexer cycle failed");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });
}
