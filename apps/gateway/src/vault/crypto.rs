//! Cryptographic operations for the OnlyKey Vault.
//!
//! - HKDF-SHA256 for deriving subkeys from the OnlyKey shared secret
//! - AES-256-GCM for unwrapping record keys and decrypting secrets
//!
//! The browser derives a shared secret via `ok.derive_shared_secret()` using the
//! per-record OneCLI public key. That raw secret is NEVER used directly as a key.
//! Instead we run HKDF to derive purpose-specific subkeys.

use anyhow::{bail, Context, Result};
use base64::Engine;
use ring::aead;
use ring::hkdf;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

// ── HKDF labels ─────────────────────────────────────────────────────────

/// Label for deriving the key-wrapping key from the OnlyKey shared secret.
const HKDF_LABEL_WRAP_KEY: &[u8] = b"okg-wrap-key-v1";

// ── HKDF derivation ─────────────────────────────────────────────────────

/// Derive a 32-byte wrapping key from the raw OnlyKey shared secret using HKDF-SHA256.
///
/// `context` should include record_id, purpose, version, etc. to bind the
/// derived key to a specific record.
pub fn derive_wrap_key(
    shared_secret: &[u8],
    context: &[u8],
) -> Result<[u8; KEY_LEN]> {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, context);
    let prk = salt.extract(shared_secret);

    let info = &[HKDF_LABEL_WRAP_KEY];
    let okm = prk
        .expand(info, HkdfKeyLen)
        .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;

    let mut key = [0u8; KEY_LEN];
    okm.fill(&mut key)
        .map_err(|_| anyhow::anyhow!("HKDF fill failed"))?;

    Ok(key)
}

/// ring HKDF requires a type implementing `KeyType` to specify output length.
struct HkdfKeyLen;

impl hkdf::KeyType for HkdfKeyLen {
    fn len(&self) -> usize {
        KEY_LEN
    }
}

// ── AES-256-GCM operations ──────────────────────────────────────────────

/// Decrypt ciphertext using AES-256-GCM.
///
/// `key` must be exactly 32 bytes.
/// `nonce` must be exactly 12 bytes.
/// `aad` is the additional authenticated data.
/// Returns the plaintext bytes.
pub fn aes_gcm_decrypt(
    key: &[u8],
    nonce_bytes: &[u8],
    aad: &[u8],
    ciphertext_with_tag: &[u8],
) -> Result<Vec<u8>> {
    if key.len() != KEY_LEN {
        bail!("AES key must be {KEY_LEN} bytes, got {}", key.len());
    }
    if nonce_bytes.len() != NONCE_LEN {
        bail!("nonce must be {NONCE_LEN} bytes, got {}", nonce_bytes.len());
    }

    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key)
        .map_err(|_| anyhow::anyhow!("failed to create AES-256-GCM key"))?;
    let less_safe_key = aead::LessSafeKey::new(unbound_key);

    let nonce = aead::Nonce::try_assume_unique_for_key(nonce_bytes)
        .map_err(|_| anyhow::anyhow!("invalid nonce"))?;

    let mut in_out = ciphertext_with_tag.to_vec();
    let plaintext = less_safe_key
        .open_in_place(nonce, aead::Aad::from(aad), &mut in_out)
        .map_err(|_| anyhow::anyhow!("AES-GCM decryption failed"))?;

    Ok(plaintext.to_vec())
}

