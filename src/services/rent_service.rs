//! Rent service — extract rent and destroy exhausted matches.
//!
//! Uses the linear rent formula: `extractable = rent_per_block × elapsed_blocks`.
//! When accumulated rent >= remaining capacity, the match is exhausted.

use diesel::sqlite::SqliteConnection;
use tracing::info;

use crate::db::DbPool;

use crate::db::matches as match_db;
use crate::db::wallets as wallet_db;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;
use crate::services::hd_wallet_signer::HdWalletSigner;
use crate::services::transaction_assembler::TransactionAssembler;
use opticrum_calculator::types::MatchInfo;

/// Result of extracting rent.
#[derive(serde::Serialize, Debug)]
pub struct ExtractRentResult {
    pub tx_hash: String,
    pub extracted_amount: u64,
    pub is_exhausted: bool,
}

/// Dependencies required for real on-chain rent extraction.
pub struct ExtractRentOptions<'a> {
    pub tx_assembler: Option<&'a TransactionAssembler>,
    pub signer: Option<&'a HdWalletSigner>,
}

impl ExtractRentOptions<'_> {
    pub fn mock() -> ExtractRentOptions<'static> {
        ExtractRentOptions {
            tx_assembler: None,
            signer: None,
        }
    }
}

/// Linear rent: `rate × elapsed_blocks`, saturating on overflow.
pub fn compute_extractable_shannons(
    shannons_per_block: i64,
    last_extraction_block: i64,
    tip_block: u64,
) -> u64 {
    if shannons_per_block <= 0 {
        return 0;
    }
    let rate = shannons_per_block as u64;
    let baseline = if last_extraction_block <= 0 {
        0
    } else {
        last_extraction_block as u64
    };
    let elapsed = tip_block.saturating_sub(baseline);
    rate.saturating_mul(elapsed)
}

/// Preview how much rent can be extracted for a tracked match.
pub fn preview_extractable(
    m: &match_db::TrackedMatch,
    match_info: Option<&MatchInfo>,
    tip_block: u64,
    already_extracted: u64,
) -> u64 {
    let (rate, baseline) = if let Some(info) = match_info {
        (
            info.match_data.shannons_per_block as i64,
            info.match_data.last_extraction_block as i64,
        )
    } else {
        (m.shannons_per_block, m.last_extraction_block)
    };

    let mut extractable = compute_extractable_shannons(rate, baseline, tip_block);
    let capacity = match_info
        .map(|info| info.ckb_capacity)
        .unwrap_or(m.ckb_capacity as u64);
    if capacity > 0 {
        let remaining = capacity.saturating_sub(already_extracted);
        extractable = extractable.min(remaining);
    }
    extractable
}

async fn find_match_info_on_chain<P: ChainProvider + ?Sized>(
    provider: &P,
    tx_hash: &str,
    output_index: i32,
) -> Result<MatchInfo, AppError> {
    provider
        .scan_matches()
        .await?
        .into_iter()
        .find(|m| {
            hex::encode(m.match_outpoint.tx_hash) == tx_hash
                && m.match_outpoint.index == output_index as u32
        })
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Match cell {tx_hash}:{output_index} not found on chain"
            ))
        })
}

fn resolve_seller_address(
    conn: &mut SqliteConnection,
    seller_address: &str,
) -> Result<String, AppError> {
    if let Some(hex_part) = seller_address.strip_prefix("lock_hash:") {
        let lock_hash_bytes = hex::decode(hex_part)
            .map_err(|_| AppError::BadRequest("Invalid seller lock hash".into()))?;
        let wallet = wallet_db::get_wallet_by_lock_hash(conn, &lock_hash_bytes)?;
        return Ok(wallet.ckb_address);
    }
    Ok(seller_address.to_string())
}

