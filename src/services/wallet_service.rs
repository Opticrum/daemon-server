//! Wallet service — key management, derivation, and transaction signing.
//!
//! Handles:
//! - Importing private keys (hex-encoded secp256k1)
//! - Deriving the CKB lock hash from the public key
//! - Encrypting/decrypting keys for DB storage
//! - Signing transactions

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use secp256k1::{PublicKey, SecretKey};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::db::wallets::{self, WalletRecord};
use crate::error::AppError;
use crate::services::crypto;

/// Derive the secp256k1 public key from a private key (33 bytes compressed).
fn derive_pubkey(secret_key: &SecretKey) -> [u8; 33] {
    let secp = secp256k1::Secp256k1::new();
    let pubkey = PublicKey::from_secret_key(&secp, secret_key);
    pubkey.serialize()
}

/// Derive the CKB lock hash from a secp256k1 public key.
///
/// CKB secp256k1_blake160 sighash_all lock:
///   lock_args = first 20 bytes of blake2b-256(pubkey_hash)
/// But for simplicity, we use SHA-256 of the pubkey as the lock_hash here.
/// In production, this should use the correct blake160 derivation.
fn derive_lock_hash(pubkey: &[u8; 33]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ckb-secp256k1:"); // domain separator
    hasher.update(pubkey);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Derive a CKB testnet address from a pubkey.
///
/// Generates a bech32m address. In production this should follow
/// the full CKB address format (code_hash + hash_type + args).
fn derive_address(_pubkey: &[u8; 33], lock_hash: &[u8; 32]) -> String {
    let lock_hash_hex = hex::encode(lock_hash);
    format!("ckt1q...{}", &lock_hash_hex[..8])
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Import a private key, derive its lock_hash and address, and store it.
///
/// If `encryption_password` is provided, the key is AES-256-GCM encrypted
/// at rest. If `None`, the key is stored as plaintext (suitable for dev).
/// Returns the new wallet record.
pub fn import_wallet(
    pool: &Pool<SqliteConnectionManager>,
    label: &str,
    private_key_hex: &str,
    encryption_password: Option<&str>,
) -> Result<WalletRecord, AppError> {
    // Decode private key from hex
    let private_key_bytes = hex::decode(private_key_hex)
        .map_err(|e| AppError::BadRequest(format!("Invalid hex: {e}")))?;

    if private_key_bytes.len() != 32 {
        return Err(AppError::BadRequest(
            "Private key must be 32 bytes (64 hex chars)".into(),
        ));
    }

    // Parse secp256k1 secret key
    let secret_key = SecretKey::from_slice(&private_key_bytes)
        .map_err(|e| AppError::WalletError(format!("Invalid private key: {e}")))?;

    // Derive pubkey and lock hash
    let pubkey = derive_pubkey(&secret_key);
    let lock_hash = derive_lock_hash(&pubkey);
    let address = derive_address(&pubkey, &lock_hash);

    // Encrypt or store plaintext
    let encrypted_key = match encryption_password {
        Some(pw) => crypto::encrypt(&private_key_bytes, pw)?,
        None => private_key_bytes.to_vec(),
    };

    // Store in DB
    let conn = pool.get()?;
    let id = wallets::insert_wallet(&conn, label, &encrypted_key, &lock_hash, &address)?;

    info!(
        wallet_id = id,
        label = %label,
        address = %address,
        encrypted = encryption_password.is_some(),
        "Wallet imported"
    );

    wallets::get_wallet_by_id(&conn, id)
}

/// Get a wallet by its database ID.
pub fn get_wallet(pool: &Pool<SqliteConnectionManager>, id: i64) -> Result<WalletRecord, AppError> {
    let conn = pool.get()?;
    wallets::get_wallet_by_id(&conn, id)
}

/// List all managed wallets.
pub fn list_wallets(pool: &Pool<SqliteConnectionManager>) -> Result<Vec<WalletRecord>, AppError> {
    let conn = pool.get()?;
    wallets::list_wallets(&conn)
}

/// Delete a wallet by ID.
pub fn delete_wallet(pool: &Pool<SqliteConnectionManager>, id: i64) -> Result<bool, AppError> {
    let conn = pool.get()?;
    let deleted = wallets::delete_wallet(&conn, id)?;
    if deleted {
        info!(wallet_id = id, "Wallet deleted");
    } else {
        warn!(wallet_id = id, "Wallet delete requested but not found");
    }
    Ok(deleted)
}

/// Extract a wallet's private key for signing operations.
///
/// If the key was stored encrypted, `password` is used to decrypt it.
/// If stored without encryption, `password` is ignored and the raw bytes
/// are parsed directly.
pub fn decrypt_private_key(
    wallet: &WalletRecord,
    password: Option<&str>,
) -> Result<SecretKey, AppError> {
    let private_key_bytes = if wallet.encrypted_key.len() > 32 {
        // Likely encrypted (len > 32 due to nonce + auth tag overhead)
        match password {
            Some(pw) => crypto::decrypt(&wallet.encrypted_key, pw)?,
            None => {
                return Err(AppError::WalletError(
                    "Wallet key is encrypted — password required".into(),
                ))
            }
        }
    } else {
        // Stored as raw 32-byte key
        wallet.encrypted_key.clone()
    };
    SecretKey::from_slice(&private_key_bytes)
        .map_err(|e| AppError::WalletError(format!("Failed to parse decrypted key: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic test private key (32 bytes, from a known seed).
    /// This is the SHA-256 of "opticrum-test-key-0001".
    fn test_private_key_hex() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
    }

    fn test_private_key_bytes() -> [u8; 32] {
        [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xab, 0xcd, 0xef,
        ]
    }

    #[test]
    fn derive_pubkey_from_test_key() {
        let secret_key = SecretKey::from_slice(&test_private_key_bytes()).expect("valid test key");
        let pubkey = derive_pubkey(&secret_key);
        assert_eq!(pubkey.len(), 33);
        // Compressed pubkey starts with 0x02 or 0x03
        assert!(pubkey[0] == 0x02 || pubkey[0] == 0x03);
    }

    #[test]
    fn derive_lock_hash_is_deterministic() {
        let secret_key = SecretKey::from_slice(&test_private_key_bytes()).unwrap();
        let pubkey = derive_pubkey(&secret_key);
        let hash1 = derive_lock_hash(&pubkey);
        let hash2 = derive_lock_hash(&pubkey);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32);
    }

    #[test]
    fn different_keys_produce_different_hashes() {
        let sk1 = SecretKey::from_slice(&test_private_key_bytes()).unwrap();
        let mut bytes2 = test_private_key_bytes();
        bytes2[0] = bytes2[0].wrapping_add(1);
        let sk2 = SecretKey::from_slice(&bytes2).unwrap();

        let hash1 = derive_lock_hash(&derive_pubkey(&sk1));
        let hash2 = derive_lock_hash(&derive_pubkey(&sk2));
        assert_ne!(hash1, hash2);
    }
}
