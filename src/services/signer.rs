//! Signing abstraction — pluggable transaction signing providers.
//!
//! The server uses the built-in HD wallet signer. Keys are loaded when the
//! user unlocks the keystore in the admin panel.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Result of a sign operation.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum SignResult {
    /// Transaction is fully signed and ready to broadcast.
    Signed {
        /// Hex-encoded signed transaction.
        tx_hex: String,
    },
    /// Transaction requires external signing.
    Unsigned {
        /// Unique ID for tracking this unsigned transaction.
        unsigned_tx_id: String,
        /// JSON-serializable transaction data for external wallets.
        /// The structure depends on the target wallet protocol.
        tx_data: serde_json::Value,
    },
}

/// Input to the signing process — a transaction description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRequest {
    /// Human-readable operation label (e.g. "create_order", "cancel_order").
    pub operation: String,
    /// Transaction hex (or placeholder) to sign.
    pub tx_hex: String,
    /// Additional context for external signers (e.g. expected outputs).
    pub context: serde_json::Value,
}

/// Pluggable signing provider.
///
/// Each signer implementation provides its own key management strategy.
/// The server selects a signer at startup based on `signing_mode` config.
#[async_trait]
pub trait Signer: Send + Sync {
    /// Sign a transaction. Returns either a signed tx or unsigned data
    /// for an external wallet to sign.
    async fn sign(&self, request: SignRequest) -> Result<SignResult, AppError>;

    /// Get the lock hashes this signer can sign for.
    /// Used by the auto-match engine to verify ownership.
    fn lock_hashes(&self) -> Vec<[u8; 32]>;

    /// Human-readable label for this signer (e.g. "Internal", "JoyID").
    fn label(&self) -> &str;
}
