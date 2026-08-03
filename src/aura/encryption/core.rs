use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{self, Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Salt length for Argon2id key derivation
const SALT_LENGTH: usize = 16;
/// Nonce length for AES-256-GCM (96 bits)
const NONCE_LENGTH: usize = 12;
/// Derived key length (256 bits for AES-256)
const KEY_LENGTH: usize = 32;

/// Encrypted payload with metadata for decryption
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedPayload {
    /// AES-256-GCM ciphertext
    pub ciphertext: Vec<u8>,
    /// Random nonce used for this encryption
    pub nonce: Vec<u8>,
    /// Argon2id salt used for key derivation
    pub salt: Vec<u8>,
}

/// Encryption key material
#[derive(Clone)]
pub struct EncryptionKey {
    key_bytes: [u8; KEY_LENGTH],
}

impl EncryptionKey {
    /// Derive an encryption key from a password and salt using Argon2id.
    pub fn derive(password: &str, salt: &[u8]) -> Result<Self, String> {
        let params = Params::new(65536, 3, 1, Some(KEY_LENGTH))
            .map_err(|e| format!("Argon2 params error: {e}"))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key_bytes = [0u8; KEY_LENGTH];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key_bytes)
            .map_err(|e| format!("Key derivation failed: {e}"))?;

        Ok(Self { key_bytes })
    }

    /// Generate a new random salt for key derivation.
    pub fn generate_salt() -> Vec<u8> {
        let mut salt = vec![0u8; SALT_LENGTH];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    /// Encrypt plaintext bytes.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedPayload, String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key_bytes).map_err(|e| format!("Cipher init: {e}"))?;

        let mut nonce_bytes = [0u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("Encryption failed: {e}"))?;

        Ok(EncryptedPayload {
            ciphertext,
            nonce: nonce_bytes.to_vec(),
            salt: Vec::new(), // Salt is stored separately in the key file
        })
    }

    /// Decrypt an encrypted payload.
    pub fn decrypt(&self, payload: &EncryptedPayload) -> Result<Vec<u8>, String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key_bytes).map_err(|e| format!("Cipher init: {e}"))?;

        let nonce = Nonce::from_slice(&payload.nonce);

        cipher
            .decrypt(nonce, payload.ciphertext.as_ref())
            .map_err(|e| format!("Decryption failed: {e}"))
    }

    /// Encrypt a string and return base64-encoded JSON payload.
    pub fn encrypt_string(&self, plaintext: &str) -> Result<String, String> {
        let payload = self.encrypt(plaintext.as_bytes())?;
        serde_json::to_string(&payload).map_err(|e| format!("Serialization failed: {e}"))
    }

    /// Decrypt a base64-encoded JSON payload back to a string.
    pub fn decrypt_string(&self, encrypted_json: &str) -> Result<String, String> {
        let payload: EncryptedPayload =
            serde_json::from_str(encrypted_json).map_err(|e| format!("Deserialization: {e}"))?;
        let plaintext = self.decrypt(&payload)?;
        String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode: {e}"))
    }
}

/// Get the path to the Aura data directory.
/// If an active user is set, returns the user-scoped directory under the
/// env-aware root returned by `default_root_aura_dir()`
/// (for example `~/.aura/users/{user_id}` in production or
/// `~/.aura-staging/users/{user_id}` when `AURA_APP_ENV=staging`);
/// otherwise it falls back to that root directory itself.
pub fn get_data_dir() -> Result<PathBuf, String> {
    let root_dir = crate::aura::config::default_root_aura_dir()
        .map_err(|e| format!("Cannot determine app data directory: {e}"))?;
    std::fs::create_dir_all(&root_dir)
        .map_err(|e| format!("Failed to create data directory: {e}"))?;

    let data_dir = if let Some(user_id) = crate::aura::config::read_active_user_id(&root_dir) {
        let user_dir = crate::aura::config::user_aura_dir(&root_dir, &user_id);
        std::fs::create_dir_all(&user_dir)
            .map_err(|e| format!("Failed to create user data directory: {e}"))?;
        user_dir
    } else {
        root_dir
    };

    Ok(data_dir)
}

/// Get the path to the encryption key file under the env-aware Aura root
/// (for example `~/.aura/encryption.key` or `~/.aura-staging/encryption.key`).
fn get_key_file_path() -> Result<PathBuf, String> {
    Ok(get_data_dir()?.join("encryption.key"))
}

/// Key file stores the salt; the actual key is derived at runtime from password.
#[derive(Serialize, Deserialize)]
struct KeyFile {
    salt: Vec<u8>,
    /// Version for future key rotation
    version: u32,
}

/// Initialize encryption with a password. Creates key file if needed.
pub async fn ai_init_encryption(password: String) -> Result<bool, String> {
    let key_path = get_key_file_path()?;

    if key_path.exists() {
        // Key file exists, verify password works by loading it
        let content =
            std::fs::read_to_string(&key_path).map_err(|e| format!("Read key file: {e}"))?;
        let key_file: KeyFile =
            serde_json::from_str(&content).map_err(|e| format!("Parse key file: {e}"))?;
        let _key = EncryptionKey::derive(&password, &key_file.salt)?;
        Ok(true)
    } else {
        // Create new key file with random salt
        let salt = EncryptionKey::generate_salt();
        let key_file = KeyFile { salt, version: 1 };
        let content =
            serde_json::to_string_pretty(&key_file).map_err(|e| format!("Serialize: {e}"))?;
        std::fs::write(&key_path, content).map_err(|e| format!("Write key file: {e}"))?;
        Ok(true)
    }
}

