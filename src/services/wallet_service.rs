//! Wallet service — key management, derivation, and transaction signing.
//!
//! Handles:
//! - Importing private keys (hex-encoded secp256k1)
//! - Creating HD wallets from BIP39 mnemonics
//! - Deriving the CKB lock hash from the public key
//! - Encrypting/decrypting keys for DB storage
//! - Balance aggregation across wallet addresses

use bip39::Mnemonic;
use secp256k1::{PublicKey, SecretKey};
use std::path::Path;
use tracing::{info, warn};

use crate::db::wallets::{self, WalletRecord};
use crate::db::DbPool;
use crate::error::AppError;
use crate::services::address::{
    blake160, ckb_address_from_pubkey, ckb_address_testnet, lock_arg_from_pubkey, script_lock_hash,
};
use crate::services::crypto;
use crate::services::hd_wallet;
use crate::services::keystore::{self, Keystore};

/// Derive the secp256k1 public key from a private key (33 bytes compressed).
fn derive_pubkey(secret_key: &SecretKey) -> [u8; 33] {
    let secp = secp256k1::Secp256k1::new();
    let pubkey = PublicKey::from_secret_key(&secp, secret_key);
    pubkey.serialize()
}

/// Derive the CKB lock hash from a secp256k1 public key (used for imported wallets).
///
/// Uses the same CKB-compliant derivation as HD wallets:
/// `lock_arg = blake2b-256(pubkey)[0..20]` with "ckb-default-hash" personalization,
/// then Molecule-serializes the secp256k1_blake160_sighash_all lock script and hashes it.
fn derive_lock_hash(pubkey: &[u8; 33]) -> [u8; 32] {
    let lock_arg = blake160(pubkey);
    script_lock_hash(&lock_arg)
}

/// Derive a CKB testnet address from a pubkey.
///
/// Produces a CKB2021 bech32m full address, matching ckb-cli output.
fn derive_address(pubkey: &[u8; 33]) -> String {
    let lock_arg = blake160(pubkey);
    ckb_address_testnet(&lock_arg)
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
    pool: &DbPool,
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

    // Derive pubkey, lock hash, and CKB address
    let pubkey = derive_pubkey(&secret_key);
    let lock_hash = derive_lock_hash(&pubkey);
    let address = derive_address(&pubkey);

    // Encrypt or store plaintext
    let encrypted_key = match encryption_password {
        Some(pw) => crypto::encrypt(&private_key_bytes, pw)?,
        None => private_key_bytes.to_vec(),
    };

    // Store in DB
    let mut conn = pool.get()?;
    let id = wallets::insert_wallet(
        &mut conn,
        label,
        &encrypted_key,
        &lock_hash,
        &address,
        None,
        None,
        None,
        "imported",
    )?;

    info!(
        wallet_id = id,
        label = %label,
        address = %address,
        encrypted = encryption_password.is_some(),
        "Wallet imported"
    );

    wallets::get_wallet_by_id(&mut conn, id)
}

/// Get a wallet by its database ID.
pub fn get_wallet(pool: &DbPool, id: i64) -> Result<WalletRecord, AppError> {
    let mut conn = pool.get()?;
    wallets::get_wallet_by_id(&mut conn, id)
}

/// List all managed wallets.
pub fn list_wallets(pool: &DbPool) -> Result<Vec<WalletRecord>, AppError> {
    let mut conn = pool.get()?;
    wallets::list_wallets(&mut conn)
}

/// Delete a wallet by ID.
pub fn delete_wallet(pool: &DbPool, id: i64) -> Result<bool, AppError> {
    let mut conn = pool.get()?;
    let deleted = wallets::delete_wallet(&mut conn, id)?;
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

// ---------------------------------------------------------------------------
// HD Wallet API
// ---------------------------------------------------------------------------

/// Derive the proper CKB lock hash for an HD wallet key (Molecule script hash).
fn derive_lock_hash_hd(pubkey: &PublicKey) -> [u8; 32] {
    script_lock_hash(&lock_arg_from_pubkey(pubkey))
}

/// Create a new HD wallet:
/// 1. Generate a 12-word BIP39 mnemonic
/// 2. Create an encrypted keystore file
/// 3. Derive `address_count` child keys
/// 4. Store each child as a wallet row in the DB
///
/// Returns the keystore, mnemonic phrase (SHOW ONCE!), and the derived wallet records.
pub fn create_hd_wallet(
    pool: &DbPool,
    keystore_path: &Path,
    label: &str,
    password: &str,
    address_count: u32,
) -> Result<(Keystore, String, Vec<WalletRecord>), AppError> {
    let count = if address_count == 0 { 5 } else { address_count };

    // 1. Generate mnemonic
    let mnemonic = hd_wallet::generate_mnemonic()?;
    let phrase = mnemonic.to_string();

    // 2. Create keystore
    let mut keystore = keystore::create_keystore(&mnemonic, password, label, "m/44'/309'/0'/0")?;

    // 3. Derive seed and child keys
    let seed = hd_wallet::mnemonic_to_seed(&mnemonic, "");
    let mut children = Vec::new();

    let mut conn = pool.get()?;
    for i in 0..count {
        let path = format!("m/44'/309'/0'/0/{i}");
        let (child_key, _chain_code) = hd_wallet::derive_path(&seed, &path)
            .map_err(|e| AppError::Internal(format!("Derive path {path}: {e}")))?;

        let secp = secp256k1::Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &child_key);
        let lock_hash = derive_lock_hash_hd(&pk);
        let address = ckb_address_from_pubkey(&pk, true);

        // Encrypt the child private key
        let encrypted_key = crypto::encrypt(&child_key.secret_bytes(), password)?;

        let derivation_path = Some(path.as_str());
        let derivation_index = Some(i as i32);

        let wallet_id = wallets::insert_wallet(
            &mut conn,
            &format!("{label} #{i}"),
            &encrypted_key,
            &lock_hash,
            &address,
            None, // parent_wallet_id — all children are top-level for now
            derivation_path,
            derivation_index,
            "hd_child",
        )?;

        // Re-read to get the full record with created_at (uses returned ID)
        let record = wallets::get_wallet_by_id(&mut conn, wallet_id)?;

        children.push(record);
    }

    // 4. Update keystore address_count and save
    keystore.address_count = count;
    keystore::save_keystore(&keystore, keystore_path)?;

    info!(
        label = %label,
        address_count = count,
        keystore = %keystore_path.display(),
        "HD wallet created"
    );

    Ok((keystore, phrase, children))
}

