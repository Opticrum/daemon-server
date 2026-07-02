//! Database schema — Diesel table definitions and migration runner.
//!
//! The actual migration SQL lives in `migrations/`. At compile time,
//! `embed_migrations!()` bundles it. At startup, `run_migrations`
//! runs any pending migrations via Diesel's versioned migration system.

use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::info;

use crate::error::AppError;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Run pending schema migrations. Uses Diesel's versioned migration
/// tracking table (`__diesel_schema_migrations`) to know which have
/// already been applied.
pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), AppError> {
    info!("Running pending database migrations");
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| AppError::Internal(format!("Migration failed: {e}")))?;
    info!("Database migrations complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Diesel table! definitions
// ---------------------------------------------------------------------------

diesel::table! {
    wallets (id) {
        id -> BigInt,
        label -> Text,
        encrypted_key -> Binary,
        lock_hash -> Binary,
        ckb_address -> Text,
        created_at -> Text,
        parent_wallet_id -> Nullable<BigInt>,
        derivation_path -> Nullable<Text>,
        derivation_index -> Nullable<Integer>,
        wallet_type -> Text,
    }
}

diesel::table! {
    unsigned_transactions (id) {
        id -> Text,
        operation -> Text,
        tx_data_json -> Text,
        status -> Text,
        signed_witnesses_json -> Nullable<Text>,
        tx_hash -> Nullable<Text>,
        created_at -> Text,
    }
}

diesel::table! {
    extraction_history (id) {
        id -> BigInt,
        match_tx_hash -> Text,
        match_output_index -> Integer,
        extracted_amount -> BigInt,
        tip_block -> BigInt,
        tx_hash -> Text,
        timestamp -> Text,
    }
}
