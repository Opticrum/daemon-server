//! Chain interaction — CKB RPC abstraction, caching, and network operations.
//!
//! - `chain_provider` — the `ChainProvider` trait and `MockChainProvider`
//! - `real_chain_provider` — production CKB RPC implementation
//! - `cached_chain_provider` — transparent cache wrapper
//! - `chain_cache` — in-memory snapshot store

pub mod cached_chain_provider;
pub mod chain_cache;
pub mod chain_provider;
pub mod real_chain_provider;