/// Encrypt plaintext using AES-256-GCM.
///
/// Returns (ciphertext_with_tag, nonce).
pub fn aes_gcm_encrypt(
    key: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; NONCE_LEN])> {
    if key.len() != KEY_LEN {
        bail!("AES key must be {KEY_LEN} bytes, got {}", key.len());
    }

    let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, key)
        .map_err(|_| anyhow::anyhow!("failed to create AES-256-GCM key"))?;
    let less_safe_key = aead::LessSafeKey::new(unbound_key);

    let rng = ring::rand::SystemRandom::new();
    let mut nonce_bytes = [0u8; NONCE_LEN];
    ring::rand::SecureRandom::fill(&rng, &mut nonce_bytes)
        .map_err(|_| anyhow::anyhow!("failed to generate nonce"))?;

    let nonce = aead::Nonce::try_assume_unique_for_key(&nonce_bytes)
        .map_err(|_| anyhow::anyhow!("invalid nonce"))?;

    let mut in_out = plaintext.to_vec();
    less_safe_key
        .seal_in_place_append_tag(nonce, aead::Aad::from(aad), &mut in_out)
        .map_err(|_| anyhow::anyhow!("AES-GCM encryption failed"))?;

    Ok((in_out, nonce_bytes))
}

// ── High-level vault operations ─────────────────────────────────────────

/// Unwrap a record key using the browser-derived shared secret.
///
/// 1. HKDF the raw shared secret → wrapping key
/// 2. AES-GCM decrypt the wrapped record key
/// 3. Return the raw record key bytes
///
/// The caller should cache this record key in memory and immediately
/// zeroize the shared secret and wrapping key.
pub fn unwrap_record_key(
    derived_secret_b64: &str,
    wrapped_key_b64: &str,
    wrapped_key_nonce_b64: &str,
    derivation_context_json: &str,
) -> Result<Vec<u8>> {
    let b64 = &base64::engine::general_purpose::STANDARD;

    let derived_secret = b64
        .decode(derived_secret_b64)
        .context("invalid derived_secret_b64")?;

    let wrapped_key = b64
        .decode(wrapped_key_b64)
        .context("invalid wrapped_key_b64")?;

    let wrapped_nonce = b64
        .decode(wrapped_key_nonce_b64)
        .context("invalid wrapped_key_nonce_b64")?;

    // HKDF: derive wrapping key from shared secret + context
    let wrap_key = derive_wrap_key(&derived_secret, derivation_context_json.as_bytes())?;

    // Unwrap the record key
    let record_key = aes_gcm_decrypt(
        &wrap_key,
        &wrapped_nonce,
        derivation_context_json.as_bytes(), // AAD = derivation context
        &wrapped_key,
    )?;

    // Best-effort zeroize temporaries (limited in safe Rust)
    drop(derived_secret);
    drop(wrap_key);

    Ok(record_key)
}

/// Decrypt a vault secret using the unwrapped record key.
///
/// The record key should come from the in-memory cache (not re-derived each time
/// unless the cache has expired).
pub fn decrypt_vault_secret(
    record_key: &[u8],
    ciphertext_b64: &str,
    nonce_b64: &str,
    aad_json: &str,
) -> Result<String> {
    let b64 = &base64::engine::general_purpose::STANDARD;

    let ciphertext = b64.decode(ciphertext_b64).context("invalid ciphertext_b64")?;
    let nonce = b64.decode(nonce_b64).context("invalid nonce_b64")?;

    let plaintext_bytes = aes_gcm_decrypt(record_key, &nonce, aad_json.as_bytes(), &ciphertext)?;

    String::from_utf8(plaintext_bytes).context("decrypted value is not valid UTF-8")
}

