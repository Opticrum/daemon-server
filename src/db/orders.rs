//! Tracked orders persistence — CRUD operations for orders created by this server.

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::tracked_orders;
use crate::error::AppError;

/// A tracked order record as stored in the database.
#[derive(Clone, Debug, serde::Serialize, Queryable, Identifiable, Selectable)]
#[diesel(table_name = tracked_orders)]
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

/// Data needed to insert a new tracked order.
#[derive(Insertable)]
#[diesel(table_name = tracked_orders)]
pub struct NewTrackedOrder<'a> {
    pub tx_hash: &'a str,
    pub output_index: i32,
    pub buyer_address: &'a str,
    pub channel_capacity: i64,
    pub escrow_blocks: i64,
    pub xudt_amount: Option<&'a str>,
}

/// Insert a tracked order. Returns the new row ID.
pub fn insert_order(
    conn: &mut SqliteConnection,
    tx_hash: &str,
    output_index: i32,
    buyer_address: &str,
    channel_capacity: u64,
    escrow_blocks: u64,
    xudt_amount: Option<&str>,
) -> Result<i64, AppError> {
    let new = NewTrackedOrder {
        tx_hash,
        output_index,
        buyer_address,
        channel_capacity: channel_capacity as i64,
        escrow_blocks: escrow_blocks as i64,
        xudt_amount,
    };

    let record: TrackedOrder = diesel::insert_into(tracked_orders::table)
        .values(&new)
        .get_result(conn)?;

    Ok(record.id)
}

/// Get an order by its database ID.
pub fn get_order_by_id(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<TrackedOrder, AppError> {
    tracked_orders::table
        .filter(tracked_orders::id.eq(id))
        .first(conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => AppError::NotFound(format!("Order id={id}")),
            other => AppError::from(other),
        })
}

/// Update the status of a tracked order.
pub fn update_order_status(
    conn: &mut SqliteConnection,
    id: i64,
    status: &str,
) -> Result<(), AppError> {
    let affected = diesel::update(tracked_orders::table.filter(tracked_orders::id.eq(id)))
        .set(tracked_orders::status.eq(status))
        .execute(conn)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("Order id={id}")));
    }
    Ok(())
}

/// List tracked orders, optionally filtered by status.
pub fn list_orders(
    conn: &mut SqliteConnection,
    status_filter: Option<&str>,
) -> Result<Vec<TrackedOrder>, AppError> {
    let mut query = tracked_orders::table
        .order(tracked_orders::id.desc())
        .into_boxed();

    if let Some(s) = status_filter {
        query = query.filter(tracked_orders::status.eq(s));
    }

    query.load(conn).map_err(AppError::from)
}
