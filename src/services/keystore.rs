//! Keystore file management — encrypted JSON on-disk storage for HD wallet mnemonics.
//!
//! The keystore is a JSON file (default: `data/keystore.json`) that stores
//! an AES-256-GCM encrypted BIP39 mnemonic phrase along with wallet metadata.
//! The encryption key is derived from a user-provided password via SHA-256
//! (same approach as the existing `crypto` module).
//!
//! # Format
//! ```json
//! {
//!   "version": 1,
//!   "mnemonic_encrypted": "<hex: nonce[12] || ciphertext>",
//!   "label": "My HD Wallet",
//!   "derivation_path": "m/44'/309'/0'/0",
//!   "address_count": 5,
//!   "created_at": "2026-06-29T12:34:56Z"
//! }
//! ```

use bip39::Mnemonic;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::AppError;
use crate::services::crypto;

/// On-disk keystore representation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keystore {
    pub version: u8,
    /// AES-256-GCM encrypted mnemonic phrase (hex: nonce[12] || ciphertext).
    pub mnemonic_encrypted: String,
    pub label: String,
    pub derivation_path: String,
    pub address_count: u32,
    pub created_at: String,
}

/// Create a new keystore by encrypting a mnemonic with the given password.
pub fn create_keystore(
    mnemonic: &Mnemonic,
    password: &str,
    label: &str,
    derivation_path: &str,
) -> Result<Keystore, AppError> {
    let phrase = mnemonic.to_string();
    let encrypted = crypto::encrypt(phrase.as_bytes(), password)?;
    let mnemonic_encrypted = hex::encode(&encrypted);

    let created_at = chrono_now();

    Ok(Keystore {
        version: 1,
        mnemonic_encrypted,
        label: label.to_string(),
        derivation_path: derivation_path.to_string(),
        address_count: 0,
        created_at,
    })
}

/// Save a keystore to a file on disk.
pub fn save_keystore(keystore: &Keystore, path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("Failed to create keystore dir: {e}")))?;
    }
    let json = serde_json::to_string_pretty(keystore)
        .map_err(|e| AppError::Internal(format!("JSON: {e}")))?;
    std::fs::write(path, json)
        .map_err(|e| AppError::Internal(format!("Failed to write keystore: {e}")))?;
    Ok(())
}

/// Load a keystore from a file on disk.
pub fn load_keystore(path: &Path) -> Result<Keystore, AppError> {
    let json =
        std::fs::read_to_string(path).map_err(|e| AppError::NotFound(format!("Keystore: {e}")))?;
    let keystore: Keystore = serde_json::from_str(&json)
        .map_err(|e| AppError::Internal(format!("Keystore parse: {e}")))?;
    Ok(keystore)
}

/// Decrypt the mnemonic from a keystore using the given password.
pub fn decrypt_mnemonic(keystore: &Keystore, password: &str) -> Result<Mnemonic, AppError> {
    let encrypted = hex::decode(&keystore.mnemonic_encrypted)
        .map_err(|e| AppError::BadRequest(format!("Hex: {e}")))?;
    let phrase_bytes = crypto::decrypt(&encrypted, password)?;
    let phrase = String::from_utf8(phrase_bytes)
        .map_err(|e| AppError::Internal(format!("Mnemonic not UTF-8: {e}")))?;
    Mnemonic::parse(&phrase).map_err(|e| AppError::BadRequest(format!("Invalid mnemonic: {e}")))
}

/// Check if a keystore file exists at the given path.
pub fn keystore_exists(path: &Path) -> bool {
    path.exists()
}

/// Update the address count in a keystore and save it.
pub fn update_address_count(path: &Path, new_count: u32) -> Result<(), AppError> {
    let mut keystore = load_keystore(path)?;
    keystore.address_count = new_count;
    save_keystore(&keystore, path)
}

/// Get a simple ISO 8601 timestamp for the current time.
fn chrono_now() -> String {
    // Simple manual formatting to avoid a chrono dependency
    // Format: YYYY-MM-DDTHH:MM:SSZ
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;

    // Approximate gregorian date from unix timestamp
    let (year, month, day) = civil_from_days(days_since_epoch as i64);
    let hours = (time_of_day / 3600) % 24;
    let minutes = (time_of_day / 60) % 60;
    let seconds = time_of_day % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// Simple civil date from days since epoch (approximate, good enough for timestamps).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_save_load_roundtrip() {
        let m = crate::services::hd_wallet::generate_mnemonic().unwrap();
        let ks = create_keystore(&m, "test-password", "test-wallet", "m/44'/309'/0'/0").unwrap();
        assert_eq!(ks.version, 1);
        assert_eq!(ks.label, "test-wallet");
        assert_eq!(ks.address_count, 0);

        // Save to temp file
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keystore.json");
        save_keystore(&ks, &path).unwrap();
        assert!(path.exists());

        // Load and verify
        let loaded = load_keystore(&path).unwrap();
        assert_eq!(loaded.label, "test-wallet");

        // Decrypt
        let decrypted = decrypt_mnemonic(&loaded, "test-password").unwrap();
        assert_eq!(decrypted.to_string(), m.to_string());
    }

    #[test]
    fn test_wrong_password_fails() {
        let m = crate::services::hd_wallet::generate_mnemonic().unwrap();
        let ks = create_keystore(&m, "correct", "w", "m/0").unwrap();
        let result = decrypt_mnemonic(&ks, "wrong");
        assert!(result.is_err());
    }

    #[test]
    fn test_keystore_not_exists() {
        assert!(!keystore_exists(Path::new(
            "/tmp/__nonexistent_keystore__.json"
        )));
    }
}