/// Wrap a record key for storage, using a derived wrapping key.
///
/// Used during record creation:
/// 1. Browser derives shared secret via OnlyKey
/// 2. HKDF → wrapping key
/// 3. AES-GCM encrypt the random record key
/// 4. Store wrapped key blob
pub fn wrap_record_key(
    derived_secret_b64: &str,
    record_key: &[u8],
    derivation_context_json: &str,
) -> Result<(String, String)> {
    let b64 = &base64::engine::general_purpose::STANDARD;

    let derived_secret = b64
        .decode(derived_secret_b64)
        .context("invalid derived_secret_b64")?;

    let wrap_key = derive_wrap_key(&derived_secret, derivation_context_json.as_bytes())?;

    let (wrapped_with_tag, nonce) = aes_gcm_encrypt(
        &wrap_key,
        derivation_context_json.as_bytes(),
        record_key,
    )?;

    Ok((b64.encode(wrapped_with_tag), b64.encode(nonce)))
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::{SecureRandom, SystemRandom};

    fn random_bytes(len: usize) -> Vec<u8> {
        let rng = SystemRandom::new();
        let mut buf = vec![0u8; len];
        rng.fill(&mut buf).expect("generate random bytes");
        buf
    }

    #[test]
    fn hkdf_derive_produces_32_bytes() {
        let secret = random_bytes(32);
        let context = b"test-context";
        let key = derive_wrap_key(&secret, context).expect("derive key");
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn hkdf_deterministic() {
        let secret = random_bytes(32);
        let context = b"record_123:wrap:v1";
        let key1 = derive_wrap_key(&secret, context).expect("derive 1");
        let key2 = derive_wrap_key(&secret, context).expect("derive 2");
        assert_eq!(key1, key2);
    }

    #[test]
    fn hkdf_different_context_different_key() {
        let secret = random_bytes(32);
        let key1 = derive_wrap_key(&secret, b"context-a").expect("derive a");
        let key2 = derive_wrap_key(&secret, b"context-b").expect("derive b");
        assert_ne!(key1, key2);
    }

    #[test]
    fn aes_gcm_round_trip() {
        let key = random_bytes(32);
        let aad = b"test-aad";
        let plaintext = b"hello world secret";

        let (ciphertext, nonce) = aes_gcm_encrypt(&key, aad, plaintext).expect("encrypt");
        let decrypted = aes_gcm_decrypt(&key, &nonce, aad, &ciphertext).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn aes_gcm_wrong_key_fails() {
        let key1 = random_bytes(32);
        let key2 = random_bytes(32);
        let aad = b"test";
        let (ciphertext, nonce) = aes_gcm_encrypt(&key1, aad, b"secret").expect("encrypt");
        assert!(aes_gcm_decrypt(&key2, &nonce, aad, &ciphertext).is_err());
    }

    #[test]
    fn aes_gcm_wrong_aad_fails() {
        let key = random_bytes(32);
        let (ciphertext, nonce) = aes_gcm_encrypt(&key, b"aad-1", b"secret").expect("encrypt");
        assert!(aes_gcm_decrypt(&key, &nonce, b"aad-2", &ciphertext).is_err());
    }

    #[test]
    fn wrap_unwrap_record_key_round_trip() {
        let b64 = &base64::engine::general_purpose::STANDARD;

        // Simulate browser-derived shared secret
        let shared_secret = random_bytes(32);
        let shared_secret_b64 = b64.encode(&shared_secret);

        // Random record key
        let record_key = random_bytes(32);

        let context = r#"{"record_id":"rec_123","purpose":"record_key_wrap","version":1}"#;

        // Wrap
        let (wrapped_b64, nonce_b64) =
            wrap_record_key(&shared_secret_b64, &record_key, context).expect("wrap");

        // Unwrap
        let recovered = unwrap_record_key(&shared_secret_b64, &wrapped_b64, &nonce_b64, context)
            .expect("unwrap");

        assert_eq!(recovered, record_key);
    }

    #[test]
    fn decrypt_vault_secret_round_trip() {
        let b64 = &base64::engine::general_purpose::STANDARD;
        let record_key = random_bytes(32);
        let aad = r#"{"record_id":"rec_123","type":"api_key","version":1}"#;
        let secret = "sk-ant-api03-my-secret-key";

        // Encrypt
        let (ciphertext, nonce) =
            aes_gcm_encrypt(&record_key, aad.as_bytes(), secret.as_bytes()).expect("encrypt");

        let ct_b64 = b64.encode(&ciphertext);
        let nonce_b64 = b64.encode(&nonce);

        // Decrypt
        let decrypted = decrypt_vault_secret(&record_key, &ct_b64, &nonce_b64, aad)
            .expect("decrypt");

        assert_eq!(decrypted, secret);
    }
}