/// Unlock an existing keystore: decrypt the mnemonic, ensure all previously-derived
/// children are in the DB, and return them.
pub fn unlock_keystore(
    pool: &DbPool,
    keystore_path: &Path,
    password: &str,
) -> Result<(Keystore, Vec<WalletRecord>), AppError> {
    let keystore = keystore::load_keystore(keystore_path)?;
    let mnemonic = keystore::decrypt_mnemonic(&keystore, password)?;
    let seed = hd_wallet::mnemonic_to_seed(&mnemonic, "");

    let mut conn = pool.get()?;
    let mut children = Vec::new();

    // Ensure all previously-derived children exist in DB
    for i in 0..keystore.address_count {
        let path = format!("m/44'/309'/0'/0/{i}");
        let (child_key, _) = hd_wallet::derive_path(&seed, &path)
            .map_err(|e| AppError::Internal(format!("Derive path {path}: {e}")))?;

        let secp = secp256k1::Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &child_key);
        let lock_hash = derive_lock_hash_hd(&pk);
        let address = ckb_address_from_pubkey(&pk, true);

        // Match by derivation path so we can refresh stale address/lock_hash values.
        let existing = wallets::get_wallet_by_derivation_path(&mut conn, &path)?;

        match existing {
            Some(record) => {
                if record.lock_hash != lock_hash || record.ckb_address != address {
                    wallets::update_wallet_derived_info(
                        &mut conn, record.id, &lock_hash, &address,
                    )?;
                    let updated = wallets::get_wallet_by_id(&mut conn, record.id)?;
                    children.push(updated);
                } else {
                    children.push(record);
                }
            }
            None => {
                let encrypted_key = crypto::encrypt(&child_key.secret_bytes(), password)?;
                wallets::insert_wallet(
                    &mut conn,
                    &format!("{} #{i}", keystore.label),
                    &encrypted_key,
                    &lock_hash,
                    &address,
                    None,
                    Some(&path),
                    Some(i as i32),
                    "hd_child",
                )?;
                let record = wallets::get_wallet_by_lock_hash(&mut conn, &lock_hash)?;
                children.push(record);
            }
        }
    }

    info!(
        label = %keystore.label,
        count = children.len(),
        "Keystore unlocked"
    );

    Ok((keystore, children))
}

/// Derive additional child addresses for an HD wallet.
pub fn derive_more_addresses(
    pool: &DbPool,
    keystore_path: &Path,
    password: &str,
    additional_count: u32,
) -> Result<Vec<WalletRecord>, AppError> {
    let keystore = keystore::load_keystore(keystore_path)?;
    let mnemonic = keystore::decrypt_mnemonic(&keystore, password)?;
    let seed = hd_wallet::mnemonic_to_seed(&mnemonic, "");

    let mut conn = pool.get()?;
    let mut new_children = Vec::new();
    let start_index = keystore.address_count;

    for i in start_index..start_index + additional_count {
        let path = format!("m/44'/309'/0'/0/{i}");
        let (child_key, _) = hd_wallet::derive_path(&seed, &path)
            .map_err(|e| AppError::Internal(format!("Derive path {path}: {e}")))?;

        let secp = secp256k1::Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &child_key);
        let lock_hash = derive_lock_hash_hd(&pk);
        let address = ckb_address_from_pubkey(&pk, true);
        let encrypted_key = crypto::encrypt(&child_key.secret_bytes(), password)?;

        wallets::insert_wallet(
            &mut conn,
            &format!("{} #{i}", keystore.label),
            &encrypted_key,
            &lock_hash,
            &address,
            None,
            Some(&path),
            Some(i as i32),
            "hd_child",
        )?;

        let record = wallets::get_wallet_by_lock_hash(&mut conn, &lock_hash)?;
        new_children.push(record);
    }

    // Update keystore and save
    keystore::update_address_count(keystore_path, start_index + additional_count)?;

    info!(
        new_count = new_children.len(),
        total = start_index + additional_count,
        "Derived additional addresses"
    );

    Ok(new_children)
}

