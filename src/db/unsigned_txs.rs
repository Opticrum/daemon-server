//! Unsigned transaction persistence — tracks transactions awaiting external signing.

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::unsigned_transactions;
use crate::error::AppError;

/// A pending unsigned transaction record.
#[derive(Clone, Debug, serde::Serialize, Queryable, Identifiable, Selectable)]
#[diesel(table_name = unsigned_transactions)]
#[diesel(primary_key(id))]
pub struct UnsignedTransaction {
    pub id: String,
    pub operation: String,
    pub tx_data_json: String,
    pub status: String,
    pub signed_witnesses_json: Option<String>,
    pub tx_hash: Option<String>,
    pub created_at: String,
}

/// Data needed to insert a new unsigned transaction.
#[derive(Insertable)]
#[diesel(table_name = unsigned_transactions)]
pub struct NewUnsignedTx<'a> {
    pub id: &'a str,
    pub operation: &'a str,
    pub tx_data_json: &'a str,
}

/// Insert a new unsigned transaction.
pub fn insert_unsigned_tx(
    conn: &mut SqliteConnection,
    id: &str,
    operation: &str,
    tx_data_json: &str,
) -> Result<(), AppError> {
    let new = NewUnsignedTx {
        id,
        operation,
        tx_data_json,
    };

    diesel::insert_into(unsigned_transactions::table)
        .values(&new)
        .execute(conn)?;

    Ok(())
}

/// Get an unsigned transaction by ID.
pub fn get_unsigned_tx(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<UnsignedTransaction, AppError> {
    unsigned_transactions::table
        .filter(unsigned_transactions::id.eq(id))
        .first(conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => {
                AppError::NotFound(format!("Unsigned tx {id} not found"))
            }
            other => AppError::from(other),
        })
}

/// List all unsigned transactions, newest first.
pub fn list_unsigned_txs(
    conn: &mut SqliteConnection,
) -> Result<Vec<UnsignedTransaction>, AppError> {
    unsigned_transactions::table
        .order(unsigned_transactions::created_at.desc())
        .load(conn)
        .map_err(AppError::from)
}

/// Update an unsigned transaction with signed witnesses from an external wallet.
pub fn set_witnesses(
    conn: &mut SqliteConnection,
    id: &str,
    witnesses_json: &str,
) -> Result<(), AppError> {
    let affected =
        diesel::update(unsigned_transactions::table.filter(unsigned_transactions::id.eq(id)))
            .set((
                unsigned_transactions::signed_witnesses_json.eq(Some(witnesses_json)),
                unsigned_transactions::status.eq("signed"),
            ))
            .execute(conn)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Unsigned tx {id} not found")));
    }
    Ok(())
}

/// Mark an unsigned transaction as broadcast.
pub fn mark_broadcast(
    conn: &mut SqliteConnection,
    id: &str,
    tx_hash: &str,
) -> Result<(), AppError> {
    let affected =
        diesel::update(unsigned_transactions::table.filter(unsigned_transactions::id.eq(id)))
            .set((
                unsigned_transactions::tx_hash.eq(Some(tx_hash)),
                unsigned_transactions::status.eq("broadcast"),
            ))
            .execute(conn)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Unsigned tx {id} not found")));
    }
    Ok(())
}

/// Mark an unsigned transaction as failed.
pub fn mark_failed(conn: &mut SqliteConnection, id: &str) -> Result<(), AppError> {
    diesel::update(unsigned_transactions::table.filter(unsigned_transactions::id.eq(id)))
        .set(unsigned_transactions::status.eq("failed"))
        .execute(conn)?;
    Ok(())
}
