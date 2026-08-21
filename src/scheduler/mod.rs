//! Background scheduler — spawns the chain indexer, rent extraction loop, and auto-matcher.

pub mod auto_matcher;
pub mod chain_indexer;
pub mod rent_extractor;
pub mod wallet_tx_sync;

use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::db::DbPool;
use crate::services::chain_cache::SharedChainCache;
use crate::services::chain_provider::ChainProvider;
use crate::services::console::scheduler_state::{
    push_event, record_error, record_success, set_tip_block, SharedSchedulerState,
};
use crate::services::hd_wallet_signer::HdWalletSigner;
use crate::services::signer::Signer;
use crate::services::transaction_assembler::TransactionAssembler;
use crate::services::RuntimeConfig;

/// Spawn all background tasks: chain indexer, rent extractor, and auto-matcher.
#[allow(clippy::too_many_arguments)]
pub fn spawn_schedulers(
    pool: DbPool,
    runtime_config: Arc<RwLock<RuntimeConfig>>,
    chain_provider: Arc<dyn ChainProvider>,
    inner_provider: Arc<dyn ChainProvider>,
    chain_cache: SharedChainCache,
    signer: Arc<HdWalletSigner>,
    tx_assembler: Option<TransactionAssembler>,
    scheduler_state: SharedSchedulerState,
) {
    chain_indexer::spawn_chain_indexer(
        chain_cache,
        runtime_config.clone(),
        inner_provider,
        scheduler_state.clone(),
    );

    wallet_tx_sync::spawn_wallet_tx_sync(
        pool.clone(),
        runtime_config.clone(),
        chain_provider.clone(),
        scheduler_state.clone(),
    );

    let pool_ext = pool.clone();
    let rc_ext = runtime_config.clone();
    let cp_ext = chain_provider.clone();
    let state_ext = scheduler_state.clone();
    let signer_ext = signer.clone();
    let tx_assembler_ext = tx_assembler.clone();

    actix_rt::spawn(async move {
        tracing::info!("Rent extractor started");

        loop {
            let (enabled, interval, min_extraction) = {
                let rc = match rc_ext.read() {
                    Ok(rc) => rc,
                    Err(e) => {
                        tracing::error!(
                            "RuntimeConfig lock poisoned: {} — rent extractor exiting",
                            e
                        );
                        break;
                    }
                };
                (
                    rc.rent_extraction_enabled,
                    rc.scheduler_interval_secs,
                    rc.min_extraction_amount_shannons,
                )
            };

            if !enabled {
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                continue;
            }

            let started = Instant::now();
            match rent_extractor::run_extraction_cycle(
                &pool_ext,
                min_extraction,
                cp_ext.as_ref(),
                tx_assembler_ext.as_ref(),
                Some(signer_ext.as_ref()),
                Some(&state_ext),
            )
            .await
            {
                Ok(extracted) => {
                    let elapsed = started.elapsed();
                    record_success(&state_ext, |s| &mut s.extractor, elapsed, extracted);
                    if let Ok(tip) = cp_ext.get_tip_block_number().await {
                        set_tip_block(&state_ext, tip);
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    record_error(&state_ext, |s| &mut s.extractor, &msg);
                    push_event(
                        Some(&state_ext),
                        "extractor",
                        "error",
                        format!("Cycle failed — {msg}"),
                    );
                    tracing::error!(error = %e, "Extraction cycle failed");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });

    let rc_am = runtime_config;
    let cp_am = chain_provider;
    let signer_am: Arc<dyn Signer> = signer;
    let state_am = scheduler_state;

    actix_rt::spawn(async move {
        tracing::info!("Auto-matcher started");

        loop {
            let (enabled, interval) = {
                let rc = match rc_am.read() {
                    Ok(rc) => rc,
                    Err(e) => {
                        tracing::error!(
                            "RuntimeConfig lock poisoned: {} — auto-matcher exiting",
                            e
                        );
                        break;
                    }
                };
                (rc.auto_match_enabled, rc.auto_match_interval_secs)
            };

            if !enabled {
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                continue;
            }

            let started = Instant::now();
            match auto_matcher::run_auto_match_cycle(
                cp_am.as_ref(),
                signer_am.as_ref(),
                &rc_am,
                Some(&state_am),
            )
            .await
            {
                Ok(n) => {
                    record_success(&state_am, |s| &mut s.matcher, started.elapsed(), n);
                }
                Err(e) => {
                    let msg = e.to_string();
                    record_error(&state_am, |s| &mut s.matcher, &msg);
                    push_event(
                        Some(&state_am),
                        "matcher",
                        "error",
                        format!("Cycle failed — {msg}"),
                    );
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });
}
