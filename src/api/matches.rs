//! Match endpoints.
//!
//! GET    /api/matches                  — list matches from chain scan
//! GET    /api/matches/scan             — scan chain for matches (detailed)
//! POST   /api/matches/{tx_hash}/{idx}/extract  — extract rent
//! POST   /api/matches/{tx_hash}/{idx}/destroy  — destroy exhausted match

use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::api::AppState;
use crate::error::AppError;
use crate::services::rent_service;

/// Query parameters for listing matches.
#[derive(Deserialize)]
pub struct ListMatchesQuery {
    pub status: Option<String>,
}

/// Path parameters for match operations (identified by on-chain outpoint).
#[derive(Deserialize)]
pub struct MatchPath {
    pub tx_hash: String,
    pub output_index: u32,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/matches — list matches from on-chain scan.
pub async fn list(
    state: web::Data<AppState>,
    query: web::Query<ListMatchesQuery>,
) -> Result<HttpResponse, AppError> {
    let tip_block = state
        .chain_provider
        .get_tip_block_number()
        .await
        .unwrap_or(0);
    let on_chain = state.chain_provider.scan_matches().await?;

    let items: Vec<serde_json::Value> = on_chain
        .iter()
        .filter(|m| match query.status.as_deref() {
            Some("live") => m.ckb_capacity > 0,
            Some("exhausted") => m.ckb_capacity == 0,
            Some("destroyed") => false, // destroyed cells are consumed
            _ => true,
        })
        .map(|m| {
            let tx_hash = hex::encode(m.match_outpoint.tx_hash);
            let output_index = m.match_outpoint.index;
            let is_exhausted = m.ckb_capacity == 0;
            serde_json::json!({
                "tx_hash": tx_hash,
                "output_index": output_index,
                "order_tx_hash": hex::encode(m.match_args.order_args.fiber_pubkey.to_bytes()),
                "seller_lock_hash": hex::encode(m.match_args.seller_lock_hash),
                "shannons_per_block": m.match_data.shannons_per_block,
                "ckb_capacity": m.ckb_capacity,
                "last_extraction_block": m.match_data.last_extraction_block,
                "xudt_amount": m.match_data.xudt_amount,
                "status": if is_exhausted { "exhausted" } else { "live" },
                "match_current_block": m.match_current_block,
                "tip_block": tip_block,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(items))
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

/// POST /api/matches/{tx_hash}/{output_index}/extract — extract rent from a match.
pub async fn extract(
    state: web::Data<AppState>,
    path: web::Path<MatchPath>,
) -> Result<HttpResponse, AppError> {
    let p = path.into_inner();
    let min_extraction = state
        .runtime_config
        .read()
        .map(|rc| rc.min_extraction_amount_shannons)
        .unwrap_or(0);
    let result = rent_service::extract_rent(
        state.chain_provider.as_ref(),
        &state.db,
        &p.tx_hash,
        p.output_index,
        &rent_service::ExtractRentOptions {
            tx_assembler: state.tx_assembler.as_ref(),
            signer: Some(state.signer.as_ref()),
            min_extraction_shannons: min_extraction,
        },
    )
    .await?;
    state.cached_chain.spawn_cache_refresh();
    Ok(HttpResponse::Ok().json(result))
}

/// POST /api/matches/{tx_hash}/{output_index}/destroy — destroy an exhausted match.
pub async fn destroy(
    state: web::Data<AppState>,
    path: web::Path<MatchPath>,
) -> Result<HttpResponse, AppError> {
    let p = path.into_inner();
    let tx_hash = rent_service::destroy_match(
        state.chain_provider.as_ref(),
        &state.db,
        &p.tx_hash,
        p.output_index,
    )
    .await?;
    state.cached_chain.spawn_cache_refresh();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "tx_hash": tx_hash,
        "status": "destroyed"
    })))
}
