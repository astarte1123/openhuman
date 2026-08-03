use crate::crypto;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: String,
    pub admin_token: String,
    pub signing_secret: String,
    pub bind_addr: String,
    pub trial_duration_days: i64,
    pub max_rate_per_min: u32,
    pub log_level: String,
    pub log_format: String,
}

impl Config {
    pub fn from_env() -> Self {
        let admin_token = env::var("ADMIN_TOKEN").unwrap_or_else(|_| {
            let token = crypto::generate_hex_secret(64);
            eprintln!(
                "[config] ADMIN_TOKEN not set — generated random token: {}",
                token
            );
            eprintln!("[config] Add this to your .env file: ADMIN_TOKEN={}", token);
            token
        });

        let signing_secret = env::var("SIGNING_SECRET").unwrap_or_else(|_| {
            let secret = crypto::generate_hex_secret(64);
            eprintln!(
                "[config] SIGNING_SECRET not set — generated random secret: {}",
                secret
            );
            eprintln!(
                "[config] Add this to your .env file: SIGNING_SECRET={}",
                secret
            );
            secret
        });

        Config {
            database_path: env::var("DATABASE_PATH")
                .unwrap_or_else(|_| "./youma-licenses.db".to_string()),
            admin_token,
            signing_secret,
            bind_addr: env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            trial_duration_days: env::var("TRIAL_DURATION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            max_rate_per_min: env::var("MAX_RATE_PER_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            log_format: env::var("LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string()),
        }
    }
}
