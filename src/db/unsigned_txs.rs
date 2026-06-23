//! Unsigned transaction persistence — tracks transactions awaiting external signing.

use rusqlite::{params, Connection};

use crate::error::AppError;

/// A pending unsigned transaction record.
#[derive(Clone, Debug, serde::Serialize)]
pub struct UnsignedTransaction {
    pub id: String,
    pub operation: String,
    pub tx_data_json: String,
    pub status: String,
    pub signed_witnesses_json: Option<String>,
    pub tx_hash: Option<String>,
    pub created_at: String,
}

/// Insert a new unsigned transaction.
pub fn insert_unsigned_tx(
    conn: &Connection,
    id: &str,
    operation: &str,
    tx_data_json: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO unsigned_transactions (id, operation, tx_data_json) VALUES (?1, ?2, ?3)",
        params![id, operation, tx_data_json],
    )
    .map_err(AppError::from)?;
    Ok(())
}

/// Get an unsigned transaction by ID.
pub fn get_unsigned_tx(conn: &Connection, id: &str) -> Result<UnsignedTransaction, AppError> {
    conn.query_row(
        "SELECT id, operation, tx_data_json, status, signed_witnesses_json, tx_hash, created_at
         FROM unsigned_transactions WHERE id = ?1",
        params![id],
        |row| {
            Ok(UnsignedTransaction {
                id: row.get(0)?,
                operation: row.get(1)?,
                tx_data_json: row.get(2)?,
                status: row.get(3)?,
                signed_witnesses_json: row.get(4)?,
                tx_hash: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Unsigned tx {} not found", id))
        }
        other => AppError::from(other),
    })
}

/// List all unsigned transactions, newest first.
pub fn list_unsigned_txs(conn: &Connection) -> Result<Vec<UnsignedTransaction>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, operation, tx_data_json, status, signed_witnesses_json, tx_hash, created_at
             FROM unsigned_transactions ORDER BY created_at DESC",
        )
        .map_err(AppError::from)?;

    let rows = stmt
        .query_map([], |row| {
            Ok(UnsignedTransaction {
                id: row.get(0)?,
                operation: row.get(1)?,
                tx_data_json: row.get(2)?,
                status: row.get(3)?,
                signed_witnesses_json: row.get(4)?,
                tx_hash: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(AppError::from)?;

    let mut txs = Vec::new();
    for row in rows {
        txs.push(row.map_err(AppError::from)?);
    }
    Ok(txs)
}

/// Update an unsigned transaction with signed witnesses from an external wallet.
pub fn set_witnesses(
    conn: &Connection,
    id: &str,
    witnesses_json: &str,
) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE unsigned_transactions SET signed_witnesses_json = ?1, status = 'signed' WHERE id = ?2",
            params![witnesses_json, id],
        )
        .map_err(AppError::from)?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Unsigned tx {} not found", id)));
    }
    Ok(())
}

/// Mark an unsigned transaction as broadcast.
pub fn mark_broadcast(conn: &Connection, id: &str, tx_hash: &str) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE unsigned_transactions SET tx_hash = ?1, status = 'broadcast' WHERE id = ?2",
            params![tx_hash, id],
        )
        .map_err(AppError::from)?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Unsigned tx {} not found", id)));
    }
    Ok(())
}

/// Mark an unsigned transaction as failed.
pub fn mark_failed(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE unsigned_transactions SET status = 'failed' WHERE id = ?1",
        params![id],
    )
    .map_err(AppError::from)?;
    Ok(())
}
