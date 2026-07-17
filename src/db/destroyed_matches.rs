//! Persistence for destroyed Match cells — cells that have been consumed on-chain
//! and therefore no longer appear in `scan_matches()`.
//!
//! When a match is destroyed, a tombstone record is inserted here so the console
//! can still list and display it (with extraction history from `extraction_history`).

use crate::db::schema::destroyed_matches;
use crate::error::AppError;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

/// A row from the destroyed_matches tombstone table.
#[derive(Clone, Debug, serde::Serialize, Queryable, Identifiable, Selectable)]
#[diesel(table_name = destroyed_matches)]
pub struct DestroyedMatchRow {
    pub id: i64,
    pub tx_hash: String,
    pub output_index: i32,
    pub order_tx_hash: String,
    pub order_output_index: i32,
    pub seller_lock_hash: String,
    pub shannons_per_block: i64,
    pub ckb_capacity: i64,
    pub last_extraction_block: i64,
    pub xudt_amount: Option<String>,
    pub extracted_total: i64,
    pub created_at_block: i64,
    pub destroyed_at: String,
}

/// Insertable row for destroyed_matches.
#[derive(Insertable)]
#[diesel(table_name = destroyed_matches)]
pub struct NewDestroyedMatch<'a> {
    pub tx_hash: &'a str,
    pub output_index: i32,
    pub order_tx_hash: &'a str,
    pub order_output_index: i32,
    pub seller_lock_hash: &'a str,
    pub shannons_per_block: i64,
    pub ckb_capacity: i64,
    pub last_extraction_block: i64,
    pub xudt_amount: Option<&'a str>,
    pub extracted_total: i64,
    pub created_at_block: i64,
}

/// Insert a tombstone record for a destroyed match.
#[allow(clippy::too_many_arguments)]
pub fn insert_destroyed_match(
    conn: &mut SqliteConnection,
    tx_hash: &str,
    output_index: i32,
    order_tx_hash: &str,
    order_output_index: i32,
    seller_lock_hash: &str,
    shannons_per_block: i64,
    ckb_capacity: i64,
    last_extraction_block: i64,
    xudt_amount: Option<&str>,
    extracted_total: i64,
    created_at_block: i64,
) -> Result<i64, AppError> {
    let new = NewDestroyedMatch {
        tx_hash,
        output_index,
        order_tx_hash,
        order_output_index,
        seller_lock_hash,
        shannons_per_block,
        ckb_capacity,
        last_extraction_block,
        xudt_amount,
        extracted_total,
        created_at_block,
    };
    let record: DestroyedMatchRow = diesel::insert_into(destroyed_matches::table)
        .values(&new)
        .get_result(conn)?;
    Ok(record.id)
}

/// List all destroyed matches, newest first.
pub fn list_destroyed_matches(
    conn: &mut SqliteConnection,
) -> Result<Vec<DestroyedMatchRow>, AppError> {
    destroyed_matches::table
        .order(destroyed_matches::destroyed_at.desc())
        .load(conn)
        .map_err(AppError::from)
}

/// Look up a single destroyed match by its on-chain outpoint.
pub fn get_destroyed_match(
    conn: &mut SqliteConnection,
    tx_hash: &str,
    output_index: i32,
) -> Result<Option<DestroyedMatchRow>, AppError> {
    destroyed_matches::table
        .filter(destroyed_matches::tx_hash.eq(tx_hash))
        .filter(destroyed_matches::output_index.eq(output_index))
        .first(conn)
        .optional()
        .map_err(AppError::from)
}

/// Count all destroyed matches (for dashboard stats).
pub fn count_destroyed_matches(conn: &mut SqliteConnection) -> Result<i64, AppError> {
    destroyed_matches::table
        .count()
        .get_result(conn)
        .map_err(AppError::from)
}
