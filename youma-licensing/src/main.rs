mod auth;
mod config;
mod crypto;
mod db;
mod error;
mod models;
mod rate_limit;
mod routes;
mod scheduler;
mod webhook;

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode, Uri},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, patch, post, put},
    Router,
};
use rate_limit::RateLimiter;
use rusqlite::Connection;
use std::{net::SocketAddr, sync::{Arc, Mutex}};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};
use tracing::{error, info};

// ─── App state shared across all handlers ─────────────────────────────────────

pub struct AppState {
    pub db: Mutex<Connection>,
    pub config: config::Config,
    pub rate_limiter: RateLimiter,
}

// ─── Embedded admin SPA ───────────────────────────────────────────────────────

static ADMIN_HTML: &str = include_str!("../static/index.html");

async fn serve_admin(_uri: Uri) -> impl IntoResponse {
    Html(ADMIN_HTML)
}

async fn root_redirect() -> impl IntoResponse {
    Redirect::permanent("/admin")
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not found")
}

/// Attaches security headers and a correlation request ID to every response.
async fn security_and_request_id(req: Request, next: Next) -> Response {
    let req_id = req
        .headers()
        .get("X-Request-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut resp = next.run(req).await;
    let h = resp.headers_mut();

    if let Ok(v) = HeaderValue::from_str(&req_id) {
        h.insert("X-Request-ID", v);
    }
    h.insert("X-Content-Type-Options",  HeaderValue::from_static("nosniff"));
    h.insert("X-Frame-Options",         HeaderValue::from_static("SAMEORIGIN"));
    h.insert("X-XSS-Protection",        HeaderValue::from_static("1; mode=block"));
    h.insert("Referrer-Policy",         HeaderValue::from_static("strict-origin-when-cross-origin"));
    h.insert("X-Youma-Server",          HeaderValue::from_static("youma-licensing/1.0"));

    resp
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Load .env file if present (ignore error if absent)
    let _ = dotenvy::dotenv();

    let config = config::Config::from_env();

    // Set up tracing
    let filter = format!("{},tower_http=info", config.log_level);
    if config.log_format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .pretty()
            .with_env_filter(filter)
            .init();
    }

    info!("[main] Youma Licensing Server v{}", env!("CARGO_PKG_VERSION"));
    info!("[main] database={}", config.database_path);
    info!("[main] bind={}", config.bind_addr);

    // Open database
    let conn = Connection::open(&config.database_path).unwrap_or_else(|e| {
        error!("[main] failed to open database: {}", e);
        std::process::exit(1);
    });

    let admin_token_hash = crypto::hash_token(&config.admin_token);
    db::init(&conn, &admin_token_hash).unwrap_or_else(|e| {
        error!("[main] database init failed: {}", e);
        std::process::exit(1);
    });

    let rate_limiter = RateLimiter::new(config.max_rate_per_min);
    let state = Arc::new(AppState {
        db: Mutex::new(conn),
        config: config.clone(),
        rate_limiter: rate_limiter.clone(),
    });

    // ─── Start background scheduler ───────────────────────────────────────────

    let sched_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            scheduler::run_scheduled_tasks(Arc::clone(&sched_state)).await;
        }
    });

    // Rate-limiter cleanup every 5 minutes
    let rl_clone = rate_limiter.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            rl_clone.cleanup();
        }
    });

    // ─── Router ───────────────────────────────────────────────────────────────

    // Public API (rate-limited per IP)
    let public_api = Router::new()
        .route("/license/activate", post(routes::public::activate))
        .route("/license/validate", post(routes::public::validate))
        .route("/license/deactivate", post(routes::public::deactivate))
        .route("/license/floating/heartbeat", post(routes::public::floating_heartbeat))
        .route("/license/trial", post(routes::public::start_trial))
        .route("/health", get(routes::public::health))
        .layer(RequestBodyLimitLayer::new(64 * 1024)); // 64KB

    // Admin API (bearer auth)
    let admin_api = Router::new()
        // Dashboard
        .route("/dashboard", get(routes::admin::dashboard::get_dashboard))
        // Organizations
        .route("/orgs", get(routes::admin::orgs::list_orgs))
        .route("/orgs", post(routes::admin::orgs::create_org))
        .route("/orgs/:id", get(routes::admin::orgs::get_org))
        .route("/orgs/:id", put(routes::admin::orgs::update_org))
        .route("/orgs/:id", delete(routes::admin::orgs::delete_org))
        // Pools
        .route("/pools", get(routes::admin::pools::list_pools))
        .route("/pools", post(routes::admin::pools::create_pool))
        .route("/pools/:id", get(routes::admin::pools::get_pool))
        .route("/pools/:id/generate", post(routes::admin::pools::generate_keys))
        .route("/pools/:id/keys", get(routes::admin::pools::list_pool_keys))
        .route("/pools/:id/export", get(routes::admin::pools::export_pool_keys))
        // Licenses
        .route("/licenses", get(routes::admin::licenses::list_licenses))
        .route("/licenses/:key", get(routes::admin::licenses::get_license))
        .route("/licenses/:key/suspend", post(routes::admin::licenses::suspend_license))
        .route("/licenses/:key/unsuspend", post(routes::admin::licenses::unsuspend_license))
        .route("/licenses/:key/revoke", post(routes::admin::licenses::revoke_license))
        .route("/licenses/:key/extend", post(routes::admin::licenses::extend_license))
        .route("/licenses/:key/transfer", post(routes::admin::licenses::transfer_license))
        .route("/licenses/:key/set-machine-label", post(routes::admin::licenses::set_machine_label))
        .route("/licenses/:key/max-transfers", patch(routes::admin::licenses::set_max_transfers))
        // Audit
        .route("/audit", get(routes::admin::audit::list_audit))
        // Webhooks
        .route("/webhooks", get(routes::admin::webhooks::list_webhooks))
        .route("/webhooks", post(routes::admin::webhooks::create_webhook))
        .route("/webhooks/:id", put(routes::admin::webhooks::update_webhook))
        .route("/webhooks/:id/test", post(routes::admin::webhooks::test_webhook))
        .route("/webhooks/:id", delete(routes::admin::webhooks::delete_webhook))
        .layer(RequestBodyLimitLayer::new(1024 * 1024)); // 1MB

    let api_v1 = Router::new()
        .nest("/", public_api)
        .nest("/admin", admin_api);

    let app = Router::new()
        .route("/", get(root_redirect))
        .route("/admin", get(serve_admin))
        .route("/admin/*path", get(serve_admin))
        .nest("/api/v1", api_v1)
        .fallback(not_found)
        .layer(middleware::from_fn(security_and_request_id))
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|req: &axum::http::Request<axum::body::Body>| {
                            let req_id = req
                                .headers()
                                .get("X-Request-ID")
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or("-");
                            tracing::info_span!(
                                "request",
                                method  = %req.method(),
                                path    = %req.uri().path(),
                                req_id  = %req_id,
                            )
                        })
                        .on_response(
                            tower_http::trace::DefaultOnResponse::new()
                                .level(tracing::Level::INFO),
                        ),
                )
                .layer(CompressionLayer::new())
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods(Any)
                        .allow_headers(Any),
                ),
        )
        .with_state(Arc::clone(&state));

    // ─── Bind and serve ───────────────────────────────────────────────────────

    let addr: SocketAddr = config.bind_addr.parse().unwrap_or_else(|e| {
        error!("[main] invalid BIND_ADDR '{}': {}", config.bind_addr, e);
        std::process::exit(1);
    });

    info!("[main] listening on http://{}", addr);
    info!("[main] admin UI at http://{}/admin", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        error!("[main] failed to bind {}: {}", addr, e);
        std::process::exit(1);
    });

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap_or_else(|e| {
        error!("[main] server error: {}", e);
        std::process::exit(1);
    });

    info!("[main] server shut down cleanly");
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { info!("[main] received Ctrl+C, shutting down"); },
        _ = terminate => { info!("[main] received SIGTERM, shutting down"); },
    }
}
