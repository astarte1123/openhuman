// encryption.rs — AES-256-GCM at-rest encryption for Youma.
//
// Key derivation: SHA-256(data_dir_bytes + "youma-enc-v1").
// Machine-specific, transparent — no user passphrase.
// Key is initialised once at startup via init(data_dir) and then
// accessible everywhere via key().

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

static ENC_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Must be called once during Tauri setup, before load_env_file() or any DB op.
pub fn init(data_dir: &str) {
    let k = derive_key(data_dir);
    ENC_KEY.set(k).ok();
    tracing::debug!("[encryption] init: machine key derived from data_dir");
}

/// Panics if init() was not called first.
pub fn key() -> &'static [u8; 32] {
    ENC_KEY.get().expect("[encryption] key() called before init()")
}

fn derive_key(data_dir: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data_dir.as_bytes());
    hasher.update(b"youma-enc-v1");
    hasher.finalize().into()
}

/// Encrypt `plaintext` → base64(nonce[12] || ciphertext).
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> String {
    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .unwrap_or_else(|e| {
            tracing::error!("[encryption] fatal: aes-gcm encrypt failed: {e}");
            panic!("encryption failed (encrypt): aes-gcm encrypt failed: {e}");
        });
    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(&out)
}

/// Decrypt base64(nonce[12] || ciphertext) → plaintext bytes.
pub fn decrypt(key: &[u8; 32], b64: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| {
            tracing::error!("[encryption] decryption failed (decrypt): base64 decode error: {e}");
            format!("decryption failed (decrypt): base64 decode error: {e}")
        })?;
    if bytes.len() < 13 {
        tracing::error!("[encryption] decryption failed (decrypt): ciphertext too short ({} bytes)", bytes.len());
        return Err("decryption failed (decrypt): invalid ciphertext — too short".to_string());
    }
    let nonce = Nonce::from_slice(&bytes[..12]);
    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);
    cipher
        .decrypt(nonce, &bytes[12..])
        .map_err(|e| {
            tracing::error!("[encryption] decryption failed (decrypt): aes-gcm error: {e}");
            format!("decryption failed (decrypt): aes-gcm error: {e}")
        })
}

/// Encrypt UTF-8 string → base64 ciphertext.
pub fn encrypt_str(key: &[u8; 32], s: &str) -> String {
    encrypt(key, s.as_bytes())
}

/// Decrypt base64 ciphertext → UTF-8 string.
pub fn decrypt_str(key: &[u8; 32], b64: &str) -> Result<String, String> {
    let bytes = decrypt(key, b64)?;
    String::from_utf8(bytes).map_err(|e| {
        tracing::error!("[encryption] decryption failed (decrypt_str): utf-8 decode error: {e}");
        format!("decryption failed (decrypt_str): utf-8 decode error: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed 32-byte test key — avoids touching the global OnceLock or the filesystem.
    fn test_key() -> [u8; 32] {
        derive_key("/tmp/youma-test-enc-unit")
    }

    #[test]
    fn round_trip_encrypt_decrypt() {
        let key = test_key();
        let original = "sk-test-secret-api-key-12345abcdef";
        let encrypted = encrypt_str(&key, original);
        assert_ne!(encrypted, original, "ciphertext must differ from plaintext");
        let decrypted = decrypt_str(&key, &encrypted).expect("decrypt should succeed");
        assert_eq!(decrypted, original, "round-trip must recover original plaintext");
    }

    #[test]
    fn empty_string_round_trip() {
        let key = test_key();
        let encrypted = encrypt_str(&key, "");
        let decrypted = decrypt_str(&key, &encrypted).expect("decrypt empty string");
        assert_eq!(decrypted, "", "round-trip of empty string should yield empty string");
    }

    #[test]
    fn invalid_ciphertext_returns_error() {
        let key = test_key();
        let result = decrypt_str(&key, "this-is-not-valid-ciphertext");
        assert!(result.is_err(), "invalid ciphertext should return Err");
    }

    #[test]
    fn too_short_ciphertext_returns_error() {
        let key = test_key();
        // Valid base64 but fewer than 13 bytes when decoded → "too short" branch.
        use base64::Engine as _;
        let short_b64 = base64::engine::general_purpose::STANDARD.encode(b"short");
        let result = decrypt(&key, &short_b64);
        assert!(result.is_err(), "ciphertext < 13 bytes should return Err");
        assert!(
            result.unwrap_err().contains("too short"),
            "error message should mention 'too short'"
        );
    }

    #[test]
    fn different_plaintexts_produce_different_ciphertexts() {
        let key = test_key();
        let enc1 = encrypt_str(&key, "hello");
        let enc2 = encrypt_str(&key, "world");
        assert_ne!(enc1, enc2, "distinct plaintexts must produce distinct ciphertexts");
    }

    #[test]
    fn same_plaintext_produces_different_ciphertexts_each_call() {
        // AES-GCM uses a random nonce so repeated encryption of the same plaintext
        // must yield different ciphertexts (IND-CPA property).
        let key = test_key();
        let enc1 = encrypt_str(&key, "repeat-me");
        let enc2 = encrypt_str(&key, "repeat-me");
        assert_ne!(enc1, enc2, "random nonce means same plaintext encrypts differently each time");
    }

    #[test]
    fn wrong_key_returns_error() {
        let key1 = test_key();
        let key2 = derive_key("/tmp/youma-test-enc-other-key");
        let encrypted = encrypt_str(&key1, "secret-value");
        let result = decrypt_str(&key2, &encrypted);
        assert!(result.is_err(), "decrypting with the wrong key should return Err");
    }

    #[test]
    fn unicode_round_trip() {
        let key = test_key();
        let original = "こんにちは — Héllo Wörld 🌍";
        let encrypted = encrypt_str(&key, original);
        let decrypted = decrypt_str(&key, &encrypted).expect("decrypt unicode");
        assert_eq!(decrypted, original, "unicode round-trip must be lossless");
    }

    #[test]
    fn derive_key_is_deterministic() {
        let k1 = derive_key("/some/path");
        let k2 = derive_key("/some/path");
        assert_eq!(k1, k2, "derive_key must be deterministic for the same input");
    }

    #[test]
    fn derive_key_differs_for_different_paths() {
        let k1 = derive_key("/path/a");
        let k2 = derive_key("/path/b");
        assert_ne!(k1, k2, "derive_key must differ for distinct paths");
    }
}
