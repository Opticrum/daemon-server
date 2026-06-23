//! Order service — create and cancel opticrum orders.
//!
//! Builds transactions via opticrum-calculator (in production) or
//! records them via MockChainProvider (in tests).

use tracing::{debug, info};

use crate::db::DbPool;

use crate::db::orders as order_db;
use crate::error::AppError;
use crate::services::chain_provider::ChainProvider;

/// Result of creating an order.
#[derive(serde::Serialize, Debug)]
pub struct CreateOrderResult {
    pub tx_hash: String,
    pub output_index: i32,
    pub order_id: i64,
}

/// Create a new liquidity order on-chain.
///
/// This builds an Order Cell via the opticrum calculator, signs it
/// with the buyer's wallet, submits the transaction, and tracks the
/// order in the local database.
pub async fn create_order<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    buyer_address: &str,
    channel_capacity: u64,
    escrow_blocks: u64,
    xudt_amount: Option<&str>,
) -> Result<CreateOrderResult, AppError> {
    // In production, this would:
    // 1. Build an Instruction via opticrum_calculator::create_order
    // 2. Sign the transaction with the buyer's private key
    // 3. Submit via provider.send_transaction()
    // For now, we record a placeholder transaction.

    let tx_hex = format!(
        "create_order:{}:{}:{}",
        buyer_address, channel_capacity, escrow_blocks
    );
    let tx_hash = provider.send_transaction(&tx_hex).await?;
    let output_index = 0; // Order Cell is always output[0]

    // Persist the tracked order
    let mut conn = pool.get()?;
    let order_id = order_db::insert_order(
        &mut conn,
        &tx_hash,
        output_index,
        buyer_address,
        channel_capacity,
        escrow_blocks,
        xudt_amount,
    )?;

    info!(
        order_id = order_id,
        tx_hash = %tx_hash,
        buyer = %buyer_address,
        capacity = channel_capacity,
        escrow_blocks = escrow_blocks,
        xudt = xudt_amount,
        "Order created"
    );

    Ok(CreateOrderResult {
        tx_hash,
        output_index,
        order_id,
    })
}

/// Cancel an unmatched order, returning funds to the buyer.
pub async fn cancel_order<P: ChainProvider + ?Sized>(
    provider: &P,
    pool: &DbPool,
    order_id: i64,
) -> Result<String, AppError> {
    let mut conn = pool.get()?;
    let order = order_db::get_order_by_id(&mut conn, order_id)?;

    if order.status != "live" {
        return Err(AppError::BadRequest(format!(
            "Order {} is already {}",
            order_id, order.status
        )));
    }

    // Build + sign + submit cancel transaction
    let tx_hex = format!("cancel_order:{}:{}", order.tx_hash, order.output_index);
    let tx_hash = provider.send_transaction(&tx_hex).await?;

    // Update status
    order_db::update_order_status(&mut conn, order_id, "cancelled")?;

    info!(
        order_id = order_id,
        tx_hash = %tx_hash,
        "Order cancelled"
    );

    Ok(tx_hash)
}

/// List tracked orders, optionally filtered by status.
pub fn list_orders(
    pool: &DbPool,
    status_filter: Option<&str>,
) -> Result<Vec<order_db::TrackedOrder>, AppError> {
    let mut conn = pool.get()?;
    let orders = order_db::list_orders(&mut conn, status_filter)?;
    debug!(
        count = orders.len(),
        filter = status_filter.unwrap_or("all"),
        "Orders listed"
    );
    Ok(orders)
}

/// Get a single tracked order by ID.
pub fn get_order(
    pool: &DbPool,
    order_id: i64,
) -> Result<order_db::TrackedOrder, AppError> {
    let mut conn = pool.get()?;
    order_db::get_order_by_id(&mut conn, order_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::services::MockChainProvider;

    #[actix_rt::test]
    async fn create_order_persists_in_db() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        let result = create_order(
            &provider,
            &pool,
            "ckt1q...testbuyer",
            100_000_000_000, // 1000 CKB
            300_000,
            None,
        )
        .await
        .expect("create_order should succeed");

        assert_eq!(result.output_index, 0);
        assert!(result.order_id > 0);

        // Verify it's in the DB
        let order = get_order(&pool, result.order_id).expect("should find order");
        assert_eq!(order.status, "live");
        assert_eq!(order.buyer_address, "ckt1q...testbuyer");
    }

    #[actix_rt::test]
    async fn cancel_order_updates_status() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        // Create order first
        let created = create_order(
            &provider,
            &pool,
            "ckt1q...testbuyer",
            500_000_000_000,
            200_000,
            None,
        )
        .await
        .unwrap();

        // Cancel it
        let tx_hash = cancel_order(&provider, &pool, created.order_id)
            .await
            .expect("cancel should succeed");

        assert!(!tx_hash.is_empty());

        // Verify status updated
        let order = get_order(&pool, created.order_id).unwrap();
        assert_eq!(order.status, "cancelled");
    }

    #[actix_rt::test]
    async fn cancel_already_matched_order_fails() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        let created = create_order(
            &provider,
            &pool,
            "ckt1q...testbuyer",
            100_000_000_000,
            100_000,
            None,
        )
        .await
        .unwrap();

        // Manually set status to 'matched'
        let mut conn = pool.get().unwrap();
        order_db::update_order_status(&mut conn, created.order_id, "matched").unwrap();

        // Cancel should fail
        let result = cancel_order(&provider, &pool, created.order_id).await;
        assert!(result.is_err());
    }

    #[actix_rt::test]
    async fn create_order_with_xudt() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        let result = create_order(
            &provider,
            &pool,
            "ckt1q...buyer",
            200_000_000_000,
            150_000,
            Some("1000000"),
        )
        .await
        .expect("xUDT order creation should succeed");

        let order = get_order(&pool, result.order_id).unwrap();
        assert_eq!(order.xudt_amount, Some("1000000".to_string()));
    }

    #[actix_rt::test]
    async fn list_orders_filters_by_status() {
        let pool = db::init_test_db();
        let provider = MockChainProvider::new();

        let o1 = create_order(&provider, &pool, "a", 1000, 100, None)
            .await
            .unwrap();
        let o2 = create_order(&provider, &pool, "b", 2000, 200, None)
            .await
            .unwrap();

        // Cancel o1
        cancel_order(&provider, &pool, o1.order_id).await.unwrap();

        // List only live
        let live = list_orders(&pool, Some("live")).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, o2.order_id);

        // List only cancelled
        let cancelled = list_orders(&pool, Some("cancelled")).unwrap();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].id, o1.order_id);
    }
}
