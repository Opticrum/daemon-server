//! Wallet key management, HD derivation, signing, and session management.
//!
//! - `wallet_service` — orchestration: create HD wallet, import keys, unlock
//! - `hd_wallet` — BIP39/BIP32/BIP44 derivation math
//! - `keystore` — AES-256-GCM encrypted mnemonic persistence
//! - `wallet_session` — HttpOnly cookie session management
//! - `crypto` — AES-256-GCM encrypt/decrypt primitives
//! - `address` — CKB address derivation from pubkey
//! - `signer` — pluggable transaction signing trait
//! - `internal_signer` — legacy single-key signer
//! - `hd_wallet_signer` — in-process signer using decrypted HD child keys

pub mod address;
pub mod crypto;
pub mod hd_wallet;
pub mod hd_wallet_signer;
pub mod internal_signer;
pub mod keystore;
pub mod signer;
pub mod wallet_service;
pub mod wallet_session;
