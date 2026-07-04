//! On-chain order discovery and match endpoints (seller-side only).
//!
//! Buyer-side operations (create/cancel) are handled by the frontend.
//!
//! GET  /api/orders/scan           — scan chain for live orders
//! POST /api/orders/{tx_hash}/match — match an on-chain order with a Fiber channel

use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::api::AppState;
use crate::error::AppError;
use crate::services::match_service;

/// Request body for matching an on-chain order.
#[derive(Deserialize)]
pub struct MatchOrderRequest {
    /// Output index of the order cell to match
    pub order_output_index: u32,
    /// Seller's CKB address
    pub seller_address: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Lightweight serializable order info for API responses (chain-scanned).
#[derive(serde::Serialize)]
struct OrderScanItem {
    tx_hash: String,
    output_index: u32,
    fiber_pubkey: String,
    buyer_lock_hash: String,
    xudt_amount: u128,
    channel_capacity: u64,
    shannons_per_block: u64,
    ckb_capacity: u64,
}

/// GET /api/orders/scan — scan the chain for live orders (seller-side).
pub async fn scan_chain(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let orders = state.chain_provider.scan_orders().await?;
    let own_pubkey = &state.own_fiber_pubkey;
    let items: Vec<OrderScanItem> = orders
        .into_iter()
        .filter(|o| {
            own_pubkey
                .as_ref()
                .is_none_or(|pk| hex::encode(o.order_args.fiber_pubkey.to_bytes()) != *pk)
        })
        .map(|o| OrderScanItem {
            tx_hash: hex::encode(o.order_outpoint.tx_hash),
            output_index: o.order_outpoint.index,
            fiber_pubkey: hex::encode(o.order_args.fiber_pubkey.to_bytes()),
            buyer_lock_hash: hex::encode(o.order_args.buyer_lock_hash),
            xudt_amount: o.order_data.xudt_amount,
            channel_capacity: o.order_data.channel_capacity,
            shannons_per_block: o.order_data.shannons_per_block,
            ckb_capacity: o.ckb_capacity,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

/// POST /api/orders/{tx_hash}/match — match an on-chain order with a Fiber channel.
///
/// The order is identified by its on-chain `tx_hash` and the `order_output_index`
/// in the request body. The server verifies the order exists on-chain before
/// matching. This replaces the old local-order-ID lookup — orders are created
/// by external buyers through the frontend.
pub async fn do_match(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<MatchOrderRequest>,
) -> Result<HttpResponse, AppError> {
    let order_tx_hash = path.into_inner();

    let result = match_service::match_order(
        state.chain_provider.as_ref(),
        &order_tx_hash,
        body.order_output_index,
        &body.seller_address,
    )
    .await?;
    state.cached_chain.spawn_cache_refresh();

    Ok(HttpResponse::Ok().json(result))
}
