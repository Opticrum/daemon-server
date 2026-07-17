//! Database initialization and connection pool.
//!
//! Provides `init_db()` which creates the SQLite connection pool (Diesel-backed)
//! and runs migrations, and the `DbPool` type alias.

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use std::path::Path;

use crate::error::AppError;

pub mod destroyed_matches;
pub mod matches;
pub mod schema;
pub mod unsigned_txs;
pub mod wallets;

/// Type alias for the r2d2 SQLite connection pool (Diesel-backed).
pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

/// Initialize the database: create pool, run migrations.
///
/// If the database file does not exist, it will be created.
/// Creates parent directories automatically.
pub fn init_db(database_url: &str) -> Result<DbPool, AppError> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(database_url).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("Failed to create data dir: {e}")))?;
        }
    }

    // Check if this is a legacy (pre-Diesel) database that lacks the migration
    // tracking table. Since this project is pre-production, we bail out with
    // a clear message rather than attempting silent migration.
    if Path::new(database_url).exists() {
        let mut test_conn = SqliteConnection::establish(database_url)
            .map_err(|e| AppError::Internal(format!("Failed to open database for check: {e}")))?;
        let has_diesel_migrations: bool = diesel::sql_query(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='__diesel_schema_migrations'",
        )
        .get_result::<DieselMigrationCount>(&mut test_conn)
        .map(|c| c.count > 0)
        .unwrap_or(false);
        if !has_diesel_migrations {
            // Check if old tables exist
            let has_old_tables: bool = diesel::sql_query(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wallets'",
            )
            .get_result::<DieselMigrationCount>(&mut test_conn)
            .map(|c| c.count > 0)
            .unwrap_or(false);
            if has_old_tables {
                return Err(AppError::Internal(
                    "Legacy database detected (pre-Diesel). \
                     Please delete the database file and restart: rm {database_url}"
                        .into(),
                ));
            }
        }
    }

    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .build(manager)
        .map_err(|e| AppError::Internal(format!("Failed to create connection pool: {e}")))?;

    // Run migrations
    let mut conn = pool.get()?;
    schema::run_migrations(&mut conn)?;

    Ok(pool)
}

/// Initialize an in-memory database for testing.
///
/// Uses a unique shared-cache in-memory database so that all connections
/// from the pool share the same data. (Plain `:memory:` creates a distinct
/// database per connection, which breaks r2d2 pooling.)
pub fn init_test_db() -> DbPool {
    use std::sync::atomic::{AtomicU64, Ordering};
    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let url = format!("file:test_db_{id}?mode=memory&cache=shared");

    let manager = ConnectionManager::<SqliteConnection>::new(&url);
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create test pool");
    let mut conn = pool.get().expect("Failed to get test connection");
    schema::run_migrations(&mut conn).expect("Failed to run test migrations");
    pool
}

/// Helper struct for raw SQL query results.
#[derive(QueryableByName)]
struct DieselMigrationCount {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}
