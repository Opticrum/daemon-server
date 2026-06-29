//! BIP39 mnemonic generation and BIP32 hierarchical deterministic key derivation.
//!
//! Implements:
//! - BIP39: 12-word mnemonic generation via the `bip39` crate
//! - BIP32: master key + hardened child key derivation (CKDpriv)
//! - BIP44: path parsing for CKB coin type 309

use bip39::Mnemonic;
use hmac::{Hmac, Mac};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::Sha512;

use crate::error::AppError;

type HmacSha512 = Hmac<Sha512>;

/// The secp256k1 curve order (n).
const CURVE_ORDER: [u8; 32] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFE, 0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36,
    0x41, 0x41,
];

/// Generate a new 12-word BIP39 mnemonic (128 bits of entropy).
pub fn generate_mnemonic() -> Result<Mnemonic, AppError> {
    Mnemonic::generate(12).map_err(|e| AppError::Internal(format!("Mnemonic generation: {e}")))
}

/// Convert a BIP39 mnemonic to a 64-byte seed using the optional passphrase.
/// Passphrase defaults to empty string ("").
pub fn mnemonic_to_seed(mnemonic: &Mnemonic, passphrase: &str) -> [u8; 64] {
    mnemonic.to_seed(passphrase)
}

/// Derive the BIP32 master key from a seed byte slice of any length.
///
/// Returns `(master_private_key, chain_code)`.
/// The master key is derived via HMAC-SHA512(key="Bitcoin seed", data=seed).
/// - master_private_key = left 32 bytes of HMAC output
/// - chain_code = right 32 bytes
pub fn derive_master_key(seed: &[u8]) -> Result<(SecretKey, [u8; 32]), AppError> {
    let mut mac =
        HmacSha512::new_from_slice(b"Bitcoin seed").expect("HMAC-SHA512 accepts any key length");

    mac.update(seed);
    let i = mac.finalize().into_bytes();

    let mut il = [0u8; 32];
    let mut chain_code = [0u8; 32];
    il.copy_from_slice(&i[0..32]);
    chain_code.copy_from_slice(&i[32..64]);

    // Validate that IL is within the secp256k1 order
    let master_key = SecretKey::from_slice(&il).map_err(|e| {
        AppError::Internal(format!(
            "Master key derivation failed (IL >= n, extremely unlikely): {e}"
        ))
    })?;

    Ok((master_key, chain_code))
}

/// Add two 32-byte big-endian integers modulo the secp256k1 curve order n.
///
/// Returns `(a + b) mod n` as a 32-byte big-endian array.
fn add_mod_n(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    // Big-endian addition with carry
    let mut carry = 0u16;
    let mut result = [0u8; 32];
    for i in (0..32).rev() {
        let sum = a[i] as u16 + b[i] as u16 + carry;
        result[i] = (sum & 0xFF) as u8;
        carry = sum >> 8;
    }

    // If the result >= n, subtract n
    if carry > 0 || is_ge(&result, &CURVE_ORDER) {
        let mut borrow = 0u16;
        for i in (0..32).rev() {
            let n_byte = CURVE_ORDER[i] as u16;
            let sub = result[i] as i32 - n_byte as i32 - borrow as i32;
            if sub < 0 {
                result[i] = (sub + 256) as u8;
                borrow = 1;
            } else {
                result[i] = sub as u8;
                borrow = 0;
            }
        }
    }

    result
}

