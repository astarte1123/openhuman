use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::AppState;

#[derive(Debug)]
struct WindowEntry {
    count: u32,
    window_start: Instant,
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, WindowEntry>>>,
    max_per_min: u32,
}

impl RateLimiter {
    pub fn new(max_per_min: u32) -> Self {
        RateLimiter {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_per_min,
        }
    }

    /// Returns Ok(()) if the request is allowed, Err(remaining_secs) if rate-limited.
    pub fn check(&self, ip: IpAddr) -> Result<(), u64> {
        let mut map = self.state.lock().unwrap();
        let now = Instant::now();
        let window_secs = 60u64;

        let entry = map.entry(ip).or_insert(WindowEntry {
            count: 0,
            window_start: now,
        });

        let elapsed = now.duration_since(entry.window_start).as_secs();
        if elapsed >= window_secs {
            // Reset window
            entry.count = 1;
            entry.window_start = now;
            return Ok(());
        }

        if entry.count >= self.max_per_min {
            let retry_after = window_secs.saturating_sub(elapsed);
            return Err(retry_after);
        }

        entry.count += 1;
        Ok(())
    }

    /// Periodic cleanup of stale entries (call from background task)
    pub fn cleanup(&self) {
        let mut map = self.state.lock().unwrap();
        let now = Instant::now();
        map.retain(|_, entry| now.duration_since(entry.window_start).as_secs() < 120);
    }
}

/// Axum extractor that enforces rate limiting on the public API.
pub struct RateLimited;

pub struct RateLimitError {
    retry_after: u64,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": "Too many requests",
            "code": "RATE_LIMITED",
            "retry_after": self.retry_after,
        });
        let mut resp = (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(body),
        )
            .into_response();
        resp.headers_mut().insert(
            "Retry-After",
            HeaderValue::from_str(&self.retry_after.to_string()).unwrap(),
        );
        resp
    }
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for RateLimited {
    type Rejection = RateLimitError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // Extract client IP: check X-Forwarded-For first (behind nginx proxy)
        let ip = extract_ip(parts);

        if let Err(retry_after) = state.rate_limiter.check(ip) {
            tracing::warn!(ip = %ip, retry_after = retry_after, "[rate_limit] request blocked");
            return Err(RateLimitError { retry_after });
        }

        Ok(RateLimited)
    }
}

fn extract_ip(parts: &Parts) -> IpAddr {
    // Try X-Forwarded-For header (nginx proxy)
    if let Some(xff) = parts.headers.get("X-Forwarded-For") {
        if let Ok(val) = xff.to_str() {
            if let Some(first) = val.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    // Try X-Real-IP
    if let Some(xri) = parts.headers.get("X-Real-IP") {
        if let Ok(val) = xri.to_str() {
            if let Ok(ip) = val.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    // Fall back to connection IP from extensions
    parts
        .extensions
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or_else(|| "127.0.0.1".parse().unwrap())
}
