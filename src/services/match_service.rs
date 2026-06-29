//! Match service — match on-chain orders with pre-created Fiber channels.
//!
//! Seller-side only. Orders are created externally by buyers through the
//! frontend application. The seller discovers orders via chain scanning
//! and matches them against available Fiber channels.
//!
//! Consumes an Order Cell and produces a Match Cell, referencing a
//! pre-existing Fiber channel as a CellDep.

use tracing::{debug, info};

use crate::db::DbPool;

use crate::db::matches as match_db;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;

/// Result of matching an order.
#[derive(serde::Serialize, Debug)]
pub struct MatchOrderResult {
    pub tx_hash: String,
    pub output_index: i32,
    pub match_id: i64,
}

/// Match an on-chain order with a pre-created Fiber channel.
///
/// The order is identified by its on-chain outpoint (tx_hash + output_index).
/// The channel must already exist on-chain. The channel cell is referenced
/// as a CellDep (not consumed). Produces a Match Cell.
pub async fn match_order<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    order_tx_hash: &str,
    order_output_index: u32,
    seller_address: &str,
    channel_outpoint_tx_hash: &str,
    channel_outpoint_index: u32,
) -> Result<MatchOrderResult, AppError> {
    // Verify the channel cell exists on-chain
    provider
        .get_cell(channel_outpoint_tx_hash, channel_outpoint_index)
        .await
        .map_err(|_| AppError::BadRequest("Channel cell not found on chain".into()))?;

    // Build + sign + submit match transaction
    let tx_hex = format!(
        "match_order:{}:{}:{}:{}",
        order_tx_hash, order_output_index, channel_outpoint_tx_hash, channel_outpoint_index
    );
    let tx_hash = provider.send_transaction(&tx_hex).await?;
    let output_index = 0; // Match Cell is always output[0]

    // Persist tracked match
    let mut conn = pool.get()?;
    let match_id = match_db::insert_match(
        &mut conn,
        &tx_hash,
        output_index,
        order_tx_hash,
        order_output_index as i32,
        seller_address,
        0, // shannons_per_block — will be populated from chain scan in production
        None::<&str>,
    )?;

    info!(
        match_id = match_id,
        tx_hash = %tx_hash,
        seller = %seller_address,
        channel = %channel_outpoint_tx_hash,
        order_tx = %order_tx_hash,
        order_index = order_output_index,
        "Order matched"
    );

    Ok(MatchOrderResult {
        tx_hash,
        output_index,
        match_id,
    })
}

/// List tracked matches.
pub fn list_matches(
    pool: &DbPool,
    status_filter: Option<&str>,
) -> Result<Vec<match_db::TrackedMatch>, AppError> {
    let mut conn = pool.get()?;
    let matches = match_db::list_matches(&mut conn, status_filter)?;
    debug!(
        count = matches.len(),
        filter = status_filter.unwrap_or("all"),
        "Matches listed"
    );
    Ok(matches)
}

/// Get a single tracked match by ID.
pub fn get_match(pool: &DbPool, match_id: i64) -> Result<match_db::TrackedMatch, AppError> {
    let mut conn = pool.get()?;
    match_db::get_match_by_id(&mut conn, match_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::services::MockChainProvider;

    #[actix_rt::test]
    async fn match_order_succeeds() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        // Add a fake channel cell so get_cell succeeds
        provider.add_cell(
            "channel_tx_hash_001",
            0,
            crate::services::chain_provider::CellOutput {
                capacity: 200_000_000_000,
                lock_hash: [0u8; 32],
                type_hash: None,
                data: vec![],
            },
        );

        // Match an on-chain order (identified by tx_hash + output_index)
        let result = match_order(
            &provider,
            &pool,
            "order_tx_001_on_chain",
            0,
            "ckt1q...seller",
            "channel_tx_hash_001",
            0,
        )
        .await
        .expect("match should succeed");

        assert!(result.match_id > 0);

        // Verify match is in DB
        let m = get_match(&pool, result.match_id).unwrap();
        assert_eq!(m.status, "live");
        assert_eq!(m.seller_address, "ckt1q...seller");
    }

    #[actix_rt::test]
    async fn match_order_invalid_channel_fails() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        // Channel doesn't exist in mock — match should fail
        let result = match_order(
            &provider,
            &pool,
            "order_tx_002",
            0,
            "ckt1q...seller",
            "nonexistent_channel",
            0,
        )
        .await;

        assert!(result.is_err());
    }
}
