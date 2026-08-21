//! Service modules — core business logic.
//!
//! Services abstract over `ChainProvider` so they can be tested with
//! a `MockChainProvider` without a real CKB node.
//!
//! # Module structure
//!
//! - `chain/` — CKB chain interaction (provider trait, RPC, caching)
//! - `wallet/` — key management, HD derivation, signing
//! - `console/` — dashboard gateway and scheduler observability
//! - Top-level files — match, rent, transaction assembly, runtime config

pub mod chain;
pub mod console;
pub mod match_inflight;
pub mod match_service;
pub mod pending_txs;
pub mod rent_service;
pub mod runtime_config;
pub mod transaction_assembler;
pub mod wallet;
pub mod wallet_tx;

// ---------------------------------------------------------------------------
// Re-exports — preserve backward compatibility for all existing import paths.
// Existing code using `crate::services::chain_provider::ChainProvider` etc.
// continues to compile unchanged.
// ---------------------------------------------------------------------------

// Chain sub-modules re-exported at the services:: level
pub use chain::cached_chain_provider;
pub use chain::chain_cache;
pub use chain::chain_provider;
pub use chain::real_chain_provider;

// Wallet sub-modules re-exported at the services:: level
pub use wallet::address;
pub use wallet::crypto;
pub use wallet::hd_wallet;
pub use wallet::hd_wallet_signer;
pub use wallet::internal_signer;
pub use wallet::keystore;
pub use wallet::signer;
pub use wallet::wallet_service;
pub use wallet::wallet_session;

// Key types re-exported at the services:: level for convenience
pub use chain::cached_chain_provider::CachedChainProvider;
pub use chain::chain_cache::ChainCache;
pub use chain::chain_provider::MockChainProvider;
pub use chain::real_chain_provider::RealChainProvider;
pub use match_inflight::MatchInflight;
pub use pending_txs::PendingTxRegistry;
pub use runtime_config::RuntimeConfig;
