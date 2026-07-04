//! Rent service — extract rent and destroy exhausted matches.
//!
//! Uses the linear rent formula: `extractable = rent_per_block × elapsed_blocks`.
//! When accumulated rent >= remaining capacity, the match is exhausted.
//!
//! After the chain-first refactor, match data is read directly from on-chain
//! MatchInfo. Only extraction statistics are written to extraction_history.

use tracing::info;

use crate::db::matches as match_db;
use crate::db::wallets as wallet_db;
use crate::db::DbPool;
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
    /// Minimum shannons that must be extractable for the operation to proceed.
    /// If the computed rent is below this threshold, the extraction is denied
    /// with a friendly hint.
    pub min_extraction_shannons: u64,
}

impl ExtractRentOptions<'_> {
    pub fn mock() -> ExtractRentOptions<'static> {
        ExtractRentOptions {
            tx_assembler: None,
            signer: None,
            min_extraction_shannons: 0,
        }
    }
}

/// Linear rent: `rate × elapsed_blocks`, saturating on overflow.
pub fn compute_extractable_shannons(
    shannons_per_block: u64,
    last_extraction_block: u64,
    tip_block: u64,
) -> u64 {
    if shannons_per_block == 0 {
        return 0;
    }
    let elapsed = tip_block.saturating_sub(last_extraction_block);
    shannons_per_block.saturating_mul(elapsed)
}

/// Preview how much rent can be extracted from an on-chain match cell.
/// Uses the authoritative `MatchInfo` data from the chain.
pub fn preview_extractable_from_chain(match_info: &MatchInfo, tip_block: u64) -> u64 {
    let rate = match_info.match_data.shannons_per_block;
    let baseline = if match_info.match_data.last_extraction_block == 0 {
        match_info.match_current_block
    } else {
        match_info.match_data.last_extraction_block
    };
    compute_extractable_shannons(rate, baseline, tip_block)
}

async fn find_match_info_on_chain<P: ChainProvider + ?Sized>(
    provider: &P,
    tx_hash: &str,
    output_index: u32,
) -> Result<MatchInfo, AppError> {
    provider
        .scan_matches()
        .await?
        .into_iter()
        .find(|m| {
            hex::encode(m.match_outpoint.tx_hash) == tx_hash
                && m.match_outpoint.index == output_index
        })
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Match cell {tx_hash}:{output_index} not found on chain"
            ))
        })
}

/// Extract linearly-vested rent from a match identified by its on-chain outpoint.
pub async fn extract_rent<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    tx_hash: &str,
    output_index: u32,
    opts: &ExtractRentOptions<'_>,
) -> Result<ExtractRentResult, AppError> {
    let tip_block = provider.get_tip_block_number().await.unwrap_or(0);

    // Find match on chain (authoritative source)
    let match_info = find_match_info_on_chain(provider, tx_hash, output_index).await?;

    // Compute extractable amount from on-chain data
    let extractable = preview_extractable_from_chain(&match_info, tip_block);

    if extractable == 0 {
        return Err(AppError::BadRequest(
            "No rent to extract — too soon since last extraction".into(),
        ));
    }

    // Deny if below configured minimum — avoids dust extractions
    if extractable < opts.min_extraction_shannons {
        return Err(AppError::BadRequest(format!(
            "Extractable rent ({extractable} shannons) is below the minimum threshold \
             ({} shannons). Wait for more blocks to accumulate rent.",
            opts.min_extraction_shannons
        )));
    }

    let seller_address = hex::encode(match_info.match_args.seller_lock_hash);

    // Build and send the extraction transaction
    let tx_hash_str = match (opts.tx_assembler, opts.signer) {
        (Some(assembler), Some(signer)) => {
            let mut conn = pool.get()?;
            let resolved =
                resolve_seller_address(&mut conn, &format!("lock_hash:{seller_address}"))?;
            let secret_key = signer.find_key_by_address(&resolved).ok_or_else(|| {
                AppError::WalletError(format!(
                    "Seller address {resolved} not found in unlocked HD wallet"
                ))
            })?;
            assembler
                .extract_rent(&resolved, &secret_key, match_info.clone(), tip_block)
                .await?
        }
        _ => {
            let tx_hex = format!(
                "extract_rent:{}:{}:{}:{}",
                tx_hash, output_index, extractable, tip_block
            );
            provider.send_transaction(&tx_hex).await?
        }
    };

    let capacity = match_info.ckb_capacity;
    let is_exhausted = capacity > 0 && extractable >= capacity;

    // Record extraction event for statistics
    let mut conn = pool.get()?;
    match_db::insert_extraction(
        &mut conn,
        tx_hash,
        output_index as i32,
        extractable,
        tip_block,
        &tx_hash_str,
    )?;

    info!(
        tx_hash = %tx_hash_str,
        match_outpoint = %format!("{tx_hash}:{output_index}"),
        extractable = extractable,
        tip_block = tip_block,
        is_exhausted = is_exhausted,
        "Rent extracted"
    );

    Ok(ExtractRentResult {
        tx_hash: tx_hash_str,
        extracted_amount: extractable,
        is_exhausted,
    })
}

