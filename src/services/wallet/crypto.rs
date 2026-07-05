//! Cryptographic utilities — AES-256-GCM encryption for private key storage.
//!
//! Private keys are encrypted at rest using AES-256-GCM. The encryption key
//! is derived from the server's encryption password via SHA-256.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use secp256k1::SecretKey;
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Derive a 32-byte AES key from a password string using SHA-256.
fn derive_key(password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypt plaintext bytes with AES-256-GCM using the given password.
///
/// Returns the ciphertext with a 12-byte nonce prepended: `nonce[12] || ciphertext`.
pub fn encrypt(plaintext: &[u8], password: &str) -> Result<Vec<u8>, AppError> {
    let key = derive_key(password);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::Internal(format!("AES init: {e}")))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AppError::Internal(format!("Encryption failed: {e}")))?;

    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypt ciphertext (format: `nonce[12] || ciphertext`) with the given password.
pub fn decrypt(ciphertext_with_nonce: &[u8], password: &str) -> Result<Vec<u8>, AppError> {
    if ciphertext_with_nonce.len() < 12 {
        return Err(AppError::WalletError("Ciphertext too short".into()));
    }

    let (nonce_bytes, ciphertext) = ciphertext_with_nonce.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let key = derive_key(password);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| AppError::Internal(format!("AES init: {e}")))?;

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::WalletError("Decryption failed — wrong password?".into()))
}

/// Decrypt a stored private key blob and parse it as a secp256k1 SecretKey.
///
/// Convenience wrapper around `decrypt()` for wallet key material.
pub fn decrypt_secret_key(encrypted_key: &[u8], password: &str) -> Result<SecretKey, AppError> {
    let key_bytes = decrypt(encrypted_key, password)?;
    SecretKey::from_slice(&key_bytes)
        .map_err(|e| AppError::WalletError(format!("Invalid private key: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let plaintext = b"this is a private key secret";
        let password = "test-password";

        let encrypted = encrypt(plaintext, password).expect("encrypt should succeed");
        assert!(encrypted.len() > 12, "ciphertext includes nonce");

        let decrypted = decrypt(&encrypted, password).expect("decrypt should succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_wrong_password_fails() {
        let plaintext = b"secret data";
        let encrypted = encrypt(plaintext, "correct-password").unwrap();
        let result = decrypt(&encrypted, "wrong-password");
        assert!(result.is_err(), "decrypt with wrong password should fail");
    }

    #[test]
    fn ciphertext_too_short() {
        let result = decrypt(b"short", "password");
        assert!(result.is_err(), "too-short ciphertext should fail");
    }

    #[test]
    fn different_nonces_produce_different_ciphertexts() {
        let plaintext = b"same data";
        let password = "pw";
        let enc1 = encrypt(plaintext, password).unwrap();
        let enc2 = encrypt(plaintext, password).unwrap();
        // Nonces differ, so ciphertexts differ
        assert_ne!(enc1, enc2);
    }
}
