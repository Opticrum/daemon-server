//! Persistence for wallet transaction history — one row per (tx_hash, wallet_id).
//!
//! Populated by the background wallet-tx sync loop and the manual refresh
//! endpoint; read (aggregated) by `GET /api/console/wallets/transactions`.

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::wallet_transactions;
use crate::error::AppError;

/// A row from the `wallet_transactions` table.
#[derive(Clone, Debug, serde::Serialize, Queryable, Identifiable, Selectable)]
#[diesel(table_name = wallet_transactions)]
pub struct WalletTxRow {
    pub id: i64,
    pub tx_hash: String,
    pub wallet_id: i64,
    pub block_number: i64,
    pub timestamp_ms: Option<i64>,
    pub received_shannons: i64,
    pub sent_shannons: i64,
}

/// Insertable row (lifetimed `&'a str` so a slice can be batch-inserted).
#[derive(Insertable)]
#[diesel(table_name = wallet_transactions)]
pub struct NewWalletTx<'a> {
    pub tx_hash: &'a str,
    pub wallet_id: i64,
    pub block_number: i64,
    pub timestamp_ms: Option<i64>,
    pub received_shannons: i64,
    pub sent_shannons: i64,
}

/// Upsert a batch, updating block/timestamp/amounts on `(tx_hash, wallet_id)`
/// conflict. Done row-by-row: Diesel cannot express `ON CONFLICT ... DO UPDATE`
/// for batch inserts on SQLite.
pub fn upsert_batch(conn: &mut SqliteConnection, rows: &[NewWalletTx]) -> Result<usize, AppError> {
    use diesel::upsert::excluded;

    let mut affected = 0usize;
    for row in rows {
        affected += diesel::insert_into(wallet_transactions::table)
            .values(row)
            .on_conflict((wallet_transactions::tx_hash, wallet_transactions::wallet_id))
            .do_update()
            .set((
                wallet_transactions::block_number.eq(excluded(wallet_transactions::block_number)),
                wallet_transactions::timestamp_ms.eq(excluded(wallet_transactions::timestamp_ms)),
                wallet_transactions::received_shannons
                    .eq(excluded(wallet_transactions::received_shannons)),
                wallet_transactions::sent_shannons.eq(excluded(wallet_transactions::sent_shannons)),
            ))
            .execute(conn)?;
    }
    Ok(affected)
}

/// All rows, newest block first.
pub fn list_all(conn: &mut SqliteConnection) -> Result<Vec<WalletTxRow>, AppError> {
    wallet_transactions::table
        .order(wallet_transactions::block_number.desc())
        .load(conn)
        .map_err(AppError::from)
}

/// Rows for a single wallet, newest block first.
pub fn list_wallet_txs(
    conn: &mut SqliteConnection,
    wallet_id: i64,
) -> Result<Vec<WalletTxRow>, AppError> {
    wallet_transactions::table
        .filter(wallet_transactions::wallet_id.eq(wallet_id))
        .order(wallet_transactions::block_number.desc())
        .load(conn)
        .map_err(AppError::from)
}

/// Delete rows whose `wallet_id` is NOT in the keep set. Returns rows deleted.
pub fn prune_other_wallets(
    conn: &mut SqliteConnection,
    keep_wallet_ids: &[i64],
) -> Result<usize, AppError> {
    if keep_wallet_ids.is_empty() {
        return Ok(diesel::delete(wallet_transactions::table).execute(conn)?);
    }
    use diesel::dsl::not;
    let n = diesel::delete(
        wallet_transactions::table
            .filter(not(wallet_transactions::wallet_id.eq_any(keep_wallet_ids))),
    )
    .execute(conn)?;
    Ok(n)
}
