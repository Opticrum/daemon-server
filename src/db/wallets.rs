//! Wallet persistence — CRUD operations for managed CKB wallets.
//!
//! Private keys are stored AES-256-GCM encrypted. The encryption/decryption
//! happens at the service layer; this module only stores/retrieves blobs.

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::wallets;
use crate::error::AppError;

/// A managed wallet record as stored in the database.
#[derive(Clone, Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = wallets)]
pub struct WalletRecord {
    pub id: i64,
    pub label: String,
    pub encrypted_key: Vec<u8>,
    pub lock_hash: Vec<u8>,
    pub ckb_address: String,
    pub created_at: String,
}

/// Data needed to insert a new wallet.
#[derive(Insertable)]
#[diesel(table_name = wallets)]
pub struct NewWallet<'a> {
    pub label: &'a str,
    pub encrypted_key: &'a [u8],
    pub lock_hash: &'a [u8],
    pub ckb_address: &'a str,
}

/// Insert a new wallet. Returns the new row ID.
pub fn insert_wallet(
    conn: &mut SqliteConnection,
    label: &str,
    encrypted_key: &[u8],
    lock_hash: &[u8],
    ckb_address: &str,
) -> Result<i64, AppError> {
    let new = NewWallet {
        label,
        encrypted_key,
        lock_hash,
        ckb_address,
    };

    let record: WalletRecord = diesel::insert_into(wallets::table)
        .values(&new)
        .get_result(conn)
        .map_err(|e| match &e {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ) => AppError::BadRequest("Wallet with this lock_hash already exists".into()),
            _ => AppError::from(e),
        })?;

    Ok(record.id)
}

/// Get a wallet by its ID.
pub fn get_wallet_by_id(conn: &mut SqliteConnection, id: i64) -> Result<WalletRecord, AppError> {
    wallets::table
        .filter(wallets::id.eq(id))
        .first(conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => AppError::NotFound(format!("Wallet id={id}")),
            other => AppError::from(other),
        })
}

/// Get a wallet by its lock_hash.
pub fn get_wallet_by_lock_hash(
    conn: &mut SqliteConnection,
    lock_hash: &[u8],
) -> Result<WalletRecord, AppError> {
    wallets::table
        .filter(wallets::lock_hash.eq(lock_hash))
        .first(conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => AppError::NotFound("Wallet not found".into()),
            other => AppError::from(other),
        })
}

/// List all managed wallets.
pub fn list_wallets(conn: &mut SqliteConnection) -> Result<Vec<WalletRecord>, AppError> {
    wallets::table
        .order(wallets::id.asc())
        .load(conn)
        .map_err(AppError::from)
}

/// Delete a wallet by ID. Returns true if a row was deleted.
pub fn delete_wallet(conn: &mut SqliteConnection, id: i64) -> Result<bool, AppError> {
    let affected =
        diesel::delete(wallets::table.filter(wallets::id.eq(id))).execute(conn)?;
    Ok(affected > 0)
}
