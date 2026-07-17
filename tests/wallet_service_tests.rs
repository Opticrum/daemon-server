//! Wallet service tests — key derivation, encryption round-trip, import flow,
//! and HD wallet integration.

mod common;

use common::{test_db, test_private_key_hex};
use rust_server::services::hd_wallet_signer::HdWalletSigner;
use rust_server::services::wallet_service;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static KEYSTORE_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_keystore_path() -> PathBuf {
    let n = KEYSTORE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join("opticrum_test");
    std::fs::create_dir_all(&dir).ok();
    dir.join(format!("keystore_test_{}.json", n))
}

#[test]
fn derive_lock_hash_from_private_key_hex() {
    // Import a test key
    let pool = test_db();
    let key_hex = test_private_key_hex();

    let wallet = wallet_service::import_wallet(&pool, "test-key", &key_hex, Some("test-password"))
        .expect("import should succeed");

    assert_eq!(wallet.label, "test-key");
    // lock_hash is 32 bytes = 64 hex chars
    assert_eq!(wallet.lock_hash.len(), 32);
    // CKB2021 bech32m testnet address starts with "ckt1q"
    assert!(
        wallet.ckb_address.starts_with("ckt1q"),
        "address must be valid CKB2021 bech32m: {}",
        wallet.ckb_address
    );
    // Must decode as valid bech32m
    bech32::decode(&wallet.ckb_address).expect("address must be valid bech32m");
}

#[test]
fn encrypt_decrypt_private_key_roundtrip() {
    let pool = test_db();
    let key_hex = test_private_key_hex();

    let wallet =
        wallet_service::import_wallet(&pool, "roundtrip", &key_hex, Some("my-encryption-password"))
            .unwrap();

    // Decrypt
    let secret_key = wallet_service::decrypt_private_key(&wallet, Some("my-encryption-password"))
        .expect("decrypt should succeed");
    assert_eq!(secret_key.secret_bytes().len(), 32);
}

#[test]
fn decrypt_wrong_password_fails() {
    let pool = test_db();
    let key_hex = test_private_key_hex();

    let wallet = wallet_service::import_wallet(&pool, "wp", &key_hex, Some("correct")).unwrap();

    let result = wallet_service::decrypt_private_key(&wallet, Some("wrong"));
    assert!(result.is_err());
}

#[test]
fn import_invalid_hex_fails() {
    let pool = test_db();
    let result =
        wallet_service::import_wallet(&pool, "bad", "not-a-hex-string!!!", Some("password"));
    assert!(result.is_err());
}

#[test]
fn import_wrong_length_key_fails() {
    let pool = test_db();
    // Too short (16 bytes = 32 hex chars)
    let result =
        wallet_service::import_wallet(&pool, "short", "abcdef0123456789", Some("password"));
    assert!(result.is_err());
}

#[test]
fn list_wallets_returns_all() {
    let pool = test_db();
    let key_hex = test_private_key_hex();

    wallet_service::import_wallet(&pool, "w1", &key_hex, Some("pw")).unwrap();
    wallet_service::import_wallet(&pool, "w2", &key_hex, Some("pw")).unwrap_err(); // duplicate lock_hash
                                                                                   // Different key
    let mut bytes = hex::decode(&key_hex).unwrap();
    bytes[0] = bytes[0].wrapping_add(1);
    let key2_hex = hex::encode(&bytes);
    wallet_service::import_wallet(&pool, "w2", &key2_hex, Some("pw")).unwrap();

    let wallets = wallet_service::list_wallets(&pool).unwrap();
    assert_eq!(wallets.len(), 2);
}

#[test]
fn delete_wallet_removes_from_db() {
    let pool = test_db();
    let key_hex = test_private_key_hex();

    let wallet = wallet_service::import_wallet(&pool, "tmp", &key_hex, Some("pw")).unwrap();
    let deleted = wallet_service::delete_wallet(&pool, wallet.id).unwrap();
    assert!(deleted);

    let result = wallet_service::get_wallet(&pool, wallet.id);
    assert!(result.is_err());
}

#[test]
fn get_wallet_not_found() {
    let pool = test_db();
    let result = wallet_service::get_wallet(&pool, 9999);
    assert!(result.is_err());
}

