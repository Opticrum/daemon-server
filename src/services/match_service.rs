//! Match service — match orders with pre-created Fiber channels.
//!
//! Consumes an Order Cell and produces a Match Cell, referencing a
//! pre-existing Fiber channel as a CellDep.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tracing::{debug, info};

use crate::db::{matches as match_db, orders as order_db};
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;

/// Result of matching an order.
#[derive(serde::Serialize, Debug)]
pub struct MatchOrderResult {
    pub tx_hash: String,
    pub output_index: i32,
    pub match_id: i64,
}

/// Match an order with a pre-created Fiber channel.
///
/// The channel must already exist on-chain. The channel cell is
/// referenced as a CellDep (not consumed). Produces a Match Cell
/// with pre-computed `rent_per_block`.
pub async fn match_order<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &Pool<SqliteConnectionManager>,
    order_id: i64,
    seller_address: &str,
    channel_outpoint_tx_hash: &str,
    channel_outpoint_index: u32,
) -> Result<MatchOrderResult, AppError> {
    let conn = pool.get()?;
    let order = order_db::get_order_by_id(&conn, order_id)?;

    if order.status != "live" {
        return Err(AppError::BadRequest(format!(
            "Order {} is already {}",
            order_id, order.status
        )));
    }

    // Verify the channel cell exists
    provider
        .get_cell(channel_outpoint_tx_hash, channel_outpoint_index)
        .await
        .map_err(|_| AppError::BadRequest("Channel cell not found on chain".into()))?;

    // Compute rent_per_block from order data
    let ckb_capacity = order.channel_capacity as u64;
    let escrow_blocks = order.escrow_blocks as u64;
    let rent_per_block = ckb_capacity as f64 / escrow_blocks as f64;

    // Build + sign + submit match transaction
    let tx_hex = format!(
        "match_order:{}:{}:{}:{}",
        order.tx_hash, order.output_index, channel_outpoint_tx_hash, channel_outpoint_index
    );
    let tx_hash = provider.send_transaction(&tx_hex).await?;
    let output_index = 0; // Match Cell is always output[0]

    // Persist tracked match
    let match_id = match_db::insert_match(
        &conn,
        &tx_hash,
        output_index,
        &order.tx_hash,
        order.output_index,
        seller_address,
        rent_per_block,
        escrow_blocks,
        order.xudt_amount.as_deref(),
    )?;

    // Update order status
    order_db::update_order_status(&conn, order_id, "matched")?;

    info!(
        order_id = order_id,
        match_id = match_id,
        tx_hash = %tx_hash,
        seller = %seller_address,
        channel = %channel_outpoint_tx_hash,
        rent_per_block = rent_per_block,
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
    pool: &Pool<SqliteConnectionManager>,
    status_filter: Option<&str>,
) -> Result<Vec<match_db::TrackedMatch>, AppError> {
    let conn = pool.get()?;
    let matches = match_db::list_matches(&conn, status_filter)?;
    debug!(
        count = matches.len(),
        filter = status_filter.unwrap_or("all"),
        "Matches listed"
    );
    Ok(matches)
}

/// Get a single tracked match by ID.
pub fn get_match(
    pool: &Pool<SqliteConnectionManager>,
    match_id: i64,
) -> Result<match_db::TrackedMatch, AppError> {
    let conn = pool.get()?;
    match_db::get_match_by_id(&conn, match_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::services::{order_service, MockChainProvider};

    #[actix_rt::test]
    async fn match_order_succeeds() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        // Add a fake channel cell
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

        // Create an order first
        let order = order_service::create_order(
            &provider,
            &pool,
            "ckt1q...buyer",
            100_000_000_000,
            300_000,
            None,
        )
        .await
        .unwrap();

        // Match it
        let result = match_order(
            &provider,
            &pool,
            order.order_id,
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
        assert!(m.rent_per_block > 0.0);

        // Order status should be 'matched'
        let o = order_service::get_order(&pool, order.order_id).unwrap();
        assert_eq!(o.status, "matched");
    }

    #[actix_rt::test]
    async fn match_order_invalid_channel_fails() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        let order = order_service::create_order(
            &provider,
            &pool,
            "ckt1q...buyer",
            100_000_000_000,
            100_000,
            None,
        )
        .await
        .unwrap();

        // Channel doesn't exist in mock
        let result = match_order(
            &provider,
            &pool,
            order.order_id,
            "ckt1q...seller",
            "nonexistent_channel",
            0,
        )
        .await;

        assert!(result.is_err());
    }

    #[actix_rt::test]
    async fn match_already_matched_order_fails() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        provider.add_cell(
            "ch001",
            0,
            crate::services::chain_provider::CellOutput {
                capacity: 500_000_000_000,
                lock_hash: [0u8; 32],
                type_hash: None,
                data: vec![],
            },
        );

        let order =
            order_service::create_order(&provider, &pool, "buyer", 100_000_000_000, 100_000, None)
                .await
                .unwrap();

        // First match
        match_order(&provider, &pool, order.order_id, "seller1", "ch001", 0)
            .await
            .unwrap();

        // Second match should fail
        let result = match_order(&provider, &pool, order.order_id, "seller2", "ch001", 0).await;
        assert!(result.is_err());
    }
}
