//! Database initialization and connection pool.
//!
//! Provides `init_db()` which creates the SQLite connection pool and
//! runs migrations, and `Pool` type alias for convenience.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

use crate::error::AppError;

pub mod matches;
pub mod orders;
pub mod schema;
pub mod unsigned_txs;
pub mod wallets;

/// Type alias for the r2d2 SQLite connection pool.
pub type DbPool = Pool<SqliteConnectionManager>;

/// Initialize the database: create pool, run migrations.
///
/// If the database file does not exist, it will be created.
/// Creates parent directories automatically.
pub fn init_db(database_url: &str) -> Result<DbPool, AppError> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(database_url).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("Failed to create data dir: {}", e)))?;
        }
    }

    let manager = SqliteConnectionManager::file(database_url);
    let pool = Pool::builder()
        .build(manager)
        .map_err(|e| AppError::Internal(format!("Failed to create connection pool: {}", e)))?;

    // Run migrations
    let conn = pool.get()?;
    schema::run_migrations(&conn)?;

    Ok(pool)
}

/// Initialize an in-memory database for testing.
/// Available in both test and non-test builds (used by integration tests).
pub fn init_test_db() -> DbPool {
    let manager = SqliteConnectionManager::memory();
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create test pool");
    let conn = pool.get().expect("Failed to get test connection");
    schema::run_migrations(&conn).expect("Failed to run test migrations");
    pool
}
