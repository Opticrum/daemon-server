//! Wallet management endpoints.
//!
//! POST /api/wallets — import a private key
//! GET  /api/wallets — list all managed wallets
//! DELETE /api/wallets/{id} — remove a wallet

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use tracing::{debug, info};

use crate::api::AppState;
use crate::error::AppError;
use crate::services::wallet_service;

/// Request body for importing a wallet.
#[derive(Deserialize)]
pub struct ImportWalletRequest {
    pub label: String,
    pub private_key_hex: String,
    /// Optional password to encrypt the key at rest. If omitted, the key is
    /// stored without encryption (suitable for dev/test environments).
    pub password: Option<String>,
}

/// Public-facing wallet info (never includes the encrypted key).
#[derive(serde::Serialize)]
pub struct WalletResponse {
    pub id: i64,
    pub label: String,
    pub lock_hash: String,
    pub ckb_address: String,
    pub created_at: String,
}

impl From<crate::db::wallets::WalletRecord> for WalletResponse {
    fn from(w: crate::db::wallets::WalletRecord) -> Self {
        Self {
            id: w.id,
            label: w.label,
            lock_hash: hex::encode(&w.lock_hash),
            ckb_address: w.ckb_address,
            created_at: w.created_at,
        }
    }
}

/// POST /api/wallets — import a new private key.
pub async fn import_key(
    state: web::Data<AppState>,
    body: web::Json<ImportWalletRequest>,
) -> Result<HttpResponse, AppError> {
    let wallet = wallet_service::import_wallet(
        &state.db,
        &body.label,
        &body.private_key_hex,
        body.password.as_deref(),
    )?;

    let resp: WalletResponse = wallet.into();
    Ok(HttpResponse::Created().json(resp))
}

/// GET /api/wallets — list all managed wallets.
pub async fn list(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let wallets = wallet_service::list_wallets(&state.db)?;
    debug!(count = wallets.len(), "Wallets listed");
    let resp: Vec<WalletResponse> = wallets.into_iter().map(Into::into).collect();
    Ok(HttpResponse::Ok().json(resp))
}

/// DELETE /api/wallets/{id} — remove a wallet.
pub async fn delete(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let deleted = wallet_service::delete_wallet(&state.db, id)?;
    if deleted {
        info!(wallet_id = id, "Wallet delete request");
        Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
    } else {
        Err(AppError::NotFound(format!("Wallet id={}", id)))
    }
}
