//! In-memory registry of on-chain transactions that have been sent but not
//! yet confirmed.  The web console polls `GET /api/console/transactions/pending`
//! so it can display the tx hash in the blocking "waiting for confirmation"
//! modal as soon as the transaction hits the chain.

use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ==================== Types ====================

/// One in-flight on-chain transaction.
#[derive(Clone, Debug, serde::Serialize)]
pub struct PendingTx {
    pub kind: String,    // "match_order" | "extract_rent" | "destroy_match"
    pub context: String, // match_order: order tx hash; extract/destroy: "{match_tx}:{idx}"
    pub tx_hash: String, // hex, no 0x prefix (same encoding as assembler returns)
    pub sent_at_ms: u64, // unix millis
}

// ==================== Registry ====================

/// Thread-safe in-memory registry of pending on-chain transactions.
///
/// Writers (TransactionAssembler) and the read-only poll endpoint share this
/// via `Arc<PendingTxRegistry>`.  The inner `RwLock` is held briefly, never
/// across `.await`.
#[derive(Default)]
pub struct PendingTxRegistry {
    inner: RwLock<Vec<PendingTx>>,
}

/// Entries older than this are evicted by `register` / `snapshot`.
/// 300 s (CONFIRM_TIMEOUT_SECS) + 60 s grace → 360 s.
const STALE_MS: u64 = 360_000;

/// Hard upper bound so an unbounded leak doesn't grow the Vec forever.
const MAX_PENDING: usize = 64;

/// Get current unix time in milliseconds.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl PendingTxRegistry {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Vec::new()),
        }
    }

    /// Register a newly-sent transaction.
    pub fn register(&self, kind: &str, context: &str, tx_hash: &str) {
        let mut entries = self.inner.write().unwrap_or_else(|e| e.into_inner());
        // Evict stale entries first.
        let cutoff = now_ms().saturating_sub(STALE_MS);
        entries.retain(|e| e.sent_at_ms > cutoff);
        // Evict oldest if at capacity.
        while entries.len() >= MAX_PENDING {
            entries.remove(0);
        }
        entries.push(PendingTx {
            kind: kind.to_string(),
            context: context.to_string(),
            tx_hash: tx_hash.to_string(),
            sent_at_ms: now_ms(),
        });
    }

    /// Remove a transaction from the registry (called when confirmed, rejected, or errored).
    pub fn resolve(&self, tx_hash: &str) {
        let mut entries = self.inner.write().unwrap_or_else(|e| e.into_inner());
        entries.retain(|e| e.tx_hash != tx_hash);
    }

    /// Return a snapshot of all currently pending transactions, purging stale
    /// entries first so the caller never sees long-dead items.
    pub fn snapshot(&self) -> Vec<PendingTx> {
        let mut entries = self.inner.write().unwrap_or_else(|e| e.into_inner());
        let cutoff = now_ms().saturating_sub(STALE_MS);
        entries.retain(|e| e.sent_at_ms > cutoff);
        entries.clone()
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// Build an entry with a custom `sent_at_ms` so tests can control staleness.
    /// Not exposed via the public API — only available in `#[cfg(test)]`.
    fn register_at(
        reg: &PendingTxRegistry,
        kind: &str,
        context: &str,
        tx_hash: &str,
        sent_at_ms: u64,
    ) {
        let mut entries = reg.inner.write().unwrap();
        entries.push(PendingTx {
            kind: kind.to_string(),
            context: context.to_string(),
            tx_hash: tx_hash.to_string(),
            sent_at_ms,
        });
    }

    #[test]
    fn register_and_snapshot_roundtrip() {
        let reg = PendingTxRegistry::new();
        reg.register("extract_rent", "abc:0", "deadbeef");
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].kind, "extract_rent");
        assert_eq!(snap[0].context, "abc:0");
        assert_eq!(snap[0].tx_hash, "deadbeef");
    }

    #[test]
    fn resolve_removes_entry() {
        let reg = PendingTxRegistry::new();
        reg.register("match_order", "ord1", "aaa");
        reg.register("extract_rent", "m1:0", "bbb");
        reg.resolve("aaa");
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].tx_hash, "bbb");
    }

    #[test]
    fn resolve_nonexistent_is_noop() {
        let reg = PendingTxRegistry::new();
        reg.register("match_order", "ord1", "aaa");
        reg.resolve("zzz");
        assert_eq!(reg.snapshot().len(), 1);
    }

    #[test]
    fn evicts_oldest_at_capacity() {
        let reg = PendingTxRegistry::new();
        for i in 0..MAX_PENDING + 5 {
            reg.register("match_order", &format!("ord{i}"), &format!("hash{i}"));
        }
        let snap = reg.snapshot();
        assert_eq!(snap.len(), MAX_PENDING);
        // First 5 entries should have been evicted.
        assert!(!snap.iter().any(|e| e.context == "ord0"));
        assert_eq!(snap[0].context, "ord5");
    }

    #[test]
    fn purges_stale_on_register() {
        let reg = PendingTxRegistry::new();
        let now = now_ms();
        let stale_ms = now.saturating_sub(STALE_MS + 10_000);
        register_at(&reg, "match_order", "stale", "hash_stale", stale_ms);
        register_at(&reg, "match_order", "fresh", "hash_fresh", now);
        // Trigger eviction via register
        reg.register("match_order", "another", "hash_another");
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 2);
        assert!(snap.iter().any(|e| e.context == "fresh"));
        assert!(!snap.iter().any(|e| e.context == "stale"));
    }

    #[test]
    fn purges_stale_on_snapshot() {
        let reg = PendingTxRegistry::new();
        let now = now_ms();
        let stale_ms = now.saturating_sub(STALE_MS + 5_000);
        register_at(&reg, "extract_rent", "stale", "hash_stale", stale_ms);

        // Even without register, snapshot itself cleans up.
        let snap = reg.snapshot();
        assert!(snap.is_empty());
    }

    #[test]
    fn concurrent_register_and_snapshot() {
        let reg = std::sync::Arc::new(PendingTxRegistry::new());
        let reg_clone = reg.clone();
        let handle = thread::spawn(move || {
            for i in 0..50 {
                reg_clone.register("match_order", &format!("ord{i}"), &format!("hash{i}"));
                thread::sleep(Duration::from_millis(1));
            }
        });
        for _ in 0..25 {
            let _ = reg.snapshot();
            thread::sleep(Duration::from_millis(2));
        }
        handle.join().unwrap();
        let final_snap = reg.snapshot();
        assert!(!final_snap.is_empty());
        // All entries should be by consecutive ordN from the writer.
        for e in &final_snap {
            assert_eq!(e.kind, "match_order");
            assert!(e.context.starts_with("ord"));
        }
    }
}
