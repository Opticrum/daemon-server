//! Tracked matches persistence — CRUD operations for matched positions.

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::{extraction_history, tracked_matches};
use crate::error::AppError;

/// A tracked match record as stored in the database.
#[derive(Clone, Debug, serde::Serialize, Queryable, Identifiable, Selectable)]
#[diesel(table_name = tracked_matches)]
pub struct TrackedMatch {
    pub id: i64,
    pub tx_hash: String,
    pub output_index: i32,
    pub order_tx_hash: String,
    pub order_output_index: i32,
    pub seller_address: String,
    pub shannons_per_block: i64,
    pub ckb_capacity: i64,
    pub last_extraction_block: i64,
    pub xudt_amount: Option<String>,
    pub status: String,
    pub created_at: String,
    pub channel_id: Option<String>,
}

/// Data needed to insert a new tracked match.
#[derive(Insertable)]
#[diesel(table_name = tracked_matches)]
pub struct NewTrackedMatch<'a> {
    pub tx_hash: &'a str,
    pub output_index: i32,
    pub order_tx_hash: &'a str,
    pub order_output_index: i32,
    pub seller_address: &'a str,
    pub shannons_per_block: i64,
    pub ckb_capacity: i64,
    pub xudt_amount: Option<&'a str>,
    pub channel_id: Option<&'a str>,
}

/// Insert a tracked match. Returns the new row ID.
#[allow(clippy::too_many_arguments)]
pub fn insert_match(
    conn: &mut SqliteConnection,
    tx_hash: &str,
    output_index: i32,
    order_tx_hash: &str,
    order_output_index: i32,
    seller_address: &str,
    shannons_per_block: u64,
    ckb_capacity: u64,
    xudt_amount: Option<&str>,
    channel_id: Option<&str>,
) -> Result<i64, AppError> {
    let new = NewTrackedMatch {
        tx_hash,
        output_index,
        order_tx_hash,
        order_output_index,
        seller_address,
        shannons_per_block: shannons_per_block as i64,
        ckb_capacity: ckb_capacity as i64,
        xudt_amount,
        channel_id,
    };

    let record: TrackedMatch = diesel::insert_into(tracked_matches::table)
        .values(&new)
        .get_result(conn)?;

    Ok(record.id)
}

/// Get a match by its database ID.
pub fn get_match_by_id(conn: &mut SqliteConnection, id: i64) -> Result<TrackedMatch, AppError> {
    tracked_matches::table
        .filter(tracked_matches::id.eq(id))
        .first(conn)
        .map_err(Into::into)
}

/// Find an existing match for the given order outpoint.
pub fn get_match_by_order(
    conn: &mut SqliteConnection,
    order_tx_hash: &str,
    order_output_index: i32,
) -> Result<TrackedMatch, AppError> {
    tracked_matches::table
        .filter(tracked_matches::order_tx_hash.eq(order_tx_hash))
        .filter(tracked_matches::order_output_index.eq(order_output_index))
        .first(conn)
        .map_err(Into::into)
}

/// Update a match's extraction state.
pub fn update_match_extraction(
    conn: &mut SqliteConnection,
    id: i64,
    last_extraction_block: u64,
) -> Result<(), AppError> {
    let affected = diesel::update(tracked_matches::table.filter(tracked_matches::id.eq(id)))
        .set(tracked_matches::last_extraction_block.eq(last_extraction_block as i64))
        .execute(conn)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Match id={id}")));
    }
    Ok(())
}

/// Update a match's status.
pub fn update_match_status(
    conn: &mut SqliteConnection,
    id: i64,
    status: &str,
) -> Result<(), AppError> {
    let affected = diesel::update(tracked_matches::table.filter(tracked_matches::id.eq(id)))
        .set(tracked_matches::status.eq(status))
        .execute(conn)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Match id={id}")));
    }
    Ok(())
}

/// List tracked matches, optionally filtered by status.
pub fn list_matches(
    conn: &mut SqliteConnection,
    status_filter: Option<&str>,
) -> Result<Vec<TrackedMatch>, AppError> {
    let mut query = tracked_matches::table
        .order(tracked_matches::id.desc())
        .into_boxed();

    if let Some(s) = status_filter {
        query = query.filter(tracked_matches::status.eq(s));
    }

    query.load(conn).map_err(AppError::from)
}

