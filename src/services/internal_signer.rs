//! Internal signer — signs transactions with a server-stored private key.
//!
//! Loads the configured wallet from the database, decrypts the private key
//! using the server's encryption password, and signs transactions in-process.
//! Required for automated matching.

use async_trait::async_trait;
use secp256k1::{Message, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

use crate::db::wallets::WalletRecord;
use crate::error::AppError;
use crate::services::crypto;
use crate::services::signer::{SignRequest, SignResult, Signer};

/// Signs transactions using a server-stored encrypted private key.
pub struct InternalSigner {
    wallet: WalletRecord,
    secret_key: SecretKey,
}

impl InternalSigner {
    /// Create a new internal signer from a wallet record.
    ///
    /// Decrypts the wallet's encrypted private key using the server password.
    pub fn new(wallet: WalletRecord, encryption_password: &str) -> Result<Self, AppError> {
        let secret_key = crypto::decrypt_secret_key(&wallet.encrypted_key, encryption_password)?;
        Ok(Self { wallet, secret_key })
    }

    /// Get a reference to the wallet this signer uses.
    pub fn wallet(&self) -> &WalletRecord {
        &self.wallet
    }

    /// Get a reference to the decrypted secret key (for real CKB transaction signing).
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    /// Derive the CKB Address from this signer's secret key.
    /// Returns a placeholder if the address derivation is not fully wired.
    pub fn ckb_address(&self) -> &str {
        &self.wallet.ckb_address
    }

    /// Sign raw bytes with secp256k1 and return the compact signature.
    fn sign_bytes(&self, message: &[u8]) -> [u8; 64] {
        let secp = Secp256k1::new();
        let msg_hash = Sha256::digest(message);
        let msg = Message::from_digest_slice(&msg_hash).expect("sha256 is 32 bytes");
        let sig = secp.sign_ecdsa(&msg, &self.secret_key);
        sig.serialize_compact()
    }
}

#[async_trait]
impl Signer for InternalSigner {
    async fn sign(&self, request: SignRequest) -> Result<SignResult, AppError> {
        // For Phase 3, signing is done by appending a secp256k1 signature
        // to the placeholder tx_hex. Phase 6 will wire real CKB transaction
        // assembly and proper witness-based signing.
        let message = format!("{}:{}", request.operation, request.tx_hex);
        let signature = self.sign_bytes(message.as_bytes());

        // Append signature to the placeholder tx for traceability
        let signed_tx_hex = format!("{}:sig={}", request.tx_hex, hex::encode(signature));

        tracing::info!(
            "InternalSigner signed {} (wallet_id={}, label={})",
            request.operation,
            self.wallet.id,
            self.wallet.label
        );

        Ok(SignResult::Signed {
            tx_hex: signed_tx_hex,
        })
    }

    fn lock_hashes(&self) -> Vec<[u8; 32]> {
        let mut hash = [0u8; 32];
        let len = self.wallet.lock_hash.len().min(32);
        hash[..len].copy_from_slice(&self.wallet.lock_hash[..len]);
        vec![hash]
    }

    fn label(&self) -> &str {
        "Internal"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::wallets;
    use crate::services::crypto;

    /// A valid secp256k1 secret key for testing.
    /// This is the SHA-256 of "opticrum-internal-signer-test-key-0001".
    fn test_secret_key_bytes() -> [u8; 32] {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"opticrum-internal-signer-test-key-0001");
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }

    #[actix_rt::test]
    async fn internal_signer_signs_and_returns_key_info() {
        let pool = db::init_test_db();
        let mut conn = pool.get().unwrap();

        // Import a test wallet with a valid secp256k1 key
        let test_key = test_secret_key_bytes();
        let encrypted = crypto::encrypt(&test_key, "test-pw").unwrap();
        let lock_hash = [0x42u8; 32];
        wallets::insert_wallet(
            &mut conn, "test", &encrypted, &lock_hash, "addr", None, None, None, "imported",
        )
        .unwrap();
        let wallet = wallets::get_wallet_by_id(&mut conn, 1).unwrap();

        let signer = InternalSigner::new(wallet, "test-pw").expect("should create signer");

        // Verify lock hashes
        let hashes = signer.lock_hashes();
        assert_eq!(hashes.len(), 1);

        // Verify label
        assert_eq!(signer.label(), "Internal");

        let request = SignRequest {
            operation: "create_order".into(),
            tx_hex: "deadbeef".into(),
            context: serde_json::json!({}),
            signer_address: None,
        };

        let result = signer.sign(request).await.expect("should sign");
        match result {
            SignResult::Signed { tx_hex } => {
                assert!(tx_hex.starts_with("deadbeef:sig="));
            }
            _ => panic!("Expected Signed result"),
        }
    }
}
