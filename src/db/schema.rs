//! Database schema and migrations.
//!
//! All CREATE TABLE statements are defined here. `run_migrations` is
//! idempotent — it uses `IF NOT EXISTS` so it's safe to run on every
//! server startup.

use rusqlite::Connection;
use tracing::info;

use crate::error::AppError;

/// Run all schema migrations. Idempotent (uses IF NOT EXISTS).
pub fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    info!("Running database migrations");

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS wallets (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            label           TEXT NOT NULL,
            encrypted_key   BLOB NOT NULL,
            lock_hash       BLOB NOT NULL UNIQUE,
            ckb_address     TEXT NOT NULL,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tracked_orders (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            tx_hash          TEXT NOT NULL,
            output_index     INTEGER NOT NULL,
            buyer_address    TEXT NOT NULL,
            channel_capacity INTEGER NOT NULL,
            escrow_blocks    INTEGER NOT NULL,
            xudt_amount      TEXT,
            status           TEXT NOT NULL DEFAULT 'live',
            created_at       TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(tx_hash, output_index)
        );

        CREATE TABLE IF NOT EXISTS tracked_matches (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            tx_hash                 TEXT NOT NULL,
            output_index            INTEGER NOT NULL,
            order_tx_hash           TEXT NOT NULL,
            order_output_index      INTEGER NOT NULL,
            seller_address          TEXT NOT NULL,
            rent_per_block          REAL NOT NULL,
            escrow_blocks           INTEGER NOT NULL,
            last_extraction_block   INTEGER NOT NULL DEFAULT 0,
            xudt_amount             TEXT,
            status                  TEXT NOT NULL DEFAULT 'live',
            created_at              TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(tx_hash, output_index)
        );

        CREATE TABLE IF NOT EXISTS unsigned_transactions (
            id              TEXT PRIMARY KEY,
            operation       TEXT NOT NULL,
            tx_data_json    TEXT NOT NULL,
            status          TEXT NOT NULL DEFAULT 'pending',
            signed_witnesses_json TEXT,
            tx_hash         TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS extraction_history (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            match_tx_hash       TEXT NOT NULL,
            match_output_index  INTEGER NOT NULL,
            extracted_amount    INTEGER NOT NULL,
            tip_block           INTEGER NOT NULL,
            tx_hash             TEXT NOT NULL,
            timestamp           TEXT NOT NULL DEFAULT (datetime('now'))
        );
        ",
    )
    .map_err(|e| AppError::Internal(format!("Migration failed: {}", e)))?;

    let table_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);

    info!(
        tables = table_count,
        "Database migrations complete"
    );

    Ok(())
}