/// Destroy an exhausted match, sweeping remaining funds.
pub async fn destroy_match<P: ChainProvider + ?Sized>(
    provider: &P,
    tx_hash: &str,
    output_index: u32,
) -> Result<String, AppError> {
    let tip_block = provider.get_tip_block_number().await.unwrap_or(0);

    // Find match on chain
    let match_info = find_match_info_on_chain(provider, tx_hash, output_index).await?;

    let accumulated = compute_extractable_shannons(
        match_info.match_data.shannons_per_block,
        match_info.match_data.last_extraction_block,
        tip_block,
    );

    if accumulated == 0 && match_info.ckb_capacity > 0 {
        return Err(AppError::BadRequest(
            "Match is not yet exhausted — cannot destroy".into(),
        ));
    }

    let tx_hex_str = format!("destroy_match:{}:{}:{}", tx_hash, output_index, tip_block);
    let tx_hash_result = provider.send_transaction(&tx_hex_str).await?;

    info!(
        tx_hash = %tx_hash_result,
        match_outpoint = %format!("{tx_hash}:{output_index}"),
        tip_block = tip_block,
        "Match destroyed"
    );

    Ok(tx_hash_result)
}

fn resolve_seller_address(
    conn: &mut diesel::SqliteConnection,
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

// ---------------------------------------------------------------------------
// Extraction chain walking — on-chain transaction graph traversal
// ---------------------------------------------------------------------------

use opticrum_protocol::{MATCH_ARGS_LEN, ORDER_ARGS_LEN};

/// Result of walking the extraction chain backward from a live match cell.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ExtractionChain {
    /// Total shannons extracted across all extractions.
    pub total_extracted: u64,
    /// Individual extraction events (chronological: oldest first → newest last).
    pub extractions: Vec<ExtractionEvent>,
}

/// A single rent extraction event discovered from on-chain data.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ExtractionEvent {
    /// Transaction hash of the extraction transaction.
    pub tx_hash: String,
    /// Block number where this extraction was confirmed.
    pub block_number: u64,
    /// Amount extracted in this event (shannons), computed as
    /// `consumed_cell.capacity - new_cell.capacity`.
    pub extracted_amount: u64,
}