/// Get total CKB balance for all HD child wallets.
pub async fn get_hd_wallet_balance(
    pool: &DbPool,
    provider: &dyn crate::services::chain_provider::ChainProvider,
) -> Result<u64, AppError> {
    let mut conn = pool.get()?;
    let children = wallets::list_wallets_by_type(&mut conn, "hd_child")?;
    let mut total = 0u64;
    for child in &children {
        total += provider
            .get_balance_by_address(&child.ckb_address)
            .await
            .unwrap_or(0);
    }
    Ok(total)
}

/// Get per-address balances for all HD child wallets.
pub async fn get_hd_wallet_address_balances(
    pool: &DbPool,
    provider: &dyn crate::services::chain_provider::ChainProvider,
) -> Result<Vec<(WalletRecord, u64)>, AppError> {
    let mut conn = pool.get()?;
    let children = wallets::list_wallets_by_type(&mut conn, "hd_child")?;
    let mut results = Vec::new();
    for child in children {
        let balance = provider
            .get_balance_by_address(&child.ckb_address)
            .await
            .unwrap_or(0);
        results.push((child, balance));
    }
    Ok(results)
}

/// Sync HD wallet addresses from keystore and fetch fresh on-chain balances.
pub async fn refresh_hd_wallet(
    pool: &DbPool,
    keystore_path: &Path,
    password: &str,
    provider: &dyn crate::services::chain_provider::ChainProvider,
) -> Result<(Keystore, Vec<WalletRecord>, u64, Vec<(WalletRecord, u64)>), AppError> {
    let (keystore, children) = unlock_keystore(pool, keystore_path, password)?;
    let mut address_balances = Vec::with_capacity(children.len());
    let mut total = 0u64;
    for child in &children {
        let balance = provider
            .get_balance_by_address(&child.ckb_address)
            .await
            .unwrap_or(0);
        total += balance;
        address_balances.push((child.clone(), balance));
    }
    Ok((keystore, children, total, address_balances))
}

/// Check if a keystore file exists at the configured path.
pub fn hd_wallet_exists(keystore_path: &Path) -> bool {
    keystore::keystore_exists(keystore_path)
}

/// Import/recover an HD wallet from a mnemonic phrase.
/// Validates the mnemonic, creates the keystore, derives child keys.
pub fn import_hd_from_mnemonic(
    pool: &DbPool,
    keystore_path: &Path,
    mnemonic_phrase: &str,
    label: &str,
    password: &str,
    address_count: u32,
) -> Result<(Keystore, Vec<WalletRecord>), AppError> {
    let mnemonic = Mnemonic::parse(mnemonic_phrase)
        .map_err(|e| AppError::BadRequest(format!("Invalid mnemonic: {e}")))?;
    let count = if address_count == 0 { 5 } else { address_count };
    let seed = hd_wallet::mnemonic_to_seed(&mnemonic, "");

    let mut keystore = keystore::create_keystore(&mnemonic, password, label, "m/44'/309'/0'/0")?;

    let mut conn = pool.get()?;
    let mut children = Vec::new();

    for i in 0..count {
        let path = format!("m/44'/309'/0'/0/{i}");
        let (child_key, _) = hd_wallet::derive_path(&seed, &path)
            .map_err(|e| AppError::Internal(format!("Derive path {path}: {e}")))?;
        let secp = secp256k1::Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &child_key);
        let lock_hash = derive_lock_hash_hd(&pk);
        let address = ckb_address_from_pubkey(&pk, true);
        let encrypted_key = crypto::encrypt(&child_key.secret_bytes(), password)?;

        wallets::insert_wallet(
            &mut conn,
            &format!("{label} #{i}"),
            &encrypted_key,
            &lock_hash,
            &address,
            None,
            Some(&path),
            Some(i as i32),
            "hd_child",
        )?;

        let record = wallets::get_wallet_by_lock_hash(&mut conn, &lock_hash)?;
        children.push(record);
    }

    keystore.address_count = count;
    keystore::save_keystore(&keystore, keystore_path)?;

    info!(label = %label, count = count, "HD wallet imported from mnemonic");
    Ok((keystore, children))
}

/// Delete the HD wallet: remove keystore file + all hd_child wallets from DB.
pub fn delete_hd_wallet(pool: &DbPool, keystore_path: &Path) -> Result<(), AppError> {
    // Delete keystore file
    if keystore_path.exists() {
        std::fs::remove_file(keystore_path)
            .map_err(|e| AppError::Internal(format!("Failed to delete keystore: {e}")))?;
    }

    // Delete all hd_child wallets
    let mut conn = pool.get()?;
    let children = wallets::list_wallets_by_type(&mut conn, "hd_child")?;
    for child in children {
        wallets::delete_wallet(&mut conn, child.id)?;
    }

    info!(keystore = %keystore_path.display(), "HD wallet deleted");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
