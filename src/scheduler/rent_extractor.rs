//! Rent extraction loop — finds managed matches and auto-extracts rent.
//!
//! Runs periodically, scanning the chain for match cells owned by managed
//! wallets and extracting rent when above the dust threshold.
//!
//! After the chain-first refactor, match data comes directly from
//! `scan_matches()` — no database match table is involved.

use tracing::{debug, info, warn};

use crate::db::wallets as wallet_db;
use crate::db::DbPool;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::hd_wallet_signer::HdWalletSigner;
use crate::services::rent_service::{self, preview_extractable_from_chain, ExtractRentOptions};
use crate::services::transaction_assembler::TransactionAssembler;

/// Run one extraction cycle.
///
/// Returns the total amount of shannons extracted in this cycle.
pub async fn run_extraction_cycle(
    pool: &DbPool,
    min_extraction_amount_shannons: u64,
    provider: &(dyn ChainProvider + Send + Sync),
    tx_assembler: Option<&TransactionAssembler>,
    signer: Option<&HdWalletSigner>,
) -> Result<u64, AppError> {
    let mut conn = pool.get()?;

    let wallets = wallet_db::list_wallets(&mut conn)?;
    if wallets.is_empty() {
        debug!("Rent extraction: no managed wallets — skipping cycle");
        return Ok(0);
    }

    if tx_assembler.is_some() {
        let unlocked = signer.map(HdWalletSigner::is_unlocked).unwrap_or(false);
        if !unlocked {
            debug!("Rent extraction: HD wallet locked — skipping cycle");
            return Ok(0);
        }
    }

    let tip_block = provider.get_tip_block_number().await?;
    let on_chain_matches = provider.scan_matches().await.unwrap_or_default();

    // Filter: only match cells whose seller lock hash matches a managed wallet
    let managed_lock_hashes: Vec<Vec<u8>> = wallets.iter().map(|w| w.lock_hash.clone()).collect();

    let managed_matches: Vec<_> = on_chain_matches
        .iter()
        .filter(|m| {
            managed_lock_hashes
                .iter()
                .any(|lh| lh.as_slice() == m.match_args.seller_lock_hash.as_ref())
        })
        .collect();

    if managed_matches.is_empty() {
        debug!("Rent extraction: no managed match cells on chain — skipping cycle");
        return Ok(0);
    }

    let opts = ExtractRentOptions {
        tx_assembler,
        signer,
    };

    let mut total_extracted = 0u64;
    let mut extractions = 0u32;

    for m in &managed_matches {
        let extractable = preview_extractable_from_chain(m, tip_block);

        if extractable < min_extraction_amount_shannons || extractable == 0 {
            continue;
        }

        let tx_hash_hex = hex::encode(m.match_outpoint.tx_hash);
        let outpoint_index = m.match_outpoint.index;

        match rent_service::extract_rent(provider, pool, &tx_hash_hex, outpoint_index, &opts).await
        {
            Ok(result) => {
                total_extracted += result.extracted_amount;
                extractions += 1;
                info!(
                    tx_hash = %result.tx_hash,
                    match_cell = %format!("{tx_hash_hex}:{outpoint_index}"),
                    extractable = result.extracted_amount,
                    tip_block,
                    "Rent extracted"
                );
            }
            Err(e) => {
                warn!(
                    match_cell = %format!("{tx_hash_hex}:{outpoint_index}"),
                    error = %e,
                    "Rent extraction skipped for match"
                );
            }
        }
    }

    if extractions > 0 {
        info!(
            extractions,
            total_shannons = total_extracted,
            total_ckb = total_extracted as f64 / 100_000_000.0,
            tip_block,
            "Rent extraction cycle complete"
        );
    } else {
        debug!(
            tip_block,
            managed_matches = managed_matches.len(),
            "Rent extraction: nothing above threshold"
        );
    }

    Ok(total_extracted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::wallets as wallet_db;
    use crate::services::MockChainProvider;

    fn test_provider() -> MockChainProvider {
        MockChainProvider::new()
    }

    #[actix_rt::test]
    async fn no_wallets_no_extraction() {
        let pool = db::init_test_db();
        let provider = test_provider();
        let extracted = run_extraction_cycle(&pool, 1000, &provider, None, None)
            .await
            .unwrap();
        assert_eq!(extracted, 0);
    }

    #[actix_rt::test]
    async fn with_wallets_but_no_matches_on_chain() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();
        let provider = test_provider();

        wallet_db::insert_wallet(
            &mut conn,
            "test-wallet",
            b"encrypted_key_placeholder",
            &[1u8; 32],
            "ckt1q...test",
            None,
            None,
            None,
            "imported",
        )
        .unwrap();

        // No matches on chain → should return 0
        let extracted = run_extraction_cycle(&pool, 1000, &provider, None, None)
            .await
            .unwrap();
        assert_eq!(extracted, 0);
    }
}