#[test]
fn different_keys_produce_different_lock_hashes() {
    let pool = test_db();
    let key1_hex = test_private_key_hex();

    let mut bytes = hex::decode(&key1_hex).unwrap();
    bytes[0] = bytes[0].wrapping_add(1);
    let key2_hex = hex::encode(&bytes);

    let w1 = wallet_service::import_wallet(&pool, "one", &key1_hex, Some("pw")).unwrap();
    let w2 = wallet_service::import_wallet(&pool, "two", &key2_hex, Some("pw")).unwrap();

    assert_ne!(w1.lock_hash, w2.lock_hash);
}

// ═══════════════════════════════════════════════════════════════════════════
// HD Wallet Integration Tests
// ═══════════════════════════════════════════════════════════════════════════

fn test_mnemonic_phrase() -> String {
    rust_server::services::hd_wallet::generate_mnemonic()
        .unwrap()
        .to_string()
}

/// Import an HD wallet from a mnemonic and verify every child wallet's
/// stored address can be re-derived from its decrypted private key.
#[test]
fn test_hd_import_re_derive_consistency() {
    let pool = test_db();
    let keystore_path = temp_keystore_path();
    let phrase = test_mnemonic_phrase();
    let password = "test-password";
    let count = 3;

    let (_keystore, children) = wallet_service::import_hd_from_mnemonic(
        &pool,
        &keystore_path,
        &phrase,
        "test-hd",
        password,
        count,
    )
    .unwrap();

    assert_eq!(children.len(), count as usize);

    for child in &children {
        // Decrypt the stored private key
        let sk = wallet_service::decrypt_private_key(child, Some(password))
            .unwrap_or_else(|e| panic!("must decrypt key for {}: {e}", child.label));

        // Re-derive the pubkey, lock_args, lock_hash, and address
        let secp = secp256k1::Secp256k1::new();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let lock_arg = rust_server::services::address::lock_arg_from_pubkey(&pk);
        let lock_hash = rust_server::services::address::script_lock_hash(&lock_arg);
        let address = rust_server::services::address::ckb_address_from_pubkey(&pk, true);

        // The stored values must match the re-derived values
        assert_eq!(
            hex::encode(&child.lock_hash),
            hex::encode(lock_hash),
            "lock_hash mismatch for {} — stored address does not match private key",
            child.label
        );
        assert_eq!(
            child.ckb_address, address,
            "address mismatch for {} — stored address does not match private key",
            child.label
        );
    }

    // Cleanup
    let _ = std::fs::remove_file(&keystore_path);
}

/// Verify that `unlock_keystore` produces the same derived keys as
/// the original `import_hd_from_mnemonic`.
#[test]
fn test_hd_unlock_re_produces_same_addresses() {
    let pool = test_db();
    let keystore_path = temp_keystore_path();
    let phrase = test_mnemonic_phrase();
    let password = "test-password-unlock";
    let count = 4;

    // Create the HD wallet
    let (_keystore1, created) = wallet_service::import_hd_from_mnemonic(
        &pool,
        &keystore_path,
        &phrase,
        "hd-unlock-test",
        password,
        count,
    )
    .unwrap();

    // Now unlock it via unlock_keystore
    let (_keystore2, unlocked) =
        wallet_service::unlock_keystore(&pool, &keystore_path, password).unwrap();

    assert_eq!(created.len(), unlocked.len());
    for (original, re_derived) in created.iter().zip(unlocked.iter()) {
        assert_eq!(
            original.ckb_address, re_derived.ckb_address,
            "unlock address {} differs from import address {}",
            re_derived.ckb_address, original.ckb_address
        );
        assert_eq!(
            original.lock_hash, re_derived.lock_hash,
            "unlock lock_hash for {} differs from import lock_hash",
            original.label
        );
        assert_eq!(original.derivation_path, re_derived.derivation_path);
        assert_eq!(original.derivation_index, re_derived.derivation_index);
    }

    let _ = std::fs::remove_file(&keystore_path);
}