async fn broadcast_extract_rent<P: ChainProvider + ?Sized>(
    provider: &P,
    conn: &mut SqliteConnection,
    m: &match_db::TrackedMatch,
    assembler: &TransactionAssembler,
    signer: &HdWalletSigner,
    tip_block: u64,
) -> Result<String, AppError> {
    let match_info = find_match_info_on_chain(provider, &m.tx_hash, m.output_index).await?;
    let seller_address = resolve_seller_address(conn, &m.seller_address)?;
    let secret_key = signer.find_key_by_address(&seller_address).ok_or_else(|| {
        AppError::WalletError(format!(
            "Seller address {seller_address} not found in unlocked HD wallet — unlock the wallet first"
        ))
    })?;

    assembler
        .extract_rent(&seller_address, &secret_key, match_info, tip_block)
        .await
}

/// Extract linearly-vested rent from a match.
pub async fn extract_rent<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    match_id: i64,
    opts: &ExtractRentOptions<'_>,
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
    let match_info = find_match_info_on_chain(provider, &m.tx_hash, m.output_index).await.ok();
    let already_extracted =
        match_db::extracted_for_match(&mut conn, &m.tx_hash, m.output_index)? as u64;
    let extractable = preview_extractable(&m, match_info.as_ref(), tip_block, already_extracted);

    if extractable == 0 {
        return Err(AppError::BadRequest(
            "No rent to extract — too soon since last extraction".into(),
        ));
    }

    let tx_hash = match (opts.tx_assembler, opts.signer) {
        (Some(assembler), Some(signer)) => {
            broadcast_extract_rent(provider, &mut conn, &m, assembler, signer, tip_block).await?
        }
        _ => {
            let tx_hex = format!(
                "extract_rent:{}:{}:{}:{}",
                m.tx_hash, m.output_index, extractable, tip_block
            );
            provider.send_transaction(&tx_hex).await?
        }
    };

    let capacity = match_info
        .as_ref()
        .map(|info| info.ckb_capacity)
        .unwrap_or(m.ckb_capacity as u64);
    let is_exhausted = capacity > 0 && already_extracted.saturating_add(extractable) >= capacity;

    if is_exhausted {
        match_db::update_match_status(&mut conn, match_id, "exhausted")?;
    } else {
        match_db::update_match_extraction(&mut conn, match_id, tip_block)?;
    }

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

    let accumulated = compute_extractable_shannons(
        m.shannons_per_block,
        m.last_extraction_block,
        tip_block,
    );

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

    #[test]
    fn compute_extractable_saturates_on_overflow() {
        let value = compute_extractable_shannons(i64::MAX, 0, u64::MAX);
        assert_eq!(value, u64::MAX);
    }

    fn setup_match(pool: &DbPool) -> i64 {
        let mut conn = pool.get().unwrap();
        match_db::insert_match(
            &mut conn,
            "match_tx_hash_001",
            0,
            "order_tx_hash_001",
            0,
            "ckt1q...seller",
            100,
            0,
            None::<&str>,
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

        let mut conn = pool.get().unwrap();
        match_db::update_match_extraction(&mut conn, match_id, 1000).unwrap();

        let result = extract_rent(&provider, &pool, match_id, &ExtractRentOptions::mock())
            .await
            .expect("extract should succeed");

        assert_eq!(result.extracted_amount, 100_000);
        assert!(!result.is_exhausted);
    }

    #[actix_rt::test]
    async fn extract_rent_too_soon_returns_zero() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(1000);

        let match_id = setup_match(&pool);

        let mut conn = pool.get().unwrap();
        match_db::update_match_extraction(&mut conn, match_id, 1000).unwrap();

        let result =
            extract_rent(&provider, &pool, match_id, &ExtractRentOptions::mock()).await;
        assert!(result.is_err(), "should fail with zero extractable");
    }

    #[actix_rt::test]
    async fn destroy_match_updates_status() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(5000);

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

        extract_rent(&provider, &pool, match_id, &ExtractRentOptions::mock())
            .await
            .unwrap();

        let history =
            match_db::get_extractions_for_match(&mut conn, "match_tx_hash_001", 0).unwrap();
        assert!(!history.is_empty());
        assert!(history[0].extracted_amount > 0);
    }
}