/// Compare two 32-byte arrays (big-endian unsigned: a >= b).
fn is_ge(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in 0..32 {
        match a[i].cmp(&b[i]) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    true // equal
}

/// Derive a hardened child key (BIP32 CKDpriv).
///
/// Hardened derivation: `child_index = index | 0x80000000`
/// - I = HMAC-SHA512(key=chain_code, data=0x00 || parent_key_bytes || child_index_be)
/// - child_key = (IL + parent_key) mod n
/// - child_chain_code = IR
///
/// Returns `(child_secret_key, child_chain_code)`.
pub fn derive_child_key(
    parent_key: &SecretKey,
    chain_code: &[u8; 32],
    index: u32,
) -> Result<(SecretKey, [u8; 32]), AppError> {
    let hardened_index = index | 0x80000000;
    let index_be = hardened_index.to_be_bytes();

    let mut mac = HmacSha512::new_from_slice(chain_code)
        .map_err(|e| AppError::Internal(format!("HMAC key: {e}")))?;

    mac.update(&[0x00]); // prepend 0x00 per BIP32
    mac.update(&parent_key.secret_bytes());
    mac.update(&index_be);

    let i = mac.finalize().into_bytes();

    let mut il = [0u8; 32];
    let mut child_chain_code = [0u8; 32];
    il.copy_from_slice(&i[0..32]);
    child_chain_code.copy_from_slice(&i[32..64]);

    // child_key = (IL + parent_key) mod n
    let parent_bytes = parent_key.secret_bytes();
    let child_bytes = add_mod_n(&il, &parent_bytes);

    let child_key = SecretKey::from_slice(&child_bytes).map_err(|e| {
        AppError::Internal(format!(
            "Child key derivation failed (result >= n, extremely unlikely): {e}"
        ))
    })?;

    Ok((child_key, child_chain_code))
}

/// Derive a normal (non-hardened) child key (BIP32 CKDpriv).
///
/// Normal derivation: `child_index = index` (no 0x80000000 flag)
/// - I = HMAC-SHA512(key=chain_code, data=serP(Kpar) || child_index_be)
/// - child_key = (IL + parent_key) mod n
/// - child_chain_code = IR
pub fn derive_child_key_normal(
    parent_key: &SecretKey,
    chain_code: &[u8; 32],
    index: u32,
) -> Result<(SecretKey, [u8; 32]), AppError> {
    let secp = Secp256k1::new();
    let parent_pubkey = PublicKey::from_secret_key(&secp, parent_key);
    let index_be = index.to_be_bytes();

    let mut mac = HmacSha512::new_from_slice(chain_code)
        .map_err(|e| AppError::Internal(format!("HMAC key: {e}")))?;

    mac.update(&parent_pubkey.serialize());
    mac.update(&index_be);

    let i = mac.finalize().into_bytes();

    let mut il = [0u8; 32];
    let mut child_chain_code = [0u8; 32];
    il.copy_from_slice(&i[0..32]);
    child_chain_code.copy_from_slice(&i[32..64]);

    let parent_bytes = parent_key.secret_bytes();
    let child_bytes = add_mod_n(&il, &parent_bytes);

    let child_key = SecretKey::from_slice(&child_bytes).map_err(|e| {
        AppError::Internal(format!(
            "Normal child key derivation failed (result >= n, extremely unlikely): {e}"
        ))
    })?;

    Ok((child_key, child_chain_code))
}

/// Derive a secret key at a given BIP32 derivation path.
///
/// Path format: `"m/44'/309'/0'/0/0"` — each segment after `m` is a hardened
/// index (the `'` suffix denotes hardened derivation; all our indices are hardened).
pub fn derive_path(seed: &[u8], path: &str) -> Result<(SecretKey, [u8; 32]), AppError> {
    let (mut key, mut chain_code) = derive_master_key(seed)?;

    for segment in path.split('/').skip(1) {
        let hardened = segment.ends_with('\'');
        let index_str = segment.trim_end_matches('\'');
        let index: u32 = index_str
            .parse()
            .map_err(|_| AppError::BadRequest(format!("Invalid derivation path segment: {segment}")))?;

        let (child_key, child_cc) = if hardened {
            derive_child_key(&key, &chain_code, index)?
        } else {
            derive_child_key_normal(&key, &chain_code, index)?
        };
        key = child_key;
        chain_code = child_cc;
    }

    Ok((key, chain_code))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BIP32 Test Vector 1 from the spec.
    /// Seed (128 bits): 000102030405060708090a0b0c0d0e0f
    #[test]
    fn test_bip32_vector_1_master() {
        let seed: [u8; 16] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f];

        let (master_key, chain_code) = derive_master_key(&seed).unwrap();
        assert_eq!(
            hex::encode(master_key.secret_bytes()),
            "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35"
        );
        assert_eq!(
            hex::encode(chain_code),
            "873dff81c02f525623fd1fe5167eac3a55a049de3d314bb42ee227ffed37d508"
        );
    }

    #[test]
    fn test_bip32_vector_1_child_0() {
        let seed: [u8; 16] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f];

        let (master_key, chain_code) = derive_master_key(&seed).unwrap();
        let (child_key, child_cc) = derive_child_key(&master_key, &chain_code, 0).unwrap();

        assert_eq!(
            hex::encode(child_key.secret_bytes()),
            "edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea"
        );
        assert_eq!(
            hex::encode(child_cc),
            "47fdacbd0f1097043b78c63c20c34ef4ed9a111d980047ad16282c7ae6236141"
        );
    }

    #[test]
    fn test_derive_path() {
        let seed: [u8; 16] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f];

        let (key, cc) = derive_path(&seed, "m/44'/309'/0'/0/0").unwrap();
        assert!(!key.secret_bytes().iter().all(|&b| b == 0));
        assert!(!cc.iter().all(|&b| b == 0));

        // Last two path segments must use normal derivation — hardened path differs.
        let (hardened_key, _) = derive_path(&seed, "m/44'/309'/0'/0'/0'").unwrap();
        assert_ne!(key.secret_bytes(), hardened_key.secret_bytes());
    }

    #[test]
    fn test_normal_vs_hardened_child() {
        let seed: [u8; 16] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f];
        let (master, cc) = derive_master_key(&seed).unwrap();
        let (normal, _) = derive_child_key_normal(&master, &cc, 0).unwrap();
        let (hardened, _) = derive_child_key(&master, &cc, 0).unwrap();
        assert_ne!(normal.secret_bytes(), hardened.secret_bytes());
    }

    #[test]
    fn test_generate_mnemonic() {
        let m = generate_mnemonic().unwrap();
        let phrase = m.to_string();
        let words: Vec<&str> = phrase.split_whitespace().collect();
        assert_eq!(words.len(), 12);
    }

    #[test]
    fn test_seed_deterministic() {
        let m = generate_mnemonic().unwrap();
        let seed1 = mnemonic_to_seed(&m, "");
        let seed2 = mnemonic_to_seed(&m, "");
        assert_eq!(seed1, seed2);

        let seed3 = mnemonic_to_seed(&m, "different");
        assert_ne!(seed1, seed3);
    }

    #[test]
    fn test_master_key_derives_from_seed() {
        let m = generate_mnemonic().unwrap();
        let seed = mnemonic_to_seed(&m, "");
        let (key, cc) = derive_master_key(&seed).unwrap();
        assert!(!key.secret_bytes().iter().all(|&b| b == 0));
        assert!(!cc.iter().all(|&b| b == 0));
    }
}
