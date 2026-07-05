//! CKB address derivation — aligned with `ckb-sdk` / `ckb-cli`.
//!
//! Flow (see `ckb-cli util key-info` and `AddressPayload::from_pubkey`):
//! 1. `lock_arg = blake2b-256(compressed_pubkey)[0..20]`
//! 2. Address encodes secp256k1_blake160_sighash_all lock (code_hash + hash_type::Type + args)
//! 3. Primary format is CKB2021 full address (bech32m, `is_new = true` in ckb-cli)
//!
//! **Important**: CKB's `blake2b-256` uses the "ckb-default-hash" personalization,
//! which is different from raw blake2b-256. All hash computations MUST use this
//! personalization to match ckb-cli and on-chain values.

use bech32::{ToBase32, Variant};
use blake2b_simd::Params as Blake2bParams;
use secp256k1::PublicKey;

/// CKB default hash personalization string (matching ckb-hash crate).
const CKB_HASH_PERSONALIZATION: &[u8] = b"ckb-default-hash";

/// Create a blake2b-256 hasher with CKB personalization.
fn ckb_blake2b_hasher() -> blake2b_simd::State {
    Blake2bParams::new()
        .hash_length(32)
        .personal(CKB_HASH_PERSONALIZATION)
        .to_state()
}

/// CKB secp256k1_blake160 sighash_all type script hash (mainnet & testnet).
pub const SIGHASH_TYPE_HASH: [u8; 32] = [
    0x9b, 0xd7, 0xe0, 0x6f, 0x3e, 0xcf, 0x4b, 0xe0, 0xf2, 0xfc, 0xd2, 0x18, 0x8b, 0x23, 0xf1, 0xb9,
    0xfc, 0xc8, 0x8e, 0x5d, 0x4b, 0x65, 0xa8, 0x63, 0x7b, 0x17, 0x72, 0x3b, 0xbd, 0xa3, 0xcc, 0xe8,
];

/// Alias kept for callers that reference the old name.
pub const SECP256K1_BLAKE160_CODE_HASH: [u8; 32] = SIGHASH_TYPE_HASH;
pub const SECP256K1_BLAKE160_CODE_HASH_MAINNET: [u8; 32] = SIGHASH_TYPE_HASH;

/// Script hash type: `type` (matches `ScriptHashType::Type` in ckb-types).
pub const HASH_TYPE_TYPE: u8 = 0x01;

/// Short-address code hash index for secp256k1_blake160_sighash_all.
const CODE_HASH_INDEX_SIGHASH: u8 = 0x00;

/// Compute lock_arg (blake160) from a 33-byte compressed public key.
///
/// Uses CKB's blake2b-256 with "ckb-default-hash" personalization,
/// matching `ckb_hash::blake2b_256` in the ckb-hash crate.
pub fn blake160(pubkey: &[u8; 33]) -> [u8; 20] {
    let hash = ckb_blake2b_hasher().update(pubkey).finalize();
    let mut result = [0u8; 20];
    result.copy_from_slice(&hash.as_bytes()[0..20]);
    result
}

/// Derive lock_arg from a secp256k1 public key (same as `AddressPayload::from_pubkey`).
pub fn lock_arg_from_pubkey(pubkey: &PublicKey) -> [u8; 20] {
    blake160(&pubkey.serialize())
}

/// Serialize a secp256k1_blake160_sighash_all lock script in Molecule format and hash it.
///
/// Matches `Script::calc_script_hash()` used by ckb-cli for `lock_hash`.
pub fn script_lock_hash(lock_arg: &[u8; 20]) -> [u8; 32] {
    // Molecule table Script { code_hash: Byte32, hash_type: byte, args: Bytes }
    // total_size=73, offsets=[16, 48, 49], then fields.
    let mut script = Vec::with_capacity(73);
    script.extend_from_slice(&73u32.to_le_bytes());
    script.extend_from_slice(&16u32.to_le_bytes());
    script.extend_from_slice(&48u32.to_le_bytes());
    script.extend_from_slice(&49u32.to_le_bytes());
    script.extend_from_slice(&SIGHASH_TYPE_HASH);
    script.push(HASH_TYPE_TYPE);
    script.extend_from_slice(&20u32.to_le_bytes());
    script.extend_from_slice(lock_arg);

    ckb_blake2b_hasher()
        .update(&script)
        .finalize()
        .as_bytes()
        .try_into()
        .unwrap()
}

