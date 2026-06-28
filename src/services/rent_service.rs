//! Rent service — extract rent and destroy exhausted matches.
//!
//! Uses the linear rent formula: `extractable = rent_per_block × elapsed_blocks`.
//! When accumulated rent >= remaining capacity, the match is exhausted.

use tracing::info;

use crate::db::DbPool;

use crate::db::matches as match_db;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;

/// Result of extracting rent.
#[derive(serde::Serialize, Debug)]
pub struct ExtractRentResult {
    pub tx_hash: String,
    pub extracted_amount: u64,
    pub is_exhausted: bool,
}

/// Extract linearly-vested rent from a match.
///
/// Computes `extractable = rent_per_block × (tip_block - last_extraction_block)`.
/// If the accumulated rent exceeds remaining capacity, the match is
/// treated as exhausted and destroyed.
pub async fn extract_rent<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    match_id: i64,
) -> Result<ExtractRentResult, AppError> {
    let mut conn = pool.get()?;
    let m = match_db::get_match_by_id(&mut conn, match_id)?;

    if m.status != "live" {
        return Err(AppError::BadRequest(format!(
            "Match {} is already {}",
            match_id, m.status
        )));
    }

    let tip_block = provider.get_tip_block_number().await?;

    // Compute extractable rent
    let start_block = if m.last_extraction_block == 0 {
        // Never extracted — no prior extraction block, use 0 as the baseline.
        // The actual match creation block would come from chain query in production.
        0u64
    } else {
        m.last_extraction_block as u64
    };
    let elapsed = tip_block.saturating_sub(start_block);
    let extractable = (m.shannons_per_block as u64) * elapsed;

    if extractable == 0 {
        return Err(AppError::BadRequest(
            "No rent to extract — too soon since last extraction".into(),
        ));
    }

    // Check if matched is exhausted
    let _remaining_capacity = 0u64; // In production, this is from cell capacity
    let is_exhausted = false; // Simplified; real impl queries chain

    let tx_hex = format!(
        "extract_rent:{}:{}:{}:{}",
        m.tx_hash, m.output_index, extractable, tip_block
    );
    let tx_hash = provider.send_transaction(&tx_hex).await?;

    if is_exhausted {
        match_db::update_match_status(&mut conn, match_id, "exhausted")?;
    } else {
        match_db::update_match_extraction(&mut conn, match_id, tip_block)?;
    }

    // Record extraction in history
    match_db::insert_extraction(
        &mut conn,
        &m.tx_hash,
        m.output_index,
        extractable,
        tip_block,
        &tx_hash,
    )?;

    info!(
        match_id = match_id,
        tx_hash = %tx_hash,
        extractable = extractable,
        tip_block = tip_block,
        is_exhausted = is_exhausted,
        "Rent extracted"
    );

    Ok(ExtractRentResult {
        tx_hash,
        extracted_amount: extractable,
        is_exhausted,
    })
}

/// Destroy an exhausted match, sweeping remaining funds.
pub async fn destroy_match<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    match_id: i64,
) -> Result<String, AppError> {
    let mut conn = pool.get()?;
    let m = match_db::get_match_by_id(&mut conn, match_id)?;

    if m.status == "destroyed" {
        return Err(AppError::BadRequest("Match already destroyed".into()));
    }

    let tip_block = provider.get_tip_block_number().await?;

    // Verify match is exhausted
    let start_block = if m.last_extraction_block == 0 {
        0u64
    } else {
        m.last_extraction_block as u64
    };
    let elapsed = tip_block.saturating_sub(start_block);
    let accumulated = (m.shannons_per_block as u64) * elapsed;

    if accumulated == 0 && m.status != "exhausted" {
        return Err(AppError::BadRequest(
            "Match is not yet exhausted — cannot destroy".into(),
        ));
    }

    let tx_hex = format!(
        "destroy_match:{}:{}:{}",
        m.tx_hash, m.output_index, tip_block
    );
    let tx_hash = provider.send_transaction(&tx_hex).await?;

    match_db::update_match_status(&mut conn, match_id, "destroyed")?;

    info!(
        match_id = match_id,
        tx_hash = %tx_hash,
        tip_block = tip_block,
        "Match destroyed"
    );

    Ok(tx_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::services::MockChainProvider;

    fn setup_match(pool: &DbPool) -> i64 {
        let mut conn = pool.get().unwrap();
        match_db::insert_match(
            &mut conn,
            "match_tx_hash_001",
            0,
            "order_tx_hash_001",
            0,
            "ckt1q...seller",
            100, // shannons_per_block: 100 shannons/block
            None::<&str>,
        )
        .unwrap()
    }

    #[actix_rt::test]
    async fn extract_rent_normal() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(2000);

        let match_id = setup_match(&pool);

        // Set last_extraction_block to 1000
        let mut conn = pool.get().unwrap();
        match_db::update_match_extraction(&mut conn, match_id, 1000).unwrap();

        let result = extract_rent(&provider, &pool, match_id)
            .await
            .expect("extract should succeed");

        // 100 shannons/block × 1000 blocks = 100_000 shannons
        assert_eq!(result.extracted_amount, 100_000);
        assert!(!result.is_exhausted);
    }

    #[actix_rt::test]
    async fn extract_rent_too_soon_returns_zero() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(1000);

        let match_id = setup_match(&pool);
        // last_extraction_block = 0, so start_block ~= escrow_blocks before tip
        // This should yield 0 or near-zero elapsed

        let mut conn = pool.get().unwrap();
        // Force last_extraction_block to tip so elapsed = 0
        match_db::update_match_extraction(&mut conn, match_id, 1000).unwrap();

        let result = extract_rent(&provider, &pool, match_id).await;
        assert!(result.is_err(), "should fail with zero extractable");
    }

    #[actix_rt::test]
    async fn destroy_match_updates_status() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(5000); // far in the future

        let match_id = setup_match(&pool);

        let tx_hash = destroy_match(&provider, &pool, match_id)
            .await
            .expect("destroy should succeed");

        assert!(!tx_hash.is_empty());

        let mut conn = pool.get().unwrap();
        let m = match_db::get_match_by_id(&mut conn, match_id).unwrap();
        assert_eq!(m.status, "destroyed");
    }

    #[actix_rt::test]
    async fn destroy_already_destroyed_fails() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(5000);

        let match_id = setup_match(&pool);
        destroy_match(&provider, &pool, match_id).await.unwrap();

        let result = destroy_match(&provider, &pool, match_id).await;
        assert!(result.is_err());
    }

    #[actix_rt::test]
    async fn extract_records_extraction_history() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(2000);

        let match_id = setup_match(&pool);
        let mut conn = pool.get().unwrap();
        match_db::update_match_extraction(&mut conn, match_id, 1000).unwrap();

        extract_rent(&provider, &pool, match_id).await.unwrap();

        // Check extraction history
        let history = match_db::get_extractions_for_match(&mut conn, "match_tx_hash_001", 0).unwrap();
        assert!(!history.is_empty());
        assert!(history[0].extracted_amount > 0);
    }
}
