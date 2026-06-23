//! Background scheduler — spawns the rent extraction loop and auto-matcher.
//!
//! The extractor runs on a configurable interval, scanning the chain
//! for managed matches and automatically extracting rent.
//! The auto-matcher scans for new on-chain orders and matches them
//! against available Fiber channels when enabled.

pub mod auto_matcher;
pub mod rent_extractor;

use std::sync::Arc;

use crate::config::Config;
use crate::db::DbPool;
use crate::services::chain_provider::ChainProvider;
use crate::services::signer::Signer;

/// Spawn all background tasks: rent extractor and auto-matcher.
pub fn spawn_schedulers(
    pool: DbPool,
    config: Config,
    chain_provider: Arc<dyn ChainProvider>,
    signer: Arc<dyn Signer>,
) {
    // Rent extractor
    let pool_ext = pool.clone();
    let _config_ext = config.clone();
    let cp_ext = chain_provider.clone();
    let interval_secs = config.scheduler_interval_secs;
    let min_extraction = config.min_extraction_amount_shannons;

    actix_rt::spawn(async move {
        tracing::info!(
            "Rent extractor started (interval={}s, min_extraction={} shannons)",
            interval_secs,
            min_extraction
        );

        loop {
            match rent_extractor::run_extraction_cycle(
                &pool_ext,
                min_extraction,
                cp_ext.as_ref(),
            )
            .await
            {
                Ok(extracted) => {
                    if extracted > 0 {
                        tracing::info!("Extracted {} shannons this cycle", extracted);
                    }
                }
                Err(e) => {
                    tracing::error!("Extraction cycle error: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
        }
    });

    // Auto-matcher (spawned regardless; checks auto_match_enabled inside loop)
    let pool_am = pool.clone();
    let config_am = config.clone();
    let cp_am = chain_provider.clone();
    let signer_am = signer.clone();
    let am_interval = config.auto_match_interval_secs;
    let am_enabled = config.auto_match_enabled;

    actix_rt::spawn(async move {
        if !am_enabled {
            tracing::info!("Auto-matcher disabled (set auto_match_enabled=true to enable)");
        } else {
            tracing::info!(
                "Auto-matcher started (interval={}s, min_capacity={}, max_escrow={})",
                am_interval,
                config_am.auto_match_min_capacity,
                config_am.auto_match_max_escrow_blocks
            );
        }

        loop {
            if !config_am.auto_match_enabled {
                tokio::time::sleep(std::time::Duration::from_secs(am_interval)).await;
                continue;
            }

            match auto_matcher::run_auto_match_cycle(
                &pool_am,
                cp_am.as_ref(),
                signer_am.as_ref(),
                &config_am,
            )
            .await
            {
                Ok(n) if n > 0 => {
                    tracing::info!("Auto-matched {} orders this cycle", n);
                }
                Ok(_) => {} // zero matches, silent
                Err(e) => {
                    tracing::error!("Auto-match cycle error: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(am_interval)).await;
        }
    });
}