/// Delete a tracked match and its extraction history by on-chain outpoint.
pub fn delete_match_by_outpoint(
    conn: &mut SqliteConnection,
    tx_hash: &str,
    output_index: i32,
) -> Result<bool, AppError> {
    diesel::delete(
        extraction_history::table.filter(
            extraction_history::match_tx_hash
                .eq(tx_hash)
                .and(extraction_history::match_output_index.eq(output_index)),
        ),
    )
    .execute(conn)?;

    let affected = diesel::delete(
        tracked_matches::table.filter(
            tracked_matches::tx_hash
                .eq(tx_hash)
                .and(tracked_matches::output_index.eq(output_index)),
        ),
    )
    .execute(conn)?;

    Ok(affected > 0)
}

// ---------------------------------------------------------------------------
// Extraction history
// ---------------------------------------------------------------------------

/// An extraction history record.
#[derive(Clone, Debug, serde::Serialize, Queryable, Identifiable, Selectable)]
#[diesel(table_name = extraction_history)]
pub struct ExtractionRecord {
    pub id: i64,
    pub match_tx_hash: String,
    pub match_output_index: i32,
    pub extracted_amount: i64,
    pub tip_block: i64,
    pub tx_hash: String,
    pub timestamp: String,
}

/// Data needed to insert an extraction history record.
#[derive(Insertable)]
#[diesel(table_name = extraction_history)]
pub struct NewExtraction<'a> {
    pub match_tx_hash: &'a str,
    pub match_output_index: i32,
    pub extracted_amount: i64,
    pub tip_block: i64,
    pub tx_hash: &'a str,
}

/// Record an extraction event.
pub fn insert_extraction(
    conn: &mut SqliteConnection,
    match_tx_hash: &str,
    match_output_index: i32,
    extracted_amount: u64,
    tip_block: u64,
    tx_hash: &str,
) -> Result<i64, AppError> {
    let new = NewExtraction {
        match_tx_hash,
        match_output_index,
        extracted_amount: extracted_amount as i64,
        tip_block: tip_block as i64,
        tx_hash,
    };

    let record: ExtractionRecord = diesel::insert_into(extraction_history::table)
        .values(&new)
        .get_result(conn)?;

    Ok(record.id)
}

/// Sum of extracted amounts for a single match (by tx_hash + output_index).
pub fn extracted_for_match(
    conn: &mut SqliteConnection,
    match_tx_hash: &str,
    match_output_index: i32,
) -> Result<i64, AppError> {
    let amounts: Vec<i64> = extraction_history::table
        .filter(extraction_history::match_tx_hash.eq(match_tx_hash))
        .filter(extraction_history::match_output_index.eq(match_output_index))
        .select(extraction_history::extracted_amount)
        .load(conn)?;
    Ok(amounts.iter().sum())
}

/// Get total extracted amount across all matches (for admin stats).
pub fn total_extracted(conn: &mut SqliteConnection) -> Result<i64, AppError> {
    let amounts: Vec<i64> = extraction_history::table
        .select(extraction_history::extracted_amount)
        .load(conn)?;
    Ok(amounts.iter().sum())
}

/// Return all non-null channel IDs from tracked matches that are still active
/// (live or exhausted). Used to prevent the same Fiber channel from being
/// matched to multiple orders.
pub fn get_used_channel_ids(conn: &mut SqliteConnection) -> Result<Vec<String>, AppError> {
    tracked_matches::table
        .filter(
            tracked_matches::channel_id
                .is_not_null()
                .and(tracked_matches::status.eq_any(vec!["live", "exhausted"])),
        )
        .select(tracked_matches::channel_id)
        .load::<Option<String>>(conn)
        .map(|rows| rows.into_iter().flatten().collect())
        .map_err(AppError::from)
}

/// Get extraction history for a specific match.
pub fn get_extractions_for_match(
    conn: &mut SqliteConnection,
    match_tx_hash: &str,
    match_output_index: i32,
) -> Result<Vec<ExtractionRecord>, AppError> {
    extraction_history::table
        .filter(
            extraction_history::match_tx_hash
                .eq(match_tx_hash)
                .and(extraction_history::match_output_index.eq(match_output_index)),
        )
        .order(extraction_history::id.desc())
        .load(conn)
        .map_err(AppError::from)
}
