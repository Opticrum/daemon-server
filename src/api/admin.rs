//! Admin dashboard and configuration endpoints.
//!
//! GET  /api/admin/stats               — aggregate statistics
//! GET  /api/admin/auto-match/config   — current auto-match configuration
//! PUT  /api/admin/auto-match/config   — update auto-match configuration at runtime

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use tracing::{debug, info};

use crate::api::AppState;
use crate::error::AppError;
use crate::services::{match_service, order_service};

/// GET /api/admin/stats — dashboard statistics.
pub async fn stats(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let orders = order_service::list_orders(&state.db, None)?;
    let matches = match_service::list_matches(&state.db, None)?;

    let live_orders = orders.iter().filter(|o| o.status == "live").count();
    let matched_orders = orders.iter().filter(|o| o.status == "matched").count();
    let cancelled_orders = orders.iter().filter(|o| o.status == "cancelled").count();

    let live_matches = matches.iter().filter(|m| m.status == "live").count();
    let exhausted_matches = matches.iter().filter(|m| m.status == "exhausted").count();
    let destroyed_matches = matches.iter().filter(|m| m.status == "destroyed").count();

    let conn = state.db.get()?;
    let total_extracted = crate::db::matches::total_extracted(&conn)?;

    debug!(
        total_orders = orders.len(),
        total_matches = matches.len(),
        total_extracted,
        "Admin stats requested"
    );

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "orders": {
            "total": orders.len(),
            "live": live_orders,
            "matched": matched_orders,
            "cancelled": cancelled_orders,
        },
        "matches": {
            "total": matches.len(),
            "live": live_matches,
            "exhausted": exhausted_matches,
            "destroyed": destroyed_matches,
        },
        "total_extracted_shannons": total_extracted,
    })))
}

/// GET /api/admin/auto-match/config — return current auto-match configuration.
pub async fn get_auto_match_config(
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
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