/// CKB2021 full address (bech32m) — primary format in ckb-cli (`is_new = true`).
fn build_full_address(lock_arg: &[u8; 20], hrp: &str) -> String {
    let mut payload = Vec::with_capacity(54);
    payload.push(0x00); // AddressType::Full
    payload.extend_from_slice(&SIGHASH_TYPE_HASH);
    payload.push(HASH_TYPE_TYPE);
    payload.extend_from_slice(lock_arg);

    bech32::encode(hrp, payload.to_base32(), Variant::Bech32m).expect("bech32m encode succeeds")
}

/// Legacy short address (bech32) — ckb-cli `address(deprecated)`.
fn build_short_address(lock_arg: &[u8; 20], hrp: &str) -> String {
    let mut payload = [0u8; 22];
    payload[0] = 0x01; // AddressType::Short
    payload[1] = CODE_HASH_INDEX_SIGHASH;
    payload[2..].copy_from_slice(lock_arg);

    bech32::encode(hrp, payload.to_base32(), Variant::Bech32).expect("bech32 encode succeeds")
}

/// Generate a CKB testnet address from lock_arg (CKB2021 full format).
pub fn ckb_address_testnet(lock_arg: &[u8; 20]) -> String {
    build_full_address(lock_arg, "ckt")
}

/// Generate a CKB mainnet address from lock_arg (CKB2021 full format).
pub fn ckb_address_mainnet(lock_arg: &[u8; 20]) -> String {
    build_full_address(lock_arg, "ckb")
}

/// Generate a CKB address from a compressed pubkey, matching ckb-cli `util key-info`.
pub fn ckb_address_from_pubkey(pubkey: &PublicKey, testnet: bool) -> String {
    let lock_arg = lock_arg_from_pubkey(pubkey);
    if testnet {
        ckb_address_testnet(&lock_arg)
    } else {
        ckb_address_mainnet(&lock_arg)
    }
}

/// Deprecated short-format testnet address (still valid on-chain).
pub fn ckb_address_testnet_short(lock_arg: &[u8; 20]) -> String {
    build_short_address(lock_arg, "ckt")
}

/// Extract the 20-byte lock args from a CKB bech32/bech32m address.
pub fn lock_arg_from_address(address: &str) -> Result<[u8; 20], crate::error::AppError> {
    use bech32::{convert_bits, Variant};

    let (_hrp, data, variant) = bech32::decode(address)
        .map_err(|e| crate::error::AppError::BadRequest(format!("Invalid CKB address: {e}")))?;

    if !matches!(variant, Variant::Bech32 | Variant::Bech32m) {
        return Err(crate::error::AppError::BadRequest(
            "Unsupported CKB address encoding".into(),
        ));
    }

    let payload = convert_bits(&data, 5, 8, false).map_err(|e| {
        crate::error::AppError::BadRequest(format!("CKB address payload decode failed: {e}"))
    })?;

    match payload.first().copied() {
        Some(0x00) if payload.len() >= 54 => {
            let mut lock_arg = [0u8; 20];
            lock_arg.copy_from_slice(&payload[34..54]);
            Ok(lock_arg)
        }
        Some(0x01) if payload.len() >= 22 => {
            let mut lock_arg = [0u8; 20];
            lock_arg.copy_from_slice(&payload[2..22]);
            Ok(lock_arg)
        }
        Some(0x02) | Some(0x04) if payload.len() >= 53 => {
            // Legacy full-data / full-type address: 0x02|0x04 | code_hash[32] | args
            let mut lock_arg = [0u8; 20];
            lock_arg.copy_from_slice(&payload[33..53]);
            Ok(lock_arg)
        }
        _ => Err(crate::error::AppError::BadRequest(format!(
            "Unsupported CKB address payload (len={})",
            payload.len()
        ))),
    }
}