/// Load keys into HdWalletSigner and verify each address is correctly found.
#[test]
fn test_signer_find_key_by_address() {
    let pool = test_db();
    let keystore_path = temp_keystore_path();
    let phrase = test_mnemonic_phrase();
    let password = "signer-test-pw";
    let count = 3;

    let (_keystore, children) = wallet_service::import_hd_from_mnemonic(
        &pool,
        &keystore_path,
        &phrase,
        "signer-test",
        password,
        count,
    )
    .unwrap();

    // Load keys into the signer
    let signer = HdWalletSigner::new();
    signer.load_keys(&pool, password).unwrap();
    assert!(signer.is_unlocked());

    // Each child address must map to the correct private key
    for child in &children {
        let found = signer.find_key_by_address(&child.ckb_address);
        assert!(
            found.is_some(),
            "signer must find key for address: {}",
            child.ckb_address
        );

        let found_sk = found.unwrap();
        let secp = secp256k1::Secp256k1::new();
        let found_pk = secp256k1::PublicKey::from_secret_key(&secp, &found_sk);
        let found_lock_arg = rust_server::services::address::lock_arg_from_pubkey(&found_pk);

        // Verify the found key produces the correct lock_arg
        let expected_lock_arg = hex::decode(
            rust_server::services::address::lock_arg_from_address(&child.ckb_address)
                .map(hex::encode)
                .unwrap_or_default(),
        )
        .unwrap_or_default();
        if !expected_lock_arg.is_empty() {
            assert_eq!(
                found_lock_arg.as_slice(),
                expected_lock_arg.as_slice(),
                "signer returned wrong key for address {}",
                child.ckb_address
            );
        }
    }

    // Non-existent address must return None
    let not_found =
        signer.find_key_by_address("ckt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq");
    assert!(not_found.is_none(), "bogus address must not match");

    // Clear and verify signer is locked
    signer.clear();
    assert!(!signer.is_unlocked());

    let _ = std::fs::remove_file(&keystore_path);
}

/// The lock_hash stored in the DB must be re-derivable from the lock_arg
/// extracted from the stored address.
#[test]
fn test_lock_hash_consistent_with_address() {
    let pool = test_db();
    let keystore_path = temp_keystore_path();
    let phrase = test_mnemonic_phrase();
    let password = "consistency-pw";
    let count = 5;

    let (_keystore, children) = wallet_service::import_hd_from_mnemonic(
        &pool,
        &keystore_path,
        &phrase,
        "consistency",
        password,
        count,
    )
    .unwrap();

    for child in &children {
        // Extract lock_arg from the stored address
        let lock_arg = rust_server::services::address::lock_arg_from_address(&child.ckb_address)
            .unwrap_or_else(|e| panic!("must decode stored address '{}': {e}", child.ckb_address));

        // Re-compute lock_hash
        let computed_lock_hash = rust_server::services::address::script_lock_hash(&lock_arg);
        let stored_lock_hash: [u8; 32] = child
            .lock_hash
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| panic!("lock_hash wrong length for {}", child.label));

        assert_eq!(
            computed_lock_hash, stored_lock_hash,
            "lock_hash in DB does not match address for {}",
            child.label
        );
    }

    let _ = std::fs::remove_file(&keystore_path);
}

/// Verify that deriving additional addresses produces wallets consistent
/// with the original derivation.
#[test]
fn test_derive_more_addresses_consistency() {
    let pool = test_db();
    let keystore_path = temp_keystore_path();
    let phrase = test_mnemonic_phrase();
    let password = "derive-more-pw";

    // Create with 3 addresses
    let (_keystore, _first_batch) = wallet_service::import_hd_from_mnemonic(
        &pool,
        &keystore_path,
        &phrase,
        "derive-more",
        password,
        3,
    )
    .unwrap();

    // Derive 2 more
    let more = wallet_service::derive_more_addresses(&pool, &keystore_path, password, 2).unwrap();

    assert_eq!(more.len(), 2);
    assert_eq!(more[0].derivation_index, Some(3));
    assert_eq!(more[1].derivation_index, Some(4));

    // Verify the new addresses are consistent
    for child in &more {
        let sk = wallet_service::decrypt_private_key(child, Some(password))
            .unwrap_or_else(|_e| panic!("must decrypt {}", child.ckb_address));
        let secp = secp256k1::Secp256k1::new();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let address = rust_server::services::address::ckb_address_from_pubkey(&pk, true);
        assert_eq!(child.ckb_address, address);
    }

    let _ = std::fs::remove_file(&keystore_path);
}
