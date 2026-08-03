use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Generate a license key in format: PROD-XXXX-XXXX-XXXX-XXXX (uppercase hex)
pub fn generate_license_key() -> String {
    let mut rng = rand::thread_rng();
    let groups: Vec<String> = (0..4)
        .map(|_| {
            let bytes: [u8; 2] = rng.gen();
            hex::encode_upper(bytes)
        })
        .collect();
    format!("PROD-{}", groups.join("-"))
}

/// SHA-256 hex of raw machine_id string
pub fn hash_machine_id(machine_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(machine_id.as_bytes());
    hex::encode(hasher.finalize())
}

/// HMAC-SHA256(secret, payload) → hex string
pub fn hmac_sign(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// SHA-256 hex of token string (for admin token storage)
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a random hex string of `len` characters (len must be even)
pub fn generate_hex_secret(len: usize) -> String {
    let byte_count = len / 2;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..byte_count).map(|_| rng.gen::<u8>()).collect();
    hex::encode(bytes)
}

/// Build HMAC signature payload for validate response
pub fn build_validate_signature(
    secret: &str,
    valid: bool,
    plan: &str,
    expires_at: Option<&str>,
    issued_at: &str,
) -> String {
    let expires_str = expires_at.unwrap_or("never");
    let payload = format!("{}|{}|{}|{}", valid, plan, expires_str, issued_at);
    hmac_sign(secret, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_key_format() {
        let key = generate_license_key();
        assert!(key.starts_with("PROD-"), "Key should start with PROD-");
        let parts: Vec<&str> = key.split('-').collect();
        assert_eq!(parts.len(), 5, "Key should have 5 parts");
        assert_eq!(parts[0], "PROD");
        for part in &parts[1..] {
            assert_eq!(part.len(), 4, "Each group should be 4 chars");
        }
    }

    #[test]
    fn test_hash_machine_id_deterministic() {
        let h1 = hash_machine_id("my-machine-123");
        let h2 = hash_machine_id("my-machine-123");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex is 64 chars
    }

    #[test]
    fn test_hmac_sign() {
        let sig = hmac_sign("secret", "payload");
        assert_eq!(sig.len(), 64);
        // Deterministic
        assert_eq!(sig, hmac_sign("secret", "payload"));
        // Different secret → different sig
        assert_ne!(sig, hmac_sign("other-secret", "payload"));
    }
}
