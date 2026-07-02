//! Rent extraction loop — finds managed matches and auto-extracts rent.
//!
//! Runs periodically, scanning the chain for matches owned by managed
//! wallets and extracting rent when above the dust threshold.

use tracing::{debug, info, warn};

use crate::db::DbPool;

use crate::db::{matches as match_db, wallets as wallet_db};
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::hd_wallet_signer::HdWalletSigner;
use crate::services::rent_service::{self, ExtractRentOptions, preview_extractable};
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

    let live_matches = match_db::list_matches(&mut conn, Some("live"))?;
    if live_matches.is_empty() {
        debug!("Rent extraction: no live matches — skipping cycle");
        return Ok(0);
    }

    let opts = ExtractRentOptions {
        tx_assembler,
        signer,
    };

    let mut total_extracted = 0u64;
    let mut extractions = 0u32;

    for m in &live_matches {
        let match_info = on_chain_matches.iter().find(|info| {
            hex::encode(info.match_outpoint.tx_hash) == m.tx_hash
                && info.match_outpoint.index == m.output_index as u32
        });
        let already_extracted =
            match_db::extracted_for_match(&mut conn, &m.tx_hash, m.output_index)? as u64;
        let extractable =
            preview_extractable(m, match_info, tip_block, already_extracted);

        if extractable < min_extraction_amount_shannons || extractable == 0 {
            continue;
        }

        match rent_service::extract_rent(provider, pool, m.id, &opts).await {
            Ok(result) => {
                total_extracted += result.extracted_amount;
                extractions += 1;
                info!(
                    match_id = m.id,
                    tx_hash = %result.tx_hash,
                    extractable = result.extracted_amount,
                    tip_block,
                    "Rent extracted"
                );
            }
            Err(e) => {
                warn!(
                    match_id = m.id,
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
            live_matches = live_matches.len(),
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
    async fn extracts_above_threshold() {
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

        match_db::insert_match(
            &mut conn,
            "match_tx_001",
            0,
            "order_tx_001",
            0,
            "ckt1q...seller",
            1000,
            0,
            None::<&str>,
            None::<&str>,
        )
        .unwrap();

        let extracted = run_extraction_cycle(&pool, 100_000, &provider, None, None)
            .await
            .unwrap();
        assert!(extracted > 0);
    }

    #[actix_rt::test]
    async fn skips_below_threshold() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();
        let provider = test_provider();

        wallet_db::insert_wallet(
            &mut conn,
            "test",
            b"encrypted",
            &[2u8; 32],
            "ckt1q...test2",
            None,
            None,
            None,
            "imported",
        )
        .unwrap();

        match_db::insert_match(
            &mut conn,
            "match_tx_002",
            0,
            "order_tx_002",
            0,
            "ckt1q...seller",
            1,
            0,
            None::<&str>,
            None::<&str>,
        )
        .unwrap();

        let extracted = run_extraction_cycle(&pool, 100_000_000, &provider, None, None)
            .await
            .unwrap();
        assert_eq!(extracted, 0);
    }

    #[actix_rt::test]
    async fn respects_min_extraction_amount() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();
        let provider = test_provider();

        wallet_db::insert_wallet(
            &mut conn,
            "low-wallet",
            b"enc",
            &[3u8; 32],
            "ckt1q...low",
            None,
            None,
            None,
            "imported",
        )
        .unwrap();

        match_db::insert_match(
            &mut conn,
            "match_tx_003",
            0,
            "order_tx_003",
            0,
            "low_seller",
            10,
            0,
            None::<&str>,
            None::<&str>,
        )
        .unwrap();

        let extracted = run_extraction_cycle(&pool, 1_000_000, &provider, None, None)
            .await
            .unwrap();
        assert_eq!(extracted, 0, "should skip when below min extraction");

        let extracted2 = run_extraction_cycle(&pool, 0, &provider, None, None)
            .await
            .unwrap();
        assert!(extracted2 > 0, "should extract with zero threshold");
    }

    #[actix_rt::test]
    async fn large_rate_does_not_overflow() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();
        let provider = test_provider();

        wallet_db::insert_wallet(
            &mut conn,
            "overflow-wallet",
            b"enc",
            &[4u8; 32],
            "ckt1q...overflow",
            None,
            None,
            None,
            "imported",
        )
        .unwrap();

        match_db::insert_match(
            &mut conn,
            "match_tx_overflow",
            0,
            "order_tx_overflow",
            0,
            "seller",
            i64::MAX as u64 / 2,
            0,
            None::<&str>,
            None::<&str>,
        )
        .unwrap();

        let result = run_extraction_cycle(&pool, u64::MAX, &provider, None, None).await;
        assert!(result.is_ok(), "overflow must not panic the scheduler");
    }
}
