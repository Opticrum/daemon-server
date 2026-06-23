//! Tracked orders persistence — CRUD operations for orders created by this server.

use rusqlite::{params, Connection};

use crate::error::AppError;

/// A tracked order record as stored in the database.
#[derive(Clone, Debug, serde::Serialize)]
pub struct TrackedOrder {
    pub id: i64,
    pub tx_hash: String,
    pub output_index: i32,
    pub buyer_address: String,
    pub channel_capacity: i64,
    pub escrow_blocks: i64,
    pub xudt_amount: Option<String>,
    pub status: String,
    pub created_at: String,
}

/// Insert a tracked order. Returns the new row ID.
pub fn insert_order(
    conn: &Connection,
    tx_hash: &str,
    output_index: i32,
    buyer_address: &str,
    channel_capacity: u64,
    escrow_blocks: u64,
    xudt_amount: Option<&str>,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO tracked_orders (tx_hash, output_index, buyer_address, channel_capacity, escrow_blocks, xudt_amount)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            tx_hash,
            output_index,
            buyer_address,
            channel_capacity as i64,
            escrow_blocks as i64,
            xudt_amount,
        ],
    )
    .map_err(AppError::from)?;

    Ok(conn.last_insert_rowid())
}

/// Get an order by its database ID.
pub fn get_order_by_id(conn: &Connection, id: i64) -> Result<TrackedOrder, AppError> {
    conn.query_row(
        "SELECT id, tx_hash, output_index, buyer_address, channel_capacity, escrow_blocks, xudt_amount, status, created_at
         FROM tracked_orders WHERE id = ?1",
        params![id],
        |row| {
            Ok(TrackedOrder {
                id: row.get(0)?,
                tx_hash: row.get(1)?,
                output_index: row.get(2)?,
                buyer_address: row.get(3)?,
                channel_capacity: row.get(4)?,
                escrow_blocks: row.get(5)?,
                xudt_amount: row.get(6)?,
                status: row.get(7)?,
                created_at: row.get(8)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Order id={}", id)),
        other => AppError::from(other),
    })
}

/// Update the status of a tracked order.
pub fn update_order_status(conn: &Connection, id: i64, status: &str) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE tracked_orders SET status = ?1 WHERE id = ?2",
            params![status, id],
        )
        .map_err(AppError::from)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Order id={}", id)));
    }
    Ok(())
}

/// List tracked orders, optionally filtered by status.
pub fn list_orders(
    conn: &Connection,
    status_filter: Option<&str>,
) -> Result<Vec<TrackedOrder>, AppError> {
    let orders = if let Some(s) = status_filter {
        let mut stmt = conn
            .prepare(
                "SELECT id, tx_hash, output_index, buyer_address, channel_capacity, escrow_blocks, xudt_amount, status, created_at
                 FROM tracked_orders WHERE status = ?1 ORDER BY id DESC",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![s], |row| {
                Ok(TrackedOrder {
                    id: row.get(0)?,
                    tx_hash: row.get(1)?,
                    output_index: row.get(2)?,
                    buyer_address: row.get(3)?,
                    channel_capacity: row.get(4)?,
                    escrow_blocks: row.get(5)?,
                    xudt_amount: row.get(6)?,
                    status: row.get(7)?,
                    created_at: row.get(8)?,
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
                "SELECT id, tx_hash, output_index, buyer_address, channel_capacity, escrow_blocks, xudt_amount, status, created_at
                 FROM tracked_orders ORDER BY id DESC",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(TrackedOrder {
                    id: row.get(0)?,
                    tx_hash: row.get(1)?,
                    output_index: row.get(2)?,
                    buyer_address: row.get(3)?,
                    channel_capacity: row.get(4)?,
                    escrow_blocks: row.get(5)?,
                    xudt_amount: row.get(6)?,
                    status: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .map_err(AppError::from)?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.map_err(AppError::from)?);
        }
        v
    };
    Ok(orders)
}
