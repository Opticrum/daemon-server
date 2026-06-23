//! Wallet persistence — CRUD operations for managed CKB wallets.
//!
//! Private keys are stored AES-256-GCM encrypted. The encryption/decryption
//! happens at the service layer; this module only stores/retrieves blobs.

use rusqlite::{params, Connection};

use crate::error::AppError;

/// A managed wallet record as stored in the database.
#[derive(Clone, Debug)]
pub struct WalletRecord {
    pub id: i64,
    pub label: String,
    pub encrypted_key: Vec<u8>,
    pub lock_hash: Vec<u8>,
    pub ckb_address: String,
    pub created_at: String,
}

/// Insert a new wallet. Returns the new row ID.
pub fn insert_wallet(
    conn: &Connection,
    label: &str,
    encrypted_key: &[u8],
    lock_hash: &[u8],
    ckb_address: &str,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO wallets (label, encrypted_key, lock_hash, ckb_address) VALUES (?1, ?2, ?3, ?4)",
        params![label, encrypted_key, lock_hash, ckb_address],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            AppError::BadRequest("Wallet with this lock_hash already exists".into())
        } else {
            AppError::from(e)
        }
    })?;
    Ok(conn.last_insert_rowid())
}

/// Get a wallet by its ID.
pub fn get_wallet_by_id(conn: &Connection, id: i64) -> Result<WalletRecord, AppError> {
    conn.query_row(
        "SELECT id, label, encrypted_key, lock_hash, ckb_address, created_at FROM wallets WHERE id = ?1",
        params![id],
        |row| {
            Ok(WalletRecord {
                id: row.get(0)?,
                label: row.get(1)?,
                encrypted_key: row.get(2)?,
                lock_hash: row.get(3)?,
                ckb_address: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Wallet id={}", id)),
        other => AppError::from(other),
    })
}

/// Get a wallet by its lock_hash.
pub fn get_wallet_by_lock_hash(
    conn: &Connection,
    lock_hash: &[u8],
) -> Result<WalletRecord, AppError> {
    conn.query_row(
        "SELECT id, label, encrypted_key, lock_hash, ckb_address, created_at FROM wallets WHERE lock_hash = ?1",
        params![lock_hash],
        |row| {
            Ok(WalletRecord {
                id: row.get(0)?,
                label: row.get(1)?,
                encrypted_key: row.get(2)?,
                lock_hash: row.get(3)?,
                ckb_address: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("Wallet not found".into()),
        other => AppError::from(other),
    })
}

/// List all managed wallets.
pub fn list_wallets(conn: &Connection) -> Result<Vec<WalletRecord>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, label, encrypted_key, lock_hash, ckb_address, created_at FROM wallets ORDER BY id",
        )
        .map_err(AppError::from)?;

    let rows = stmt
        .query_map([], |row| {
            Ok(WalletRecord {
                id: row.get(0)?,
                label: row.get(1)?,
                encrypted_key: row.get(2)?,
                lock_hash: row.get(3)?,
                ckb_address: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(AppError::from)?;

    let mut wallets = Vec::new();
    for row in rows {
        wallets.push(row.map_err(AppError::from)?);
    }
    Ok(wallets)
}

/// Delete a wallet by ID. Returns true if a row was deleted.
pub fn delete_wallet(conn: &Connection, id: i64) -> Result<bool, AppError> {
    let affected = conn
        .execute("DELETE FROM wallets WHERE id = ?1", params![id])
        .map_err(AppError::from)?;
    Ok(affected > 0)
}
