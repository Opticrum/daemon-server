//! Chain cache integration tests.

mod common;

use common::test_app_state;
use rust_server::services::cached_chain_provider::CachedChainProvider;
use rust_server::services::chain_cache::ChainCache;
use rust_server::services::chain_provider::ChainProvider;
use rust_server::services::runtime_config::RuntimeConfig;
use rust_server::services::MockChainProvider;
use std::sync::{Arc, RwLock};

#[actix_rt::test]
async fn refresh_populates_cache_from_mock_provider() {
    let cache = ChainCache::new();
    let provider = MockChainProvider::new();

    cache.refresh(&provider).await.unwrap();
    let status = cache.status();
    assert!(status.updated_at_ms > 0);
    assert!(!status.refreshing);
}

#[actix_rt::test]
async fn cache_reads_return_populated_snapshot() {
    let state = test_app_state();
    state.cached_chain.refresh_cache().await.unwrap();

    let orders = state.chain_provider.scan_orders().await.unwrap();
    assert_eq!(
        orders.len(),
        state.chain_cache.status().order_count as usize
    );
}

#[actix_rt::test]
async fn disabled_cache_falls_back_to_live_scan() {
    let cache = Arc::new(ChainCache::new());
    let inner = Arc::new(MockChainProvider::new());
    cache.refresh(inner.as_ref()).await.unwrap();

    let runtime_config = Arc::new(RwLock::new(RuntimeConfig {
        chain_cache_enabled: false,
        ..RuntimeConfig::from_config(&rust_server::config::Config::default())
    }));
    let cached = CachedChainProvider::new(inner, cache, runtime_config);

    let orders = cached.scan_orders().await.unwrap();
    assert!(orders.is_empty());
}
