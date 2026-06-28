//! External signer — produces unsigned transaction data for external wallets.
//!
//! Supports external signing providers like JoyID, UTXOGlobal, etc.
//! The server prepares the transaction and the external wallet provides
//! signatures through the `/api/transactions/unsigned/*` endpoints.

use async_trait::async_trait;
use std::sync::Mutex;

use crate::error::AppError;
use crate::services::signer::{SignRequest, SignResult, Signer};

/// External signing provider.
///
/// Does not hold any private keys. Instead, produces unsigned transaction
/// data that the admin panel or external wallet can sign.
pub struct ExternalSigner {
    /// Counter for generating unique unsigned tx IDs.
    counter: Mutex<u64>,
    /// CKB network the server is connected to ("testnet" or "mainnet").
    network: String,
}

impl ExternalSigner {
    pub fn new(network: &str) -> Self {
        Self {
            counter: Mutex::new(0),
            network: network.to_string(),
        }
    }

    fn next_id(&self) -> String {
        let mut c = self.counter.lock().unwrap();
        *c += 1;
        format!("unsigned-{}", c)
    }
}

impl Default for ExternalSigner {
    fn default() -> Self {
        Self::new("testnet")
    }
}

#[async_trait]
impl Signer for ExternalSigner {
    async fn sign(&self, request: SignRequest) -> Result<SignResult, AppError> {
        let unsigned_tx_id = self.next_id();

        // Build a JSON structure compatible with CKB external wallets.
        // The structure follows the pattern used by JoyID/UTXOGlobal:
        // transaction hex + context about what's being signed.
        let tx_data = serde_json::json!({
            "operation": request.operation,
            "tx_hex": request.tx_hex,
            "context": request.context,
            "unsigned_tx_id": unsigned_tx_id,
            "network": self.network,
        });

        tracing::info!(
            "ExternalSigner created unsigned tx {} for operation {}",
            unsigned_tx_id,
            request.operation
        );

        Ok(SignResult::Unsigned {
            unsigned_tx_id,
            tx_data,
        })
    }

    fn lock_hashes(&self) -> Vec<[u8; 32]> {
        // External signer doesn't manage keys — lock hashes are provided
        // by the external wallet when submitting signed witnesses.
        Vec::new()
    }

    fn label(&self) -> &str {
        "External"
    }
}
