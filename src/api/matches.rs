//! Match endpoints.
//!
//! GET    /api/matches             — list tracked matches
//! GET    /api/matches/scan        — scan chain for matches
//! POST   /api/matches/{id}/extract — extract rent
//! POST   /api/matches/{id}/destroy — destroy exhausted match

use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::api::AppState;
use crate::error::AppError;
use crate::services::{match_service, rent_service};

/// Query parameters for listing matches.
#[derive(Deserialize)]
pub struct ListMatchesQuery {
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/matches — list tracked matches.
pub async fn list(
    state: web::Data<AppState>,
    query: web::Query<ListMatchesQuery>,
) -> Result<HttpResponse, AppError> {
    let matches = match_service::list_matches(&state.db, query.status.as_deref())?;
    Ok(HttpResponse::Ok().json(matches))
}

/// Lightweight serializable match info for API responses.
#[derive(serde::Serialize)]
struct MatchScanItem {
    tx_hash: String,
    output_index: u32,
    order_tx_hash: String,
    order_output_index: u32,
    fiber_pubkey: String,
    buyer_lock_hash: String,
    seller_lock_hash: String,
    channel_outpoint_tx_hash: String,
    channel_outpoint_index: u32,
    xudt_amount: u128,
    shannons_per_block: u64,
    last_extraction_block: u64,
    ckb_capacity: u64,
    match_current_block: u64,
}

/// GET /api/matches/scan — scan the chain for live matches.
pub async fn scan_chain(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let matches = state.chain_provider.scan_matches().await?;
    let items: Vec<MatchScanItem> = matches
        .into_iter()
        .map(|m| MatchScanItem {
            tx_hash: hex::encode(m.match_outpoint.tx_hash),
            output_index: m.match_outpoint.index,
            order_tx_hash: hex::encode(m.match_args.order_args.fiber_pubkey.to_bytes()),
            order_output_index: 0, // order outpoint is embedded in match_args
            fiber_pubkey: hex::encode(m.match_args.order_args.fiber_pubkey.to_bytes()),
            buyer_lock_hash: hex::encode(m.match_args.order_args.buyer_lock_hash),
            seller_lock_hash: hex::encode(m.match_args.seller_lock_hash),
            channel_outpoint_tx_hash: hex::encode(m.match_args.channel_outpoint.tx_hash),
            channel_outpoint_index: m.match_args.channel_outpoint.index,
            xudt_amount: m.match_data.xudt_amount,
            shannons_per_block: m.match_data.shannons_per_block,
            last_extraction_block: m.match_data.last_extraction_block,
            ckb_capacity: m.ckb_capacity,
            match_current_block: m.match_current_block,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

/// POST /api/matches/{id}/extract — extract rent from a match.
pub async fn extract(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let match_id = path.into_inner();
    let result = rent_service::extract_rent(
        state.chain_provider.as_ref(),
        &state.db,
        match_id,
        &rent_service::ExtractRentOptions {
            tx_assembler: state.tx_assembler.as_ref(),
            signer: Some(state.signer.as_ref()),
        },
    )
    .await?;
    Ok(HttpResponse::Ok().json(result))
}

/// POST /api/matches/{id}/destroy — destroy an exhausted match.
pub async fn destroy(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let match_id = path.into_inner();
    let tx_hash =
        rent_service::destroy_match(state.chain_provider.as_ref(), &state.db, match_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "tx_hash": tx_hash,
        "status": "destroyed"
    })))
}
