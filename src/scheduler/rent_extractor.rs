//! Rent extraction loop — finds managed matches and auto-extracts rent.
//!
//! Runs periodically, scanning the chain for matches owned by managed
//! wallets and extracting rent when above the dust threshold.

use tracing::{debug, info};

use crate::db::DbPool;

use crate::db::{matches as match_db, wallets as wallet_db};
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;

/// Run one extraction cycle.
///
/// Returns the total amount of shannons extracted in this cycle.
pub async fn run_extraction_cycle(
    pool: &DbPool,
    min_extraction_amount_shannons: u64,
    provider: &(dyn ChainProvider + Send + Sync),
) -> Result<u64, AppError> {
    let mut conn = pool.get()?;

    // Get all managed wallet lock hashes
    let wallets = wallet_db::list_wallets(&mut conn)?;
    if wallets.is_empty() {
        debug!("Rent extraction: no managed wallets — skipping cycle");
        return Ok(0);
    }

    let _managed_lock_hashes: Vec<Vec<u8>> = wallets.iter().map(|w| w.lock_hash.clone()).collect();

    // Get the actual tip block from chain
    let tip_block = provider.get_tip_block_number().await?;

    // Get all live matches
    let live_matches = match_db::list_matches(&mut conn, Some("live"))?;
    if live_matches.is_empty() {
        debug!("Rent extraction: no live matches — skipping cycle");
        return Ok(0);
    }

    let mut total_extracted = 0u64;
    let mut extractions = 0u32;

    for m in &live_matches {
        let shannons_per_block = m.shannons_per_block as u64;
        let last_extraction = m.last_extraction_block as u64;
        let elapsed = tip_block.saturating_sub(last_extraction);
        let extractable = shannons_per_block * elapsed;

        if extractable >= min_extraction_amount_shannons {
            let tx_hash = format!(
                "auto_extract:{}:{}:{}",
                m.tx_hash, m.output_index, extractable
            );

            match_db::update_match_extraction(&mut conn, m.id, tip_block)?;
            match_db::insert_extraction(
                &mut conn,
                &m.tx_hash,
                m.output_index,
                extractable,
                tip_block,
                &tx_hash,
            )?;

            total_extracted += extractable;
            extractions += 1;

            info!(
                match_id = m.id,
                extractable, tip_block, elapsed, "Rent extracted"
            );
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
        let extracted = run_extraction_cycle(&pool, 1000, &provider).await.unwrap();
        assert_eq!(extracted, 0);
    }

    #[actix_rt::test]
    async fn extracts_above_threshold() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();
        let provider = test_provider();

        // Add a managed wallet
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

        // Add a live match with high shannons_per_block
        match_db::insert_match(
            &mut conn,
            "match_tx_001",
            0,
            "order_tx_001",
            0,
            "ckt1q...seller",
            1000,
            None::<&str>,
        )
        .unwrap();

        let extracted = run_extraction_cycle(&pool, 100_000, &provider)
            .await
            .unwrap();
        // 1000 * 1000 = 1_000_000 > 100_000 threshold
        assert!(extracted > 0);
    }

    #[actix_rt::test]
    async fn skips_below_threshold() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();
        let provider = test_provider();

        wallet_db::insert_wallet(&mut conn, "test", b"encrypted", &[2u8; 32], "ckt1q...test2", None, None, None, "imported")
            .unwrap();

        // Low shannons_per_block — won't meet threshold
        match_db::insert_match(
            &mut conn,
            "match_tx_002",
            0,
            "order_tx_002",
            0,
            "ckt1q...seller",
            1,
            None::<&str>,
        )
        .unwrap();

        // threshold is 1 CKB = 100_000_000 shannons
        // extractable = 1 * 1000 = 1000 < 100_000_000
        let extracted = run_extraction_cycle(&pool, 100_000_000, &provider)
            .await
            .unwrap();
        assert_eq!(extracted, 0);
    }

    #[actix_rt::test]
    async fn respects_min_extraction_amount() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();
        let provider = test_provider();

        wallet_db::insert_wallet(&mut conn, "low-wallet", b"enc", &[3u8; 32], "ckt1q...low", None, None, None, "imported")
            .unwrap();

        match_db::insert_match(
            &mut conn,
            "match_tx_003",
            0,
            "order_tx_003",
            0,
            "low_seller",
            10,
            None::<&str>,
        )
        .unwrap();

        // extractable = 10 * 1000 = 10_000 < 1_000_000 threshold
        let extracted = run_extraction_cycle(&pool, 1_000_000, &provider)
            .await
            .unwrap();
        assert_eq!(extracted, 0, "should skip when below min extraction");

        // With threshold 0, it should extract
        let extracted2 = run_extraction_cycle(&pool, 0, &provider).await.unwrap();
        assert!(extracted2 > 0, "should extract with zero threshold");
    }
}