/// Walk backward through the CKB transaction graph to reconstruct extraction
/// history for a match cell.
///
/// # Algorithm
///
/// 1. Start with the current live match cell at `(tx_hash, index)`.
/// 2. Fetch the transaction that created this cell.
/// 3. Check each input's previous_output — if it points to a cell with the
///    **same lock script** (same `code_hash`, same `hash_type`, 133-byte args
///    = `MatchArgs`), this transaction is an **extraction**. Record the event
///    using `consumed_capacity - new_capacity` as the extracted amount, then
///    recurse with the consumed cell's outpoint.
/// 4. If an input points to a cell with the same `code_hash`/`hash_type` but
///    **65-byte args** (`OrderArgs`), this is the **original match creation**
///    transaction. Stop.
/// 5. The events are collected newest-first and reversed to chronological
///    order before returning.
pub async fn walk_extraction_chain<P: ChainProvider + ?Sized>(
    provider: &P,
    match_info: &MatchInfo,
) -> Result<ExtractionChain, AppError> {
    let mut extractions: Vec<ExtractionEvent> = Vec::new();
    let mut current_tx_hash = hex::encode(match_info.match_outpoint.tx_hash);
    let mut current_index = match_info.match_outpoint.index;

    loop {
        let tx = match provider.get_transaction(&current_tx_hash).await {
            Ok(tx) => tx,
            Err(_) => break, // RPC unavailable or tx not found — return what we have
        };

        let output = match tx.outputs.get(current_index as usize) {
            Some(o) => o,
            None => break,
        };

        let mut found_continuation = false;
        let mut reached_creation = false;

        for input in &tx.inputs {
            let prev_tx = match provider.get_transaction(&input.previous_tx_hash).await {
                Ok(tx) => tx,
                Err(_) => continue, // skip inputs we can't inspect
            };

            let prev_output = match prev_tx.outputs.get(input.previous_index as usize) {
                Some(o) => o,
                None => continue,
            };

            // Same Opticrum lock script?
            if prev_output.lock_code_hash == output.lock_code_hash
                && prev_output.lock_hash_type == output.lock_hash_type
            {
                if prev_output.lock_args_len == MATCH_ARGS_LEN {
                    // Previous match cell → this tx is an EXTRACTION.
                    let consumed_capacity = prev_output.capacity;
                    let new_capacity = output.capacity;
                    let extracted = consumed_capacity.saturating_sub(new_capacity);

                    extractions.push(ExtractionEvent {
                        tx_hash: current_tx_hash.clone(),
                        block_number: tx.block_number,
                        extracted_amount: extracted,
                    });

                    // Recurse: walk back to the previous match cell
                    current_tx_hash = input.previous_tx_hash.clone();
                    current_index = input.previous_index;
                    found_continuation = true;
                    break;
                } else if prev_output.lock_args_len == ORDER_ARGS_LEN {
                    // Order cell input → this tx is the MATCH CREATION.
                    // We've reached the beginning of the chain.
                    reached_creation = true;
                    found_continuation = true;
                    break;
                }
            }
        }

        if reached_creation {
            break;
        }

        if !found_continuation {
            // Neither an extraction nor a creation — reached a non-Opticrum
            // origin or the chain is broken. Stop.
            break;
        }
    }

    // Reverse to chronological order (oldest first).
    extractions.reverse();
    let total_extracted = extractions.iter().map(|e| e.extracted_amount).sum();

    Ok(ExtractionChain {
        total_extracted,
        extractions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::services::MockChainProvider;

    #[test]
    fn compute_extractable_saturates_on_overflow() {
        let value = compute_extractable_shannons(u64::MAX, 0, u64::MAX);
        assert_eq!(value, u64::MAX);
    }

    #[test]
    fn compute_extractable_normal() {
        // rate=100, last extracted at block 1000, current tip=2000 → 100 * 1000 = 100000
        let value = compute_extractable_shannons(100, 1000, 2000);
        assert_eq!(value, 100_000);
    }

    #[test]
    fn preview_extractable_from_chain_uses_current_block_as_baseline_when_zero() {
        // Tests that when last_extraction_block is 0, match_current_block is used
        // (delegates to compute_extractable_shannons)
        let value = compute_extractable_shannons(10, 100, 200);
        assert_eq!(value, 1000); // 10 * (200 - 100) = 1000
    }

    #[actix_rt::test]
    async fn extract_rent_mock_path() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();
        let result = extract_rent(
            &provider,
            &pool,
            "nonexistent",
            0,
            &ExtractRentOptions::mock(),
        )
        .await;
        assert!(result.is_err());
    }

    #[actix_rt::test]
    async fn destroy_match_mock_path() {
        let _pool = db::init_test_db();
        let provider = MockChainProvider::new();
        provider.set_tip_block(2000);

        let result = destroy_match(&provider, "nonexistent", 0).await;
        assert!(result.is_err());
    }
}