/// Build the secp256k1_blake160_sighash_all lock script JSON for indexer queries.
pub fn secp256k1_blake160_lock_script(lock_arg: &[u8; 20]) -> serde_json::Value {
    serde_json::json!({
        "code_hash": format!("0x{}", hex::encode(SIGHASH_TYPE_HASH)),
        "hash_type": "type",
        "args": format!("0x{}", hex::encode(lock_arg)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bech32::convert_bits;
    use secp256k1::Secp256k1;

    // -----------------------------------------------------------------------
    // Known test vectors — verified against ckb-cli 1.8.0
    //
    // Private key:
    //   d00c06bfd800d27397002dca6fb0993d5ba6399b4238b2f29ee9deb97593d2bc
    // Derived via `ckb-cli util key-info --privkey-path <file>`:
    //   pubkey:    03fe6c6d09d1a0f70255cddf25c5ed57d41b5c08822ae710dc10f8c88290e0acdf
    //   lock_arg:  0xc8328aabcd9b9e8e64fbc566c4385c3bdeb219d7
    //   lock_hash: 0x32e555f3ff8e135cece1351a6a2971518392c1e30375c1e006ad0ce8eac07947
    //   testnet:   ckt1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqwgx292hnvmn68xf779vmzrshpmm6epn4c0cgwga
    //   mainnet:   ckb1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqwgx292hnvmn68xf779vmzrshpmm6epn4cp2rpz9
    // -----------------------------------------------------------------------

    const REFERENCE_PRIVATE_KEY_HEX: &str =
        "d00c06bfd800d27397002dca6fb0993d5ba6399b4238b2f29ee9deb97593d2bc";
    const REFERENCE_PUBKEY_HEX: &str =
        "03fe6c6d09d1a0f70255cddf25c5ed57d41b5c08822ae710dc10f8c88290e0acdf";
    const REFERENCE_LOCK_ARG_HEX: &str = "c8328aabcd9b9e8e64fbc566c4385c3bdeb219d7";
    const REFERENCE_LOCK_HASH_HEX: &str =
        "32e555f3ff8e135cece1351a6a2971518392c1e30375c1e006ad0ce8eac07947";
    const REFERENCE_TESTNET_ADDRESS: &str =
        "ckt1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqwgx292hnvmn68xf779vmzrshpmm6epn4c0cgwga";
    const REFERENCE_MAINNET_ADDRESS: &str =
        "ckb1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqwgx292hnvmn68xf779vmzrshpmm6epn4cp2rpz9";

    fn reference_secret_key() -> secp256k1::SecretKey {
        let bytes = hex::decode(REFERENCE_PRIVATE_KEY_HEX).unwrap();
        secp256k1::SecretKey::from_slice(&bytes).unwrap()
    }

    fn reference_public_key() -> PublicKey {
        let secp = Secp256k1::new();
        PublicKey::from_secret_key(&secp, &reference_secret_key())
    }

    #[test]
    fn test_blake160_known() {
        let pubkey = [0u8; 33];
        let hash = blake160(&pubkey);
        assert_eq!(hash.len(), 20);
    }

    #[test]
    fn test_ckb_sdk_mainnet_new_address_vector() {
        // ckb-sdk-5.0.0 address.rs test_new_full_address
        let lock_arg = hex::decode("b39bbc0b3673c7d36450bc14cfcdad2d559c6c64")
            .unwrap()
            .try_into()
            .unwrap();
        let addr = ckb_address_mainnet(&lock_arg);
        assert_eq!(
            addr,
            "ckb1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqdnnw7qkdnnclfkg59uzn8umtfd2kwxceqxwquc4"
        );
    }

    #[test]
    fn test_ckb_sdk_mainnet_short_address_vector() {
        let lock_arg = hex::decode("b39bbc0b3673c7d36450bc14cfcdad2d559c6c64")
            .unwrap()
            .try_into()
            .unwrap();
        let addr = build_short_address(&lock_arg, "ckb");
        assert_eq!(addr, "ckb1qyqt8xaupvm8837nv3gtc9x0ekkj64vud3jqfwyw5v");
    }

    #[test]
    fn test_address_from_private_key_matches_ckb_cli_flow() {
        // ckb-signer keystore test vector private key
        let sk_bytes =
            hex::decode("d00c06bfd800d27397002dca6fb0993d5ba6399b4238b2f29ee9deb97593d2bc")
                .unwrap();
        let sk = secp256k1::SecretKey::from_slice(&sk_bytes).unwrap();
        let secp = Secp256k1::new();
        let pk = PublicKey::from_secret_key(&secp, &sk);
        let lock_arg = lock_arg_from_pubkey(&pk);
        let addr = ckb_address_mainnet(&lock_arg);
        assert!(addr.starts_with("ckb1q"));
        bech32::decode(&addr).expect("address must decode as valid bech32m");
    }

    #[test]
    fn test_address_roundtrips() {
        let mut pubkey = [0xABu8; 33];
        pubkey[0] = 0x02;
        let lock_arg = blake160(&pubkey);

        let addr = ckb_address_testnet(&lock_arg);
        let (hrp, data_5bit, variant) = bech32::decode(&addr).expect("Decode own address");
        assert_eq!(hrp, "ckt");
        assert_eq!(variant, Variant::Bech32m);

        let payload = convert_bits(&data_5bit, 5, 8, false).expect("Convert back");
        assert_eq!(payload.len(), 54);
        assert_eq!(payload[0], 0x00);
        assert_eq!(&payload[1..33], SIGHASH_TYPE_HASH.as_slice());
        assert_eq!(payload[33], HASH_TYPE_TYPE);
        assert_eq!(&payload[34..54], lock_arg.as_slice());
    }

    #[test]
    fn test_lock_arg_from_user_address() {
        let addr = "ckt1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsq0qwp0ymwaejdhj2agsws2hhfxpz23mkhchz29x5";
        let lock_arg = lock_arg_from_address(addr).expect("decode address");
        assert_eq!(lock_arg.len(), 20);
        assert_eq!(
            hex::encode(lock_arg),
            "e0705e4dbbb9936f25751074157ba4c112a3bb5f"
        );
    }

    // -----------------------------------------------------------------------
    // HD wallet lock_args & address tests — verified against ckb-cli
    // -----------------------------------------------------------------------

    #[test]
    fn test_pubkey_derivation_matches_ckb_cli() {
        // Verify our secp256k1 impl produces the same pubkey as ckb-cli
        let pk = reference_public_key();
        assert_eq!(hex::encode(pk.serialize()), REFERENCE_PUBKEY_HEX);
    }

    #[test]
    fn test_lock_arg_from_pubkey_matches_ckb_cli() {
        // The critical test: lock_arg = blake160(compressed_pubkey)
        let pk = reference_public_key();
        let lock_arg = lock_arg_from_pubkey(&pk);
        assert_eq!(
            hex::encode(lock_arg),
            REFERENCE_LOCK_ARG_HEX,
            "lock_arg must match ckb-cli output"
        );
    }

    #[test]
    fn test_script_lock_hash_matches_ckb_cli() {
        let lock_arg = hex::decode(REFERENCE_LOCK_ARG_HEX)
            .unwrap()
            .try_into()
            .unwrap();
        let lock_hash = script_lock_hash(&lock_arg);
        assert_eq!(
            hex::encode(lock_hash),
            REFERENCE_LOCK_HASH_HEX,
            "lock_hash must match ckb-cli output"
        );
    }

    #[test]
    fn test_mainnet_address_matches_ckb_cli() {
        let pk = reference_public_key();
        let addr = ckb_address_from_pubkey(&pk, false);
        assert_eq!(
            addr, REFERENCE_MAINNET_ADDRESS,
            "mainnet address must match ckb-cli output"
        );
    }

    #[test]
    fn test_testnet_address_matches_ckb_cli() {
        let pk = reference_public_key();
        let addr = ckb_address_from_pubkey(&pk, true);
        assert_eq!(
            addr, REFERENCE_TESTNET_ADDRESS,
            "testnet address must match ckb-cli output"
        );
    }

    #[test]
    fn test_lock_arg_from_address_roundtrip_ckb_cli() {
        // Decode the ckb-cli-produced testnet address and verify lock_arg is
        // properly extracted.
        let lock_arg = lock_arg_from_address(REFERENCE_TESTNET_ADDRESS)
            .expect("must decode valid ckb-cli address");
        assert_eq!(hex::encode(lock_arg), REFERENCE_LOCK_ARG_HEX);

        // Same for mainnet
        let lock_arg2 = lock_arg_from_address(REFERENCE_MAINNET_ADDRESS)
            .expect("must decode valid ckb-cli mainnet address");
        assert_eq!(hex::encode(lock_arg2), REFERENCE_LOCK_ARG_HEX);
    }

    #[test]
    fn test_blake160_deterministic() {
        let mut pubkey = [0x03u8; 33];
        pubkey[1] = 0xfe;
        let h1 = blake160(&pubkey);
        let h2 = blake160(&pubkey);
        assert_eq!(h1, h2, "blake160 must be deterministic");
        // Different pubkey → different hash
        pubkey[31] ^= 1;
        let h3 = blake160(&pubkey);
        assert_ne!(h1, h3);
    }
}
