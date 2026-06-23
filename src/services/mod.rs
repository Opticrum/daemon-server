//! Service modules — core business logic.
//!
//! Services abstract over `ChainProvider` so they can be tested with
//! a `MockChainProvider` without a real CKB node.

pub mod chain_provider;
pub mod crypto;
pub mod external_signer;
pub mod internal_signer;
pub mod match_service;
pub mod order_service;
pub mod real_chain_provider;
pub mod rent_service;
pub mod signer;
pub mod transaction_assembler;
pub mod wallet_service;

pub use chain_provider::MockChainProvider;
pub use real_chain_provider::RealChainProvider;
