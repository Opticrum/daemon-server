//! Background scheduler — spawns the rent extraction loop and auto-matcher.
//!
//! The extractor runs on a configurable interval, scanning the chain
//! for managed matches and automatically extracting rent.
//! The auto-matcher scans for new on-chain orders and matches them
//! against available Fiber channels when enabled.
//!
//! Both loops read `RuntimeConfig` from an `Arc<RwLock<>>` at the start of
//! each cycle, so runtime config changes take effect without a restart.

pub mod auto_matcher;
pub mod rent_extractor;

use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::db::DbPool;
use crate::services::chain_provider::ChainProvider;
use crate::services::console::scheduler_state::{
    record_error, record_success, set_tip_block, SharedSchedulerState,
};
use crate::services::signer::Signer;
use crate::services::RuntimeConfig;

/// Spawn all background tasks: rent extractor and auto-matcher.
pub fn spawn_schedulers(
    pool: DbPool,
    runtime_config: Arc<RwLock<RuntimeConfig>>,
    chain_provider: Arc<dyn ChainProvider>,
    signer: Arc<dyn Signer>,
    scheduler_state: SharedSchedulerState,
) {
    // Rent extractor
    let pool_ext = pool.clone();
    let rc_ext = runtime_config.clone();
    let cp_ext = chain_provider.clone();
    let state_ext = scheduler_state.clone();

    actix_rt::spawn(async move {
        tracing::info!("Rent extractor started");

        loop {
            let rc = rc_ext.read().unwrap();
            let interval = rc.scheduler_interval_secs;
            let min_extraction = rc.min_extraction_amount_shannons;
            drop(rc);

            let started = Instant::now();
            match rent_extractor::run_extraction_cycle(&pool_ext, min_extraction, cp_ext.as_ref())
                .await
            {
                Ok(extracted) => {
                    let elapsed = started.elapsed();
                    record_success(&state_ext, |s| &mut s.extractor, elapsed, extracted);
                    if extracted > 0 {
                        tracing::debug!(extracted, "Extraction cycle");
                    }
                    if let Ok(tip) = cp_ext.get_tip_block_number().await {
                        set_tip_block(&state_ext, tip);
                    }
                }
                Err(e) => {
                    let _elapsed = started.elapsed();
                    record_error(&state_ext, |s| &mut s.extractor, &e.to_string());
                    tracing::error!(error = %e, "Extraction cycle failed");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });

    // Auto-matcher
    let pool_am = pool;
    let rc_am = runtime_config;
    let cp_am = chain_provider;
    let signer_am = signer;
    let state_am = scheduler_state;

    actix_rt::spawn(async move {
        tracing::info!("Auto-matcher started");

        loop {
            let rc = rc_am.read().unwrap();
            let enabled = rc.auto_match_enabled;
            let interval = rc.auto_match_interval_secs;
            drop(rc);

            if !enabled {
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                continue;
            }

            let started = Instant::now();
            match auto_matcher::run_auto_match_cycle(
                &pool_am,
                cp_am.as_ref(),
                signer_am.as_ref(),
                &rc_am,
            )
            .await
            {
                Ok(n) => {
                    let elapsed = started.elapsed();
                    record_success(&state_am, |s| &mut s.matcher, elapsed, n);
                    if n > 0 {
                        tracing::debug!(matched = n, "Auto-match cycle");
                    }
                }
                Err(e) => {
                    let _elapsed = started.elapsed();
                    record_error(&state_am, |s| &mut s.matcher, &e.to_string());
                    tracing::error!(error = %e, "Auto-match cycle failed");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });
}
