//! In-flight match guard — prevents duplicate match submissions for one order.
//!
//! A match transaction spends the order cell, but until that transaction
//! confirms on-chain the order still appears in chain scans. A second match
//! request for the same order would then reuse the same Fiber channel and
//! rebuild a byte-identical transaction, which the CKB node rejects as a
//! duplicate broadcast. This guard marks an order as "being matched" so a
//! concurrent or sequential duplicate request fails fast with a clear error
//! instead of surfacing an opaque chain-level rejection.

use std::collections::HashSet;
use std::sync::RwLock;

/// Thread-safe set of order keys currently being matched.
///
/// The key is `"{order_tx_hash}:{order_output_index}"`. The inner `RwLock` is
/// held only briefly and never across an `.await`.
#[derive(Default)]
pub struct MatchInflight {
    inner: RwLock<HashSet<String>>,
}

impl MatchInflight {
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically claim an order key. Returns `true` if the caller may proceed
    /// (the key was not already claimed); `false` if a match for the same order
    /// is already in flight and the caller must abort.
    pub fn begin(&self, key: &str) -> bool {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.insert(key.to_string())
    }

    /// Release the claim for an order key. Call on every exit path (success and
    /// error) so the order can be matched again once the current attempt ends.
    pub fn end(&self, key: &str) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.remove(key);
    }

    /// Whether an order key is currently being matched.
    pub fn is_inflight(&self, key: &str) -> bool {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        inner.contains(key)
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_claims_unique_key() {
        let g = MatchInflight::new();
        assert!(g.begin("order_a:0"));
        assert!(!g.begin("order_a:0"), "second claim of same key must fail");
        // A different output index on the same tx is a distinct order cell.
        assert!(g.begin("order_a:1"));
    }

    #[test]
    fn end_releases_claim() {
        let g = MatchInflight::new();
        assert!(g.begin("order_a:0"));
        g.end("order_a:0");
        assert!(g.begin("order_a:0"), "key must be reclaimable after end");
    }

    #[test]
    fn is_inflight_reflects_state() {
        let g = MatchInflight::new();
        assert!(!g.is_inflight("order_b:0"));
        g.begin("order_b:0");
        assert!(g.is_inflight("order_b:0"));
        g.end("order_b:0");
        assert!(!g.is_inflight("order_b:0"));
    }

    #[test]
    fn end_nonexistent_is_noop() {
        let g = MatchInflight::new();
        g.end("never-begun:0"); // must not panic
        assert!(g.begin("never-begun:0"));
    }
}
