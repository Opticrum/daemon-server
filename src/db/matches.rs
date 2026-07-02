//! Extraction history persistence — statistics cache for the admin dashboard.
//!
//! After the chain-first refactor, `tracked_matches` has been removed.
//! This module now only contains `extraction_history` operations for
//! aggregate statistics (total extracted, per-match sums, monthly stats).

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::extraction_history;
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Extraction history (kept for statistics)
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

/// Record an extraction event (statistics only).
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
