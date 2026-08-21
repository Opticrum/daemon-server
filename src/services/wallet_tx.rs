//! Wallet transaction history — chain sync to DB + DB-backed read.
//!
//! `sync` is the single place that talks to the indexer/RPC for wallet tx
//! history; it persists per-(tx_hash, wallet_id) rows into `wallet_transactions`.
//! `list` is a pure DB read that aggregates rows across all managed wallets.
//! Both are used by the background scheduler and the console API.

use std::collections::HashMap;

use serde::Serialize;

use crate::db::wallet_txs as wallet_txs_db;
use crate::db::wallets as wallet_db;
use crate::db::DbPool;
use crate::error::AppError;
use crate::services::address::lock_arg_from_address;
use crate::services::chain_provider::{ChainProvider, TxInputInfo};

/// One row to persist (per (tx_hash, wallet_id)).
pub struct WalletTxRecord {
    pub tx_hash: String,
    pub wallet_id: i64,
    pub block_number: u64,
    pub timestamp_ms: Option<i64>,
    pub received_shannons: u64,
    pub sent_shannons: u64,
}

/// Result of a sync cycle (also serialized as the POST /sync response).
#[derive(Debug, Clone, Serialize)]
pub struct SyncStats {
    pub wallets: usize,
    pub rows_synced: usize,
    pub pruned: usize,
}

/// Response shape for GET /console/wallets/transactions (aggregated across
/// all managed wallet addresses).
#[derive(Clone, Debug, Serialize)]
pub struct WalletTx {
    pub tx_hash: String,
    pub block_number: u64,
    pub timestamp_ms: Option<i64>,
    pub received_shannons: u64,
    pub sent_shannons: u64,
    pub addresses: Vec<String>,
}

/// Per-transaction aggregation accumulator.
#[derive(Default)]
struct TxAgg {
    block_number: u64,
    timestamp_ms: Option<i64>,
    received_shannons: u64,
    sent_shannons: u64,
    addresses: Vec<String>,
    io_refs: Vec<(String, u32)>, // (io_type, io_index)
}

/// Fetch the latest window of wallet tx history for every managed wallet and
/// persist it, then prune rows for wallets that no longer exist.
pub async fn sync(pool: &DbPool, provider: &dyn ChainProvider) -> Result<SyncStats, AppError> {
    // Read wallets, then release the connection before the (slow) chain work.
    let wallets = {
        let mut conn = pool.get()?;
        wallet_db::list_wallets(&mut conn)?
    };

    // lock_arg → (wallet_id, label); dedup by lock_arg, skip unparseable.
    let mut by_lock_arg: HashMap<[u8; 20], (i64, String)> = HashMap::new();
    for w in &wallets {
        if let Ok(lock_arg) = lock_arg_from_address(&w.ckb_address) {
            by_lock_arg
                .entry(lock_arg)
                .or_insert_with(|| (w.id, w.label.clone()));
        }
    }

    let mut records: Vec<WalletTxRecord> = Vec::new();
    let mut ts_cache: HashMap<u64, Option<i64>> = HashMap::new();

    for (lock_arg, (wallet_id, _label)) in &by_lock_arg {
        let refs = provider.get_transactions_by_lock_arg(lock_arg).await?;

        // Per-wallet aggregation by tx_hash.
        let mut agg: HashMap<String, TxAgg> = HashMap::new();
        for r in refs {
            let e = agg.entry(r.tx_hash.clone()).or_default();
            e.block_number = r.block_number;
            e.io_refs.push((r.io_type, r.io_index));
        }

        // Resolve amounts per tx; cache previous-output capacities so each input's
        // source transaction is fetched at most once.
        let mut prev_cache: HashMap<String, Vec<u64>> = HashMap::new();
        for (tx_hash, e) in agg.iter_mut() {
            let Ok(info) = provider.get_transaction(tx_hash).await else {
                tracing::debug!(tx_hash = %tx_hash, "wallet_tx sync: failed to fetch tx, skipping");
                continue;
            };
            for (io_type, io_index) in &e.io_refs {
                let idx = *io_index as usize;
                if io_type == "output" {
                    if let Some(o) = info.outputs.get(idx) {
                        e.received_shannons += o.capacity;
                    }
                } else if let Some(input) = info.inputs.get(idx) {
                    if let Some(cap) = prev_output_capacity(provider, &mut prev_cache, input).await
                    {
                        e.sent_shannons += cap;
                    }
                }
            }

            let ts = resolve_block_timestamp(provider, e.block_number, &mut ts_cache).await;
            records.push(WalletTxRecord {
                tx_hash: tx_hash.clone(),
                wallet_id: *wallet_id,
                block_number: e.block_number,
                timestamp_ms: ts,
                received_shannons: e.received_shannons,
                sent_shannons: e.sent_shannons,
            });
        }
    }

    let rows_synced = records.len();
    let new_rows: Vec<wallet_txs_db::NewWalletTx> = records
        .iter()
        .map(|r| wallet_txs_db::NewWalletTx {
            tx_hash: &r.tx_hash,
            wallet_id: r.wallet_id,
            block_number: r.block_number as i64,
            timestamp_ms: r.timestamp_ms,
            received_shannons: r.received_shannons as i64,
            sent_shannons: r.sent_shannons as i64,
        })
        .collect();

    let mut conn = pool.get()?;
    wallet_txs_db::upsert_batch(&mut conn, &new_rows)?;
    let keep_ids: Vec<i64> = wallets.iter().map(|w| w.id).collect();
    let pruned = wallet_txs_db::prune_other_wallets(&mut conn, &keep_ids)?;

    tracing::info!(
        wallets = wallets.len(),
        rows_synced,
        pruned,
        "Wallet tx sync complete"
    );
    Ok(SyncStats {
        wallets: wallets.len(),
        rows_synced,
        pruned,
    })
}

