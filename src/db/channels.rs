//! Dismissed Fiber channel persistence — hide closed channels from the console.

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::dismissed_fiber_channels;
use crate::error::AppError;

/// Mark a Fiber channel as dismissed so it no longer appears in the console list.
pub fn dismiss_channel(conn: &mut SqliteConnection, channel_id: &str) -> Result<(), AppError> {
    diesel::insert_into(dismissed_fiber_channels::table)
        .values((
            dismissed_fiber_channels::channel_id.eq(channel_id),
            dismissed_fiber_channels::dismissed_at.eq(diesel::dsl::sql("datetime('now')")),
        ))
        .on_conflict(dismissed_fiber_channels::channel_id)
        .do_nothing()
        .execute(conn)?;
    Ok(())
}

/// Return all dismissed channel IDs.
pub fn list_dismissed_ids(conn: &mut SqliteConnection) -> Result<Vec<String>, AppError> {
    dismissed_fiber_channels::table
        .select(dismissed_fiber_channels::channel_id)
        .load(conn)
        .map_err(AppError::from)
}

/// Check whether a channel has been dismissed.
pub fn is_dismissed(conn: &mut SqliteConnection, channel_id: &str) -> Result<bool, AppError> {
    use diesel::dsl::exists;
    use diesel::select;

    select(exists(
        dismissed_fiber_channels::table
            .filter(dismissed_fiber_channels::channel_id.eq(channel_id)),
    ))
    .get_result(conn)
    .map_err(AppError::from)
}
