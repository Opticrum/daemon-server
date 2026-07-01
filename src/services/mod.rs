//! Service modules — core business logic.
//!
//! Services abstract over `ChainProvider` so they can be tested with
//! a `MockChainProvider` without a real CKB node.

pub mod address;
pub mod chain_provider;
pub mod console;
pub mod crypto;
pub mod external_signer;
pub mod hd_wallet;
pub mod hd_wallet_signer;
pub mod internal_signer;
pub mod keystore;
pub mod match_service;
pub mod real_chain_provider;
pub mod rent_service;
pub mod runtime_config;
pub mod signer;
pub mod transaction_assembler;
pub mod wallet_service;
pub mod wallet_session;

pub use chain_provider::MockChainProvider;
pub use real_chain_provider::RealChainProvider;
pub use runtime_config::RuntimeConfig;
