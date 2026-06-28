//! Fiber channel endpoints.
//!
//! GET /api/fiber/channels — scan Fiber network for channels owned by a lock hash.

use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::api::AppState;
use crate::error::AppError;

/// Query parameters for scanning Fiber channels.
#[derive(Deserialize)]
pub struct FiberChannelsQuery {
    /// Owner lock hash (hex-encoded, 32 bytes = 64 hex chars).
    pub owner: Option<String>,
}

/// GET /api/fiber/channels — scan Fiber network for available channels.
///
/// If `owner` is provided, filters channels by owner lock hash.
/// Returns channel outpoints, capacities, and statuses.
pub async fn list_channels(
    state: web::Data<AppState>,
    query: web::Query<FiberChannelsQuery>,
) -> Result<HttpResponse, AppError> {
    let owner_lock_hash = match &query.owner {
        Some(hex_str) => {
            let bytes = hex::decode(hex_str)
                .map_err(|e| AppError::BadRequest(format!("Invalid hex: {e}")))?;
            if bytes.len() != 32 {
                return Err(AppError::BadRequest(
                    "owner lock hash must be 32 bytes (64 hex chars)".into(),
                ));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes);
            hash
        }
        None => {
            // No filter — scan all channels. In production, this may be
            // scoped to channels relevant to managed wallets.
            [0u8; 32]
        }
    };

    let channels = state
        .chain_provider
        .scan_fiber_channels(&owner_lock_hash)
        .await?;

    Ok(HttpResponse::Ok().json(channels))
}
