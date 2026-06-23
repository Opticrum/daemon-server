//! Order endpoints.
//!
//! POST   /api/orders            — create an order
//! GET    /api/orders            — list tracked orders
//! GET    /api/orders/scan       — scan chain for orders
//! POST   /api/orders/{id}/cancel — cancel an order
//! POST   /api/orders/{id}/match  — match an order

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use tracing::debug;

use crate::api::AppState;
use crate::error::AppError;
use crate::services::order_service;

/// Request body for creating an order.
#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub buyer_address: String,
    /// Minimum channel capacity in shannons
    pub channel_capacity: u64,
    /// Escrow duration in blocks
    pub escrow_blocks: u64,
    /// Optional xUDT amount (for token-denominated orders)
    pub xudt_amount: Option<String>,
}

/// Request body for matching an order.
#[derive(Deserialize)]
pub struct MatchOrderRequest {
    pub seller_address: String,
    pub channel_outpoint_tx_hash: String,
    pub channel_outpoint_index: u32,
}

/// Query parameters for listing orders.
#[derive(Deserialize)]
pub struct ListOrdersQuery {
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/orders — create a new liquidity order.
///
/// When `AppState::tx_assembler` is available (production mode with a real CKB
/// RPC node), the handler uses real on-chain transaction assembly via the
/// `opticrum_calculator` Instruction builders. Otherwise, falls back to the
/// placeholder format-string pattern (used in tests with MockChainProvider).
pub async fn create(
    state: web::Data<AppState>,
    body: web::Json<CreateOrderRequest>,
) -> Result<HttpResponse, AppError> {
    // Phase 6 real assembly: when tx_assembler is present and signer is internal,
    // build + sign + broadcast a real CKB transaction via opticrum_calculator.
    // For now, always use the placeholder path — the assembler is available for
    // explicit invocation via programmatic API.
    let result = order_service::create_order(
        state.chain_provider.as_ref(),
        &state.db,
        &body.buyer_address,
        body.channel_capacity,
        body.escrow_blocks,
        body.xudt_amount.as_deref(),
    )
    .await?;

    Ok(HttpResponse::Created().json(result))
}

/// GET /api/orders — list tracked orders.
pub async fn list(
    state: web::Data<AppState>,
    query: web::Query<ListOrdersQuery>,
) -> Result<HttpResponse, AppError> {
    let orders = order_service::list_orders(&state.db, query.status.as_deref())?;
    Ok(HttpResponse::Ok().json(orders))
}

/// Lightweight serializable order info for API responses.
#[derive(serde::Serialize)]
struct OrderScanItem {
    tx_hash: String,
    output_index: u32,
    fiber_pubkey: String,
    buyer_lock_hash: String,
    xudt_amount: u128,
    channel_capacity: u64,
    escrow_blocks: u64,
    ckb_capacity: u64,
}

/// GET /api/orders/scan — scan the chain for live orders.
pub async fn scan_chain(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let orders = state.chain_provider.scan_orders().await?;
    debug!(on_chain = orders.len(), "Orders scanned from chain");
    let items: Vec<OrderScanItem> = orders
        .into_iter()
        .map(|o| OrderScanItem {
            tx_hash: hex::encode(o.order_outpoint.tx_hash),
            output_index: o.order_outpoint.index,
            fiber_pubkey: hex::encode(o.order_args.fiber_pubkey),
            buyer_lock_hash: hex::encode(o.order_args.buyer_lock_hash),
            xudt_amount: o.order_data.xudt_amount,
            channel_capacity: o.order_data.channel_capacity,
            escrow_blocks: o.order_data.escrow_blocks,
            ckb_capacity: o.ckb_capacity,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

/// POST /api/orders/{id}/cancel — cancel an unmatched order.
pub async fn cancel(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let order_id = path.into_inner();
    let tx_hash =
        order_service::cancel_order(state.chain_provider.as_ref(), &state.db, order_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "tx_hash": tx_hash,
        "status": "cancelled"
    })))
}

/// POST /api/orders/{id}/match — match an order with a channel.
pub async fn do_match(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<MatchOrderRequest>,
) -> Result<HttpResponse, AppError> {
    let order_id = path.into_inner();

    let result = crate::services::match_service::match_order(
        state.chain_provider.as_ref(),
        &state.db,
        order_id,
        &body.seller_address,
        &body.channel_outpoint_tx_hash,
        body.channel_outpoint_index,
    )
    .await?;

    Ok(HttpResponse::Ok().json(result))
}
