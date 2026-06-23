//! External signing endpoints.
//!
//! GET    /api/transactions/unsigned             — list pending unsigned txs
//! GET    /api/transactions/unsigned/{id}        — get unsigned tx data for signing
//! POST   /api/transactions/unsigned/{id}/witnesses — submit signed witnesses
//! POST   /api/transactions/unsigned/{id}/submit — broadcast signed tx to chain

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use tracing::info;

use crate::api::AppState;
use crate::db::unsigned_txs;
use crate::error::AppError;

/// GET /api/transactions/unsigned — list pending unsigned transactions.
pub async fn list_unsigned(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let conn = state.db.get()?;
    let txs = unsigned_txs::list_unsigned_txs(&conn)?;
    Ok(HttpResponse::Ok().json(txs))
}

/// GET /api/transactions/unsigned/{id} — get a single unsigned transaction.
pub async fn get_unsigned(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let conn = state.db.get()?;
    let tx = unsigned_txs::get_unsigned_tx(&conn, &id)?;
    Ok(HttpResponse::Ok().json(tx))
}

/// Request body for submitting signed witnesses from an external wallet.
#[derive(Deserialize)]
pub struct SubmitWitnessesRequest {
    /// JSON-serialized witnesses from the external wallet.
    pub witnesses: serde_json::Value,
}

/// POST /api/transactions/unsigned/{id}/witnesses — submit signed witnesses.
pub async fn submit_witnesses(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<SubmitWitnessesRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let conn = state.db.get()?;

    // Verify the unsigned tx exists
    let tx = unsigned_txs::get_unsigned_tx(&conn, &id)?;
    if tx.status != "pending" {
        return Err(AppError::BadRequest(format!(
            "Transaction {} is already {}",
            id, tx.status
        )));
    }

    let witnesses_json = serde_json::to_string(&body.witnesses)?;
    unsigned_txs::set_witnesses(&conn, &id, &witnesses_json)?;

    info!(tx_id = %id, "Witnesses submitted for unsigned transaction");

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "id": id,
        "status": "signed"
    })))
}

/// POST /api/transactions/unsigned/{id}/submit — broadcast signed tx to chain.
pub async fn submit_to_chain(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let conn = state.db.get()?;
    let tx = unsigned_txs::get_unsigned_tx(&conn, &id)?;

    if tx.status != "signed" {
        return Err(AppError::BadRequest(format!(
            "Transaction {} is {} — must be 'signed' before broadcast",
            id, tx.status
        )));
    }

    // In Phase 3, we broadcast a composite tx_hex built from the original
    // data + the external witness. Phase 6 will wire real CKB transaction
    // assembly where witnesses are embedded in the transaction structure.
    let witnesses = tx
        .signed_witnesses_json
        .as_deref()
        .unwrap_or("{}");
    let tx_hex = format!("ext_tx:{}:witness={}", tx.tx_data_json, witnesses);

    let tx_hash = state.chain_provider.send_transaction(&tx_hex).await?;

    unsigned_txs::mark_broadcast(&conn, &id, &tx_hash)?;

    info!(tx_id = %id, tx_hash = %tx_hash, "Signed transaction broadcast to chain");

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "id": id,
        "tx_hash": tx_hash,
        "status": "broadcast"
    })))
}