/// Pure DB read: aggregate `wallet_transactions` by tx_hash, newest block
/// first. No chain/RPC access.
///
/// With `wallet_id = Some(id)`, only that wallet's rows are returned (each
/// transaction tagged with the single wallet). With `None`, rows are aggregated
/// across all managed wallets.
pub async fn list(pool: &DbPool, wallet_id: Option<i64>) -> Result<Vec<WalletTx>, AppError> {
    let mut conn = pool.get()?;
    let rows = match wallet_id {
        Some(id) => wallet_txs_db::list_wallet_txs(&mut conn, id)?,
        None => wallet_txs_db::list_all(&mut conn)?,
    };
    let wallets = wallet_db::list_wallets(&mut conn)?;

    let label_by_id: HashMap<i64, String> =
        wallets.iter().map(|w| (w.id, w.label.clone())).collect();
    // Rows for deleted-but-not-yet-pruned wallets are skipped.
    let live_ids: std::collections::HashSet<i64> = label_by_id.keys().copied().collect();

    let mut agg: HashMap<String, TxAgg> = HashMap::new();
    for row in rows {
        if !live_ids.contains(&row.wallet_id) {
            continue; // stale row for a deleted wallet
        }
        let e = agg.entry(row.tx_hash.clone()).or_default();
        e.block_number = row.block_number as u64;
        if e.timestamp_ms.is_none() {
            e.timestamp_ms = row.timestamp_ms;
        }
        e.received_shannons += row.received_shannons as u64;
        e.sent_shannons += row.sent_shannons as u64;
        if let Some(label) = label_by_id.get(&row.wallet_id) {
            if !e.addresses.iter().any(|a| a == label) {
                e.addresses.push(label.clone());
            }
        }
    }

    let mut out: Vec<WalletTx> = agg
        .into_iter()
        .map(|(tx_hash, e)| WalletTx {
            tx_hash,
            block_number: e.block_number,
            timestamp_ms: e.timestamp_ms,
            received_shannons: e.received_shannons,
            sent_shannons: e.sent_shannons,
            addresses: e.addresses,
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.block_number));
    Ok(out)
}

/// Capacity of the cell an input spends, resolved from its previous
/// transaction (cached per previous tx so it is fetched at most once).
async fn prev_output_capacity(
    provider: &dyn ChainProvider,
    cache: &mut HashMap<String, Vec<u64>>,
    input: &TxInputInfo,
) -> Option<u64> {
    if let Some(caps) = cache.get(&input.previous_tx_hash) {
        return caps.get(input.previous_index as usize).copied();
    }
    let info = provider
        .get_transaction(&input.previous_tx_hash)
        .await
        .ok()?;
    let caps: Vec<u64> = info.outputs.iter().map(|o| o.capacity).collect();
    let cap = caps.get(input.previous_index as usize).copied();
    cache.insert(input.previous_tx_hash.clone(), caps);
    cap
}

/// Resolve a block's Unix-ms timestamp, cached per block per sync cycle.
/// Returns `None` when the provider reports 0 (unavailable) or errors.
async fn resolve_block_timestamp(
    provider: &dyn ChainProvider,
    block_number: u64,
    cache: &mut HashMap<u64, Option<i64>>,
) -> Option<i64> {
    if let Some(ts) = cache.get(&block_number) {
        return *ts;
    }
    let ts = provider
        .get_block_timestamp(block_number)
        .await
        .ok()
        .filter(|&ts| ts > 0)
        .map(|ts| ts as i64);
    cache.insert(block_number, ts);
    ts
}
