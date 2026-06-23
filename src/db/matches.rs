//! Tracked matches persistence — CRUD operations for matched positions.

use rusqlite::{params, Connection};

use crate::error::AppError;

/// A tracked match record as stored in the database.
#[derive(Clone, Debug, serde::Serialize)]
pub struct TrackedMatch {
    pub id: i64,
    pub tx_hash: String,
    pub output_index: i32,
    pub order_tx_hash: String,
    pub order_output_index: i32,
    pub seller_address: String,
    pub rent_per_block: f64,
    pub escrow_blocks: i64,
    pub last_extraction_block: i64,
    pub xudt_amount: Option<String>,
    pub status: String,
    pub created_at: String,
}

/// Insert a tracked match. Returns the new row ID.
#[allow(clippy::too_many_arguments)]
pub fn insert_match(
    conn: &Connection,
    tx_hash: &str,
    output_index: i32,
    order_tx_hash: &str,
    order_output_index: i32,
    seller_address: &str,
    rent_per_block: f64,
    escrow_blocks: u64,
    xudt_amount: Option<&str>,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO tracked_matches (tx_hash, output_index, order_tx_hash, order_output_index, seller_address, rent_per_block, escrow_blocks, xudt_amount)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            tx_hash,
            output_index,
            order_tx_hash,
            order_output_index,
            seller_address,
            rent_per_block,
            escrow_blocks as i64,
            xudt_amount,
        ],
    )
    .map_err(AppError::from)?;

    Ok(conn.last_insert_rowid())
}

/// Get a match by its database ID.
pub fn get_match_by_id(conn: &Connection, id: i64) -> Result<TrackedMatch, AppError> {
    conn.query_row(
        "SELECT id, tx_hash, output_index, order_tx_hash, order_output_index, seller_address,
                rent_per_block, escrow_blocks, last_extraction_block, xudt_amount, status, created_at
         FROM tracked_matches WHERE id = ?1",
        params![id],
        |row| {
            Ok(TrackedMatch {
                id: row.get(0)?,
                tx_hash: row.get(1)?,
                output_index: row.get(2)?,
                order_tx_hash: row.get(3)?,
                order_output_index: row.get(4)?,
                seller_address: row.get(5)?,
                rent_per_block: row.get(6)?,
                escrow_blocks: row.get(7)?,
                last_extraction_block: row.get(8)?,
                xudt_amount: row.get(9)?,
                status: row.get(10)?,
                created_at: row.get(11)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Match id={}", id)),
        other => AppError::from(other),
    })
}

/// Update a match's extraction state.
pub fn update_match_extraction(
    conn: &Connection,
    id: i64,
    last_extraction_block: u64,
) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE tracked_matches SET last_extraction_block = ?1 WHERE id = ?2",
            params![last_extraction_block as i64, id],
        )
        .map_err(AppError::from)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Match id={}", id)));
    }
    Ok(())
}

/// Update a match's status.
pub fn update_match_status(conn: &Connection, id: i64, status: &str) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE tracked_matches SET status = ?1 WHERE id = ?2",
            params![status, id],
        )
        .map_err(AppError::from)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Match id={}", id)));
    }
    Ok(())
}

/// List tracked matches, optionally filtered by status.
pub fn list_matches(
    conn: &Connection,
    status_filter: Option<&str>,
) -> Result<Vec<TrackedMatch>, AppError> {
    let matches = if let Some(s) = status_filter {
        let mut stmt = conn
            .prepare(
                "SELECT id, tx_hash, output_index, order_tx_hash, order_output_index, seller_address,
                        rent_per_block, escrow_blocks, last_extraction_block, xudt_amount, status, created_at
                 FROM tracked_matches WHERE status = ?1 ORDER BY id DESC",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![s], |row| {
                Ok(TrackedMatch {
                    id: row.get(0)?,
                    tx_hash: row.get(1)?,
                    output_index: row.get(2)?,
                    order_tx_hash: row.get(3)?,
                    order_output_index: row.get(4)?,
                    seller_address: row.get(5)?,
                    rent_per_block: row.get(6)?,
                    escrow_blocks: row.get(7)?,
                    last_extraction_block: row.get(8)?,
                    xudt_amount: row.get(9)?,
                    status: row.get(10)?,
                    created_at: row.get(11)?,
                })
            })
            .map_err(AppError::from)?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.map_err(AppError::from)?);
        }
        v
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, tx_hash, output_index, order_tx_hash, order_output_index, seller_address,
                        rent_per_block, escrow_blocks, last_extraction_block, xudt_amount, status, created_at
                 FROM tracked_matches ORDER BY id DESC",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(TrackedMatch {
                    id: row.get(0)?,
                    tx_hash: row.get(1)?,
                    output_index: row.get(2)?,
                    order_tx_hash: row.get(3)?,
                    order_output_index: row.get(4)?,
                    seller_address: row.get(5)?,
                    rent_per_block: row.get(6)?,
                    escrow_blocks: row.get(7)?,
                    last_extraction_block: row.get(8)?,
                    xudt_amount: row.get(9)?,
                    status: row.get(10)?,
                    created_at: row.get(11)?,
                })
            })
            .map_err(AppError::from)?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.map_err(AppError::from)?);
        }
        v
    };
    Ok(matches)
}

// ---------------------------------------------------------------------------
// Extraction history
// ---------------------------------------------------------------------------

/// An extraction history record.
#[derive(Clone, Debug)]
pub struct ExtractionRecord {
    pub id: i64,
    pub match_tx_hash: String,
    pub match_output_index: i32,
    pub extracted_amount: i64,
    pub tip_block: i64,
    pub tx_hash: String,
    pub timestamp: String,
}

/// Record an extraction event.
pub fn insert_extraction(
    conn: &Connection,
    match_tx_hash: &str,
    match_output_index: i32,
    extracted_amount: u64,
    tip_block: u64,
    tx_hash: &str,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO extraction_history (match_tx_hash, match_output_index, extracted_amount, tip_block, tx_hash)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            match_tx_hash,
            match_output_index,
            extracted_amount as i64,
            tip_block as i64,
            tx_hash,
        ],
    )
    .map_err(AppError::from)?;
    Ok(conn.last_insert_rowid())
}

/// Get total extracted amount across all matches (for admin stats).
pub fn total_extracted(conn: &Connection) -> Result<i64, AppError> {
    let total: Option<i64> = conn
        .query_row(
            "SELECT COALESCE(SUM(extracted_amount), 0) FROM extraction_history",
            [],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;
    Ok(total.unwrap_or(0))
}

/// Get extraction history for a specific match.
pub fn get_extractions_for_match(
    conn: &Connection,
    match_tx_hash: &str,
    match_output_index: i32,
) -> Result<Vec<ExtractionRecord>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, match_tx_hash, match_output_index, extracted_amount, tip_block, tx_hash, timestamp
             FROM extraction_history WHERE match_tx_hash = ?1 AND match_output_index = ?2
             ORDER BY id DESC",
        )
        .map_err(AppError::from)?;

    let rows = stmt
        .query_map(params![match_tx_hash, match_output_index], |row| {
            Ok(ExtractionRecord {
                id: row.get(0)?,
                match_tx_hash: row.get(1)?,
                match_output_index: row.get(2)?,
                extracted_amount: row.get(3)?,
                tip_block: row.get(4)?,
                tx_hash: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .map_err(AppError::from)?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(AppError::from)?);
    }
    Ok(records)
}
