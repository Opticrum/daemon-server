//! Health check endpoint.

use actix_web::{web, HttpResponse};
use tracing::debug;

use crate::api::AppState;

/// GET /api/health — liveness probe.
pub async fn check(_state: web::Data<AppState>) -> HttpResponse {
    debug!("Health check");
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok"
    }))
}
