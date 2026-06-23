//! Wallet service tests — key derivation, encryption round-trip, import flow.

mod common;

use common::{test_db, test_private_key_hex};
use rust_server::services::wallet_service;

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
    assert!(wallet.ckb_address.starts_with("ckt1q..."));
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
    let result = wallet_service::import_wallet(&pool, "bad", "not-a-hex-string!!!", Some("password"));
    assert!(result.is_err());
}

#[test]
fn import_wrong_length_key_fails() {
    let pool = test_db();
    // Too short (16 bytes = 32 hex chars)
    let result = wallet_service::import_wallet(&pool, "short", "abcdef0123456789", Some("password"));
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
