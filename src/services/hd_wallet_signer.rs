//! HD wallet signer — signs transactions with decrypted HD child keys.
//!
//! Keys are loaded into memory when the user unlocks the keystore via the
//! admin panel. Auto-match and manual operations use these keys until the
//! keystore is deleted or the process restarts.

use async_trait::async_trait;
use secp256k1::{Message, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

use crate::db::wallets::{self, WalletRecord};
use crate::db::DbPool;
use crate::error::AppError;
use crate::services::signer::{SignRequest, SignResult, Signer};
use crate::services::wallet_service;

struct HdWalletSignerInner {
    keys: Vec<(WalletRecord, SecretKey)>,
}

/// In-process signer backed by HD wallet child keys.
#[derive(Clone)]
pub struct HdWalletSigner {
    inner: Arc<Mutex<HdWalletSignerInner>>,
}

impl HdWalletSigner {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HdWalletSignerInner { keys: Vec::new() })),
        }
    }

    /// Decrypt and load all HD child keys from the database.
    pub fn load_keys(&self, pool: &DbPool, password: &str) -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let children = wallets::list_wallets_by_type(&mut conn, "hd_child")?;
        if children.is_empty() {
            return Err(AppError::WalletError(
                "No HD wallet addresses found — create or import an HD wallet first".into(),
            ));
        }

        let mut keys = Vec::with_capacity(children.len());
        for wallet in children {
            let secret_key = wallet_service::decrypt_private_key(&wallet, Some(password))?;
            keys.push((wallet, secret_key));
        }

        let mut inner = self
            .inner
            .lock()
            .map_err(|e| AppError::Internal(format!("HD signer lock: {e}")))?;
        inner.keys = keys;
        Ok(())
    }

    /// Clear decrypted keys from memory.
    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.keys.clear();
        }
    }

    /// Return the loaded wallet records (without secret keys).
    /// Returns an empty vec if the signer is locked or the mutex is poisoned.
    pub fn wallet_records(&self) -> Vec<WalletRecord> {
        self.inner
            .lock()
            .map(|inner| inner.keys.iter().map(|(wr, _)| wr.clone()).collect())
            .unwrap_or_default()
    }

    /// Look up the secret key for a specific HD child address.
    /// Returns `None` if the address is not among the loaded keys or the signer is locked.
    pub fn find_key_by_address(&self, address: &str) -> Option<SecretKey> {
        self.inner.lock().ok().and_then(|inner| {
            inner
                .keys
                .iter()
                .find(|(wr, _)| wr.ckb_address == address)
                .map(|(_, sk)| *sk)
        })
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| !inner.keys.is_empty())
            .unwrap_or(false)
    }

    fn sign_bytes(secret_key: &SecretKey, message: &[u8]) -> [u8; 64] {
        let secp = Secp256k1::new();
        let msg_hash = Sha256::digest(message);
        let msg = Message::from_digest_slice(&msg_hash).expect("sha256 is 32 bytes");
        let sig = secp.sign_ecdsa(&msg, secret_key);
        sig.serialize_compact()
    }
}

impl Default for HdWalletSigner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Signer for HdWalletSigner {
    async fn sign(&self, request: SignRequest) -> Result<SignResult, AppError> {
        let inner = self
            .inner
            .lock()
            .map_err(|e| AppError::Internal(format!("HD signer lock: {e}")))?;

        let (wallet, secret_key) = inner.keys.first().ok_or_else(|| {
            AppError::WalletError(
                "HD wallet is locked — unlock it in Wallet Management first".into(),
            )
        })?;

        let message = format!("{}:{}", request.operation, request.tx_hex);
        let signature = Self::sign_bytes(secret_key, message.as_bytes());
        let signed_tx_hex = format!("{}:sig={}", request.tx_hex, hex::encode(signature));

        tracing::info!(
            "HdWalletSigner signed {} (wallet_id={}, label={})",
            request.operation,
            wallet.id,
            wallet.label
        );

        Ok(SignResult::Signed {
            tx_hex: signed_tx_hex,
        })
    }

    fn lock_hashes(&self) -> Vec<[u8; 32]> {
        self.inner
            .lock()
            .map(|inner| {
                inner
                    .keys
                    .iter()
                    .map(|(wallet, _)| {
                        let mut hash = [0u8; 32];
                        let len = wallet.lock_hash.len().min(32);
                        hash[..len].copy_from_slice(&wallet.lock_hash[..len]);
                        hash
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn label(&self) -> &str {
        "HD Wallet"
    }
}
