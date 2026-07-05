//! Wallet persistence — CRUD operations for managed CKB wallets.
//!
//! Private keys are stored AES-256-GCM encrypted. The encryption/decryption
//! happens at the service layer; this module only stores/retrieves blobs.
//!
//! Wallet types:
//! - `imported` — single private key imported via hex
//! - `hd_child`  — derived from an HD wallet mnemonic/keystore

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::OptionalExtension;

use crate::db::schema::wallets;
use crate::error::AppError;

/// A managed wallet record as stored in the database.
#[derive(Clone, Debug, serde::Serialize, Queryable, Identifiable, Selectable)]
#[diesel(table_name = wallets)]
pub struct WalletRecord {
    pub id: i64,
    pub label: String,
    pub encrypted_key: Vec<u8>,
    pub lock_hash: Vec<u8>,
    pub ckb_address: String,
    pub created_at: String,
    pub parent_wallet_id: Option<i64>,
    pub derivation_path: Option<String>,
    pub derivation_index: Option<i32>,
    pub wallet_type: String,
}

/// Data needed to insert a new wallet.
#[derive(Insertable)]
#[diesel(table_name = wallets)]
pub struct NewWallet<'a> {
    pub label: &'a str,
    pub encrypted_key: &'a [u8],
    pub lock_hash: &'a [u8],
    pub ckb_address: &'a str,
    pub parent_wallet_id: Option<i64>,
    pub derivation_path: Option<&'a str>,
    pub derivation_index: Option<i32>,
    pub wallet_type: &'a str,
}

/// Insert a new wallet. Returns the new row ID.
#[allow(clippy::too_many_arguments)]
pub fn insert_wallet(
    conn: &mut SqliteConnection,
    label: &str,
    encrypted_key: &[u8],
    lock_hash: &[u8],
    ckb_address: &str,
    parent_wallet_id: Option<i64>,
    derivation_path: Option<&str>,
    derivation_index: Option<i32>,
    wallet_type: &str,
) -> Result<i64, AppError> {
    let new = NewWallet {
        label,
        encrypted_key,
        lock_hash,
        ckb_address,
        parent_wallet_id,
        derivation_path,
        derivation_index,
        wallet_type,
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

/// List wallets by wallet type.
pub fn list_wallets_by_type(
    conn: &mut SqliteConnection,
    wt: &str,
) -> Result<Vec<WalletRecord>, AppError> {
    wallets::table
        .filter(wallets::wallet_type.eq(wt))
        .order(wallets::derivation_index.asc())
        .load(conn)
        .map_err(AppError::from)
}

/// List child wallets belonging to a parent (by parent_wallet_id).
pub fn list_wallets_by_parent(
    conn: &mut SqliteConnection,
    parent_id: i64,
) -> Result<Vec<WalletRecord>, AppError> {
    wallets::table
        .filter(wallets::parent_wallet_id.eq(parent_id))
        .order(wallets::derivation_index.asc())
        .load(conn)
        .map_err(AppError::from)
}

/// Update derived address metadata for an HD child wallet (e.g. after fixing derivation).
pub fn update_wallet_derived_info(
    conn: &mut SqliteConnection,
    id: i64,
    lock_hash: &[u8],
    ckb_address: &str,
) -> Result<(), AppError> {
    diesel::update(wallets::table.filter(wallets::id.eq(id)))
        .set((
            wallets::lock_hash.eq(lock_hash),
            wallets::ckb_address.eq(ckb_address),
        ))
        .execute(conn)?;
    Ok(())
}

/// Find an HD child wallet by derivation path.
pub fn get_wallet_by_derivation_path(
    conn: &mut SqliteConnection,
    path: &str,
) -> Result<Option<WalletRecord>, AppError> {
    wallets::table
        .filter(wallets::derivation_path.eq(path))
        .filter(wallets::wallet_type.eq("hd_child"))
        .first(conn)
        .optional()
        .map_err(AppError::from)
}

/// Delete a wallet by ID. Returns true if a row was deleted.
pub fn delete_wallet(conn: &mut SqliteConnection, id: i64) -> Result<bool, AppError> {
    let affected = diesel::delete(wallets::table.filter(wallets::id.eq(id))).execute(conn)?;
    Ok(affected > 0)
}

/// Resolve a `"lock_hash:<hex>"` string to a CKB address from the wallet DB.
///
/// If the input starts with `"lock_hash:"`, the hex part is decoded and looked up
/// in the `wallets` table. Otherwise the input is returned unchanged (pass-through
/// for already-resolved plain addresses).
pub fn resolve_lock_hash_to_address(
    conn: &mut SqliteConnection,
    address_or_lock_hash: &str,
) -> Result<String, AppError> {
    if let Some(hex_part) = address_or_lock_hash.strip_prefix("lock_hash:") {
        let lock_hash_bytes = hex::decode(hex_part)
            .map_err(|_| AppError::BadRequest("Invalid seller lock hash".into()))?;
        let wallet = get_wallet_by_lock_hash(conn, &lock_hash_bytes)?;
        return Ok(wallet.ckb_address);
    }
    Ok(address_or_lock_hash.to_string())
}
