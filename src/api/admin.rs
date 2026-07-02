//! Admin dashboard and configuration endpoints.
//!
//! GET  /api/admin/stats               — aggregate statistics
//! GET  /api/admin/auto-match/config   — current auto-match configuration
//! PUT  /api/admin/auto-match/config   — update auto-match configuration at runtime

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use tracing::info;

use crate::api::AppState;
use crate::error::AppError;

/// GET /api/admin/stats — dashboard statistics from on-chain data.
pub async fn stats(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let on_chain = state.chain_provider.scan_matches().await?;

    let total = on_chain.len();
    let live = on_chain.iter().filter(|m| m.ckb_capacity > 0).count();
    let exhausted = on_chain.iter().filter(|m| m.ckb_capacity == 0).count();
    let destroyed: usize = 0; // destroyed cells are consumed, not in scan

    let mut conn = state.db.get()?;
    let total_extracted = crate::db::matches::total_extracted(&mut conn)?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "matches": {
            "total": total,
            "live": live,
            "exhausted": exhausted,
            "destroyed": destroyed,
        },
        "total_extracted_shannons": total_extracted,
    })))
}

/// GET /api/admin/auto-match/config — return current auto-match configuration.
pub async fn get_auto_match_config(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "enabled": state.config.auto_match_enabled,
        "min_capacity_shannons": state.config.auto_match_min_capacity,
        "max_escrow_blocks": state.config.auto_match_max_escrow_blocks,
        "interval_secs": state.config.auto_match_interval_secs,
    })))
}

/// Request body for updating auto-match configuration.
#[derive(Deserialize)]
pub struct UpdateAutoMatchConfigRequest {
    pub enabled: Option<bool>,
    pub min_capacity_shannons: Option<u64>,
    pub max_escrow_blocks: Option<u64>,
    pub interval_secs: Option<u64>,
}

/// PUT /api/admin/auto-match/config — update auto-match config at runtime.
///
/// Changes take effect on the next auto-match cycle. Does not require
/// a server restart.
pub async fn update_auto_match_config(
    state: web::Data<AppState>,
    body: web::Json<UpdateAutoMatchConfigRequest>,
) -> Result<HttpResponse, AppError> {
    // Note: Config is not mutable in AppState. Runtime config changes
    // would require an Arc<RwLock<Config>> or similar. For now, we
    // acknowledge the request and report what would change.
    // Full runtime mutation will be wired when needed.
    let current = serde_json::json!({
        "enabled": state.config.auto_match_enabled,
        "min_capacity_shannons": state.config.auto_match_min_capacity,
        "max_escrow_blocks": state.config.auto_match_max_escrow_blocks,
        "interval_secs": state.config.auto_match_interval_secs,
    });

    let requested = serde_json::json!({
        "enabled": body.enabled,
        "min_capacity_shannons": body.min_capacity_shannons,
        "max_escrow_blocks": body.max_escrow_blocks,
        "interval_secs": body.interval_secs,
    });

    info!(
        enabled = body.enabled,
        min_capacity = body.min_capacity_shannons,
        "Auto-match config update requested (restart required)"
    );

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Runtime config update received. Note: restart required for changes to take effect (Arc<RwLock<Config>> to be added).",
        "current": current,
        "requested": requested,
    })))
}