/// Encrypt a string value using the password-derived key.
pub async fn ai_encrypt(password: String, plaintext: String) -> Result<String, String> {
    let key_path = get_key_file_path()?;
    let content = std::fs::read_to_string(&key_path).map_err(|e| format!("Read key: {e}"))?;
    let key_file: KeyFile =
        serde_json::from_str(&content).map_err(|e| format!("Parse key: {e}"))?;
    let key = EncryptionKey::derive(&password, &key_file.salt)?;
    key.encrypt_string(&plaintext)
}

/// Decrypt a string value using the password-derived key.
pub async fn ai_decrypt(password: String, encrypted: String) -> Result<String, String> {
    let key_path = get_key_file_path()?;
    let content = std::fs::read_to_string(&key_path).map_err(|e| format!("Read key: {e}"))?;
    let key_file: KeyFile =
        serde_json::from_str(&content).map_err(|e| format!("Parse key: {e}"))?;
    let key = EncryptionKey::derive(&password, &key_file.salt)?;
    key.decrypt_string(&encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_bytes() {
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::derive("correct horse battery staple", &salt).unwrap();
        let plaintext = b"sqlite-page-or-embedding-blob";

        let payload = key.encrypt(plaintext).unwrap();
        let decrypted = key.decrypt(&payload).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn round_trips_strings() {
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::derive("pw", &salt).unwrap();
        let plaintext = "session transcript with unicode: héllo wörld 日本語";

        let encrypted = key.encrypt_string(plaintext).unwrap();
        let decrypted = key.decrypt_string(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn derive_is_deterministic_for_same_password_and_salt() {
        let salt = EncryptionKey::generate_salt();
        let key_a = EncryptionKey::derive("pw", &salt).unwrap();
        let key_b = EncryptionKey::derive("pw", &salt).unwrap();

        // Two independently-derived keys from the same password+salt must
        // agree: what `key_a` encrypts, `key_b` must be able to decrypt.
        let payload = key_a.encrypt(b"payload").unwrap();
        assert_eq!(key_b.decrypt(&payload).unwrap(), b"payload");
    }

    #[test]
    fn wrong_password_fails_to_decrypt() {
        let salt = EncryptionKey::generate_salt();
        let right_key = EncryptionKey::derive("correct-password", &salt).unwrap();
        let wrong_key = EncryptionKey::derive("wrong-password", &salt).unwrap();

        let payload = right_key.encrypt(b"top secret").unwrap();

        assert!(wrong_key.decrypt(&payload).is_err());
    }

    #[test]
    fn wrong_salt_fails_to_decrypt() {
        let salt_a = EncryptionKey::generate_salt();
        let salt_b = EncryptionKey::generate_salt();
        let key_a = EncryptionKey::derive("same-password", &salt_a).unwrap();
        let key_b = EncryptionKey::derive("same-password", &salt_b).unwrap();

        let payload = key_a.encrypt(b"data").unwrap();

        assert!(key_b.decrypt(&payload).is_err());
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::derive("pw", &salt).unwrap();
        let mut payload = key.encrypt(b"authenticated data").unwrap();

        // Flip a bit in the ciphertext; AES-GCM's auth tag must catch this
        // rather than silently returning corrupted plaintext.
        let last = payload.ciphertext.len() - 1;
        payload.ciphertext[last] ^= 0x01;

        assert!(key.decrypt(&payload).is_err());
    }

    #[test]
    fn tampered_nonce_is_rejected() {
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::derive("pw", &salt).unwrap();
        let mut payload = key.encrypt(b"authenticated data").unwrap();

        payload.nonce[0] ^= 0x01;

        assert!(key.decrypt(&payload).is_err());
    }

    #[test]
    fn same_plaintext_yields_different_ciphertext_each_call() {
        // Nonce must be fresh per encryption so identical plaintexts don't
        // leak equality via identical ciphertext (classic ECB-style leak).
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::derive("pw", &salt).unwrap();

        let a = key.encrypt(b"same plaintext").unwrap();
        let b = key.encrypt(b"same plaintext").unwrap();

        assert_ne!(a.ciphertext, b.ciphertext);
        assert_ne!(a.nonce, b.nonce);
    }

    #[test]
    fn generate_salt_has_expected_length_and_varies() {
        let a = EncryptionKey::generate_salt();
        let b = EncryptionKey::generate_salt();

        assert_eq!(a.len(), SALT_LENGTH);
        assert_eq!(b.len(), SALT_LENGTH);
        assert_ne!(a, b);
    }

    #[test]
    fn decrypt_string_rejects_malformed_json() {
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::derive("pw", &salt).unwrap();

        let result = key.decrypt_string("not valid json");

        assert!(result.is_err());
    }

    #[test]
    fn decrypt_string_rejects_truncated_ciphertext() {
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::derive("pw", &salt).unwrap();
        let mut payload = key.encrypt(b"data").unwrap();
        payload
            .ciphertext
            .truncate(payload.ciphertext.len().saturating_sub(4));

        let encrypted_json = serde_json::to_string(&payload).unwrap();
        assert!(key.decrypt_string(&encrypted_json).is_err());
    }
}
