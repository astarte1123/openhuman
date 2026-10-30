//! HTTP/JSON-RPC server lifecycle: binding, background task bootstrap,
//! and long-lived domain event-bus subscriber registration.

use tokio_util::sync::CancellationToken;

use super::router::{build_core_http_router, core_host, core_port};

/// Runs the HTTP/JSON-RPC server.
///
/// This function binds to the specified host and port, initializes the router,
/// bootstraps long-lived runtime infrastructure, and starts serving requests.
pub async fn run_server(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
) -> anyhow::Result<()> {
    run_server_inner(host, port, socketio_enabled, false, None).await
}

/// Like [`run_server`] but marks the instance as embedded.
pub async fn run_server_embedded(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
    shutdown_token: CancellationToken,
) -> anyhow::Result<()> {
    run_server_inner(host, port, socketio_enabled, true, Some(shutdown_token)).await
}

/// Internal server entrypoint.
async fn run_server_inner(
    host: Option<&str>,
    port: Option<u16>,
    socketio_enabled: bool,
    embedded_core: bool,
    shutdown_token: Option<CancellationToken>,
) -> anyhow::Result<()> {
    // Ensure all controllers are registered before starting.
    let _ = crate::core::all::all_registered_controllers();

    // Initialize the per-process RPC bearer token.
    // Written to {workspace_dir}/core.token so the Tauri shell can read it.
    let token_dir = crate::aura::config::default_root_aura_dir().unwrap_or_else(|_| {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".aura")
    });
    crate::core::auth::init_rpc_token(&token_dir)?;

    // Initialize the global MemoryClient so composio providers
    // (gmail/slack/notion) can persist their sync_state via kv_get/kv_set,
    // and so any subsystem that calls `memory::global::client_if_ready()`
    // gets a live handle. Without this, every periodic sync bails with
    // "[composio:gmail] memory client not ready".
    {
        // Surface a config-load failure explicitly. Falling silently to
        // `Config::default()` would hide a serious operator-visible
        // problem (corrupt toml, permissions, missing AURA_WORKSPACE
        // workspace dir) and the memory client would init against the
        // wrong workspace — leading to chunk loss / cross-workspace
        // bleed-over. We log loud, then proceed with default so the
        // server still comes up; the operator sees the error in stderr
        // and can fix their config.
        let cfg = match crate::aura::config::Config::load_or_init().await {
            Ok(c) => c,
            Err(e) => {
                log::error!(
                    "[boot] memory::global init: Config::load_or_init failed ({e:#}); \
                     falling back to default workspace dir — fix your config.toml \
                     or AURA_WORKSPACE before relying on memory persistence"
                );
                Default::default()
            }
        };
        match crate::aura::memory::global::init(cfg.workspace_dir.clone()) {
            Ok(_) => log::info!(
                "[boot] memory::global initialized (workspace={})",
                cfg.workspace_dir.display()
            ),
            Err(e) => log::warn!("[boot] memory::global init failed: {e}"),
        }
        // Initialize the WhatsApp data store so scanner ingest calls
        // can write data without requiring a lazy-init fallback.
        match crate::aura::whatsapp_data::global::init(cfg.workspace_dir.clone()) {
            Ok(_) => log::info!(
                "[boot] whatsapp_data::global initialized (workspace={})",
                cfg.workspace_dir.display()
            ),
            Err(e) => log::warn!("[boot] whatsapp_data::global init failed: {e}"),
        }
    }

    let (resolved_port, port_source) = match port {
        Some(p) => (p, "CLI --port"),
        None => (
            core_port(),
            if std::env::var("AURA_CORE_PORT").is_ok() {
                "env AURA_CORE_PORT"
            } else {
                "default"
            },
        ),
    };
    let (resolved_host, host_source) = match host {
        Some(h) => (h.to_string(), "CLI --host"),
        None => (
            core_host(),
            if std::env::var("AURA_CORE_HOST")
                .ok()
                .filter(|s| !s.is_empty())
                .is_some()
            {
                "env AURA_CORE_HOST"
            } else {
                "default"
            },
        ),
    };

    log::debug!(
        "[core] Bind resolution: host={resolved_host} (from {host_source}), port={resolved_port} (from {port_source})"
    );

    let port = resolved_port;
    let host = resolved_host;
    let bind_addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .map_err(|e| {
            log::error!("[core] Failed to bind to {bind_addr}: {e}");
            e
        })?;

    let app = build_core_http_router(socketio_enabled);

    // --- Core runtime bootstrap --------------------------------------------
    bootstrap_core_runtime(embedded_core).await;

    log::info!(
        "[core] Aura core is ready — listening on http://{bind_addr} (version {})",
        env!("CARGO_PKG_VERSION")
    );
    log::info!("[rpc:http] JSON-RPC — POST http://{bind_addr}/rpc (JSON-RPC 2.0)");
    if socketio_enabled {
        log::info!("[rpc:socketio] Socket.IO — ws://{bind_addr}/socket.io/ (same HTTP server)");
    } else {
        log::info!("[rpc:socketio] disabled (--jsonrpc-only)");
    }

    // Background bootstrap for services — gated on login state.
    //
    // Heavy services (local AI, voice, screen intelligence, autocomplete)
    // are only started when a user is logged in. If no user session exists
    // on disk, startup is deferred until the login handler in
    // `credentials::ops::store_session()` triggers it.
    tokio::spawn(async move {
        match crate::aura::config::Config::load_or_init().await {
            Ok(config) => {
                if embedded_core {
                    log::debug!("[core] embedded core startup");
                } else {
                    log::debug!("[core] desktop core startup");
                }

                // Register autocomplete shutdown hook so the engine (and its
                // Swift overlay helper) are stopped cleanly on process exit.
                // This is unconditional — the hook should fire regardless of
                // whether the user is currently logged in.
                crate::core::shutdown::register(|| async {
                    let engine = crate::aura::autocomplete::global_engine();
                    let status = engine.status().await;
                    if status.running {
                        log::info!(
                            "[core] stopping autocomplete engine (phase={})",
                            status.phase
                        );
                        engine.stop(None).await;
                        log::info!("[core] autocomplete engine stopped");
                    }
                });

                // Check if a user is already logged in from a previous session.
                let already_logged_in = crate::aura::config::default_root_aura_dir()
                    .ok()
                    .and_then(|root| crate::aura::config::read_active_user_id(&root))
                    .is_some();

                if already_logged_in {
                    // User has an active session — start all services now.
                    log::info!("[services] existing session found, starting services");
                    crate::aura::credentials::ops::start_login_gated_services(&config).await;

                    // Subconscious engine + heartbeat.
                    if !config.heartbeat.enabled {
                        log::info!("[subconscious] disabled by config (heartbeat.enabled = false)");
                    } else {
                        match crate::aura::subconscious::global::bootstrap_after_login().await {
                            Ok(()) => log::info!(
                                "[subconscious] bootstrapped on startup (existing session)"
                            ),
                            Err(e) => log::warn!("[subconscious] startup bootstrap failed: {e}"),
                        }
                    }
                } else {
                    log::info!(
                        "[services] no active session — deferring service startup until login"
                    );
                }
            }
            Err(err) => {
                log::warn!("[core] config load failed, skipping service startup: {err}");
            }
        }
    });

    // Periodic self-update checker (default: every 1 hour).
    tokio::spawn(async {
        match crate::aura::config::Config::load_or_init().await {
            Ok(config) => {
                crate::aura::update::scheduler::run(config.update).await;
            }
            Err(err) => {
                log::warn!("[core] config load failed, skipping update scheduler: {err}");
            }
        }
    });

    // Cron scheduler — polls due_jobs() every ~5s and executes them automatically.
    tokio::spawn(async {
        match crate::aura::config::Config::load_or_init().await {
            Ok(config) => {
                if !config.cron.enabled {
                    log::info!("[cron] scheduler disabled via config; skipping");
                    return;
                }
                log::info!("[cron] spawning scheduler polling loop");
                if let Err(e) = crate::aura::cron::scheduler::run(config).await {
                    log::error!("[cron] scheduler loop ended with error: {e}");
                }
            }
            Err(err) => {
                log::warn!("[core] config load failed, skipping cron scheduler: {err}");
            }
        }
    });

    // Realtime channel listeners (Telegram getUpdates, Discord gateway, etc.) live in
    // `start_channels`. Without this task, `aura run` would only expose RPC while
    // inbound bot messages are never polled.
    if std::env::var("AURA_DISABLE_CHANNEL_LISTENERS")
        .ok()
        .filter(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .is_none()
    {
        tokio::spawn(async move {
            let config = match crate::aura::config::Config::load_or_init().await {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("[channels] could not load config for listeners: {e}");
                    return;
                }
            };
            if !config.channels_config.has_listening_integrations() {
                log::debug!(
                    "[channels] no channel integrations configured; not spawning listeners"
                );
                return;
            }
            log::info!("[channels] spawning in-process realtime listeners (Telegram, Discord, …)");
            if let Err(e) = crate::aura::channels::start_channels(config).await {
                log::error!("[channels] start_channels ended with error: {e}");
            }
        });
    } else {
        log::info!("[channels] AURA_DISABLE_CHANNEL_LISTENERS set — skipping start_channels");
    }

    if let Some(shutdown_token) = shutdown_token {
        log::info!("[core] embedded server waiting on cancellation token for graceful shutdown");
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_token.cancelled().await;
            })
            .await?;
    } else {
        axum::serve(listener, app)
            .with_graceful_shutdown(crate::core::shutdown::signal())
            .await?;
    }

    // Server has stopped accepting and in-flight requests drained.
    // Kill any `ollama serve` aura itself spawned (no-op when the
    // daemon was externally managed) and clear the spawn marker so the
    // next launch doesn't try to reclaim a daemon that's already dead.
    // Bounded so a wedged Ollama can't hold up app shutdown.
    if let Some(svc) = crate::aura::inference::local::try_global() {
        let cfg = crate::aura::config::Config::load_or_init()
            .await
            .unwrap_or_default();
        log::info!("[core] shutdown: cleaning up aura-owned ollama if any");
        let shutdown_fut = svc.shutdown_owned_ollama(&cfg);
        if tokio::time::timeout(std::time::Duration::from_secs(2), shutdown_fut)
            .await
            .is_err()
        {
            log::warn!("[core] shutdown: ollama cleanup exceeded 2s budget; proceeding with exit");
        }
    }

    Ok(())
}

/// Registers all long-lived domain event-bus subscribers exactly once.
///
/// Guarded by `std::sync::Once` so repeated calls to `bootstrap_core_runtime`
/// are safe and idempotent.
fn register_domain_subscribers(
    workspace_dir: std::path::PathBuf,
    config: crate::aura::config::Config,
    embedded_core: bool,
) {
    use std::sync::{Arc, Once};

    static REGISTERED: Once = Once::new();
    REGISTERED.call_once(|| {
        // Leak the SubscriptionHandle so the background tasks live for the
        // entire process — SubscriptionHandle::drop aborts the task.
        if let Some(handle) = crate::core::event_bus::subscribe_global(Arc::new(
            crate::aura::webhooks::bus::WebhookRequestSubscriber::new(),
        )) {
            std::mem::forget(handle);
        } else {
            log::warn!("[event_bus] failed to register webhook subscriber — bus not initialized");
        }

        if let Some(handle) = crate::core::event_bus::subscribe_global(Arc::new(
            crate::aura::channels::bus::ChannelInboundSubscriber::new(),
        )) {
            std::mem::forget(handle);
        } else {
            log::warn!("[event_bus] failed to register channel subscriber — bus not initialized");
        }

        crate::aura::health::bus::register_health_subscriber();
        crate::aura::notifications::register_notification_bridge_subscriber();
        crate::aura::memory::conversations::register_conversation_persistence_subscriber(
            workspace_dir.clone(),
        );
        if let Err(error) = crate::aura::composio::init_composio_trigger_history(
            workspace_dir.clone(),
        ) {
            log::warn!("[composio][history] failed to initialize trigger archive: {error}");
        }
        crate::aura::composio::register_composio_trigger_subscriber();
        crate::aura::composio::start_periodic_sync();
        // Initialise the scheduler gate before any background AI workers
        // start so they observe a real policy on their first iteration
        // (otherwise they fall back to `Policy::Normal` and miss the
        // initial throttle decision on battery-powered hosts).
        crate::aura::scheduler_gate::init_global(&config);

        // Seed the scheduler-gate signed-out override from the on-disk
        // session. Without this, a sidecar that boots with no stored JWT
        // would happily spin up cron / channel loops and fire LLM requests
        // that all 401 immediately.
        match crate::api::jwt::get_session_token(&config) {
            Ok(Some(_)) => {
                crate::aura::scheduler_gate::set_signed_out(false);
            }
            Ok(None) => {
                log::info!(
                    "[auth] no session token at startup — scheduler gate set to signed_out"
                );
                crate::aura::scheduler_gate::set_signed_out(true);
            }
            Err(err) => {
                log::warn!(
                    "[auth] failed to read session token at startup ({err}) — assuming signed_out"
                );
                crate::aura::scheduler_gate::set_signed_out(true);
            }
        }

        // Register the SessionExpired handler before any subscribers that
        // might publish 401-derived events, so the very first 401 is
        // routed through `clear_session` + the scheduler-gate override.
        if let Some(handle) = crate::core::event_bus::subscribe_global(Arc::new(
            crate::aura::credentials::bus::SessionExpiredSubscriber::new(),
        )) {
            std::mem::forget(handle);
        } else {
            log::warn!(
                "[event_bus] failed to register SessionExpired subscriber — bus not initialized"
            );
        }

        crate::aura::memory::tree::jobs::start(config.clone());

        // Restart requests go through a subscriber so every trigger path shares
        // the same respawn logic.
        crate::aura::service::bus::register_restart_subscriber();
        if embedded_core {
            log::info!(
                "[event_bus] embedded core: service shutdown subscriber not registered; Tauri cancellation token owns shutdown"
            );
        } else {
            // Shutdown requests use the same pattern; the standalone CLI
            // subscriber exits the current process after a short grace period.
            crate::aura::service::bus::register_shutdown_subscriber();
        }

        // Proactive message subscriber (web-only in the desktop runtime —
        // no external channel instances are registered here). Uses a
        // Once-guarded registrar so domain-level startup can't duplicate it.
        crate::aura::channels::proactive::register_web_only_proactive_subscriber();

        // Native request handlers — typed in-process request/response.
        // The agent `agent.run_turn` handler is what channel dispatch
        // calls instead of importing `run_tool_call_loop` directly.
        crate::aura::agent::bus::register_agent_handlers();

        log::info!(
            "[event_bus] domain subscribers registered (webhook, channel, health, conversation, composio, restart, proactive, agent, session_expired)"
        );
    });
}

/// Initializes long-lived socket/event-bus infrastructure.
pub async fn bootstrap_core_runtime(embedded_core: bool) {
    use crate::aura::socket::{set_global_socket_manager, SocketManager};
    use std::sync::Arc;
    let cfg = match crate::aura::config::Config::load_or_init().await {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("[runtime] Failed to load config for socket manager: {e}");
            return;
        }
    };
    let workspace_dir = cfg.workspace_dir.clone();

    // --- Event bus bootstrap ---
    // Ensure the global event bus is initialized (no-op if already done by start_channels).
    crate::core::event_bus::init_global(crate::core::event_bus::DEFAULT_CAPACITY);
    // Register domain subscribers for cross-module event handling.
    // Uses a Once guard so repeated calls to bootstrap_core_runtime()
    // cannot double-subscribe.
    register_domain_subscribers(workspace_dir.clone(), cfg.clone(), embedded_core);

    // --- Turn-state recovery -------------------------------------------
    // Any per-thread turn snapshots left on disk from a previous process
    // are stale by definition — there is no live driver to resume them.
    // Stamp them as `Interrupted` so the UI can offer a retry without
    // confusing a stale `Streaming` lifecycle for an in-flight turn.
    {
        let now = chrono::Utc::now().to_rfc3339();
        match crate::aura::threads::turn_state::store::mark_all_interrupted(
            workspace_dir.clone(),
            &now,
        ) {
            Ok(0) => {}
            Ok(count) => {
                log::info!("[runtime] marked {count} stale turn snapshot(s) as interrupted")
            }
            Err(err) => {
                log::warn!("[runtime] failed to mark stale turn snapshots interrupted: {err}")
            }
        }
    }

    // --- Sub-agent definition registry bootstrap ---
    // Loads built-in archetype definitions plus any custom TOML files
    // under `<workspace>/agents/*.toml`. Idempotent — safe to call
    // multiple times. Uses the per-user scoped workspace_dir.
    if let Err(err) =
        crate::aura::agent::harness::AgentDefinitionRegistry::init_global(&workspace_dir)
    {
        log::warn!(
            "[runtime] AgentDefinitionRegistry::init_global failed: {err} — \
             spawn_subagent will be unavailable until restart"
        );
    }

    // --- Session storage layout migration -------------------------------
    // One-shot move from `session_raw/{DDMMYYYY}/` (≤ 0.53.4) to the new
    // flat `session_raw/{stem}.jsonl` layout, plus DDMMYYYY → YYYY_MM_DD
    // for the human-readable `sessions/` companions. Idempotent via a
    // marker file at `state/migrations/session_layout_v1.done`, so this
    // costs one stat() on every subsequent boot.
    match crate::aura::agent::harness::session::migrate_session_layout_if_needed(&workspace_dir) {
        Ok(outcome) if outcome.already_done => {
            log::debug!("[runtime] session_layout migration already applied");
        }
        Ok(outcome) => {
            log::info!(
                "[runtime] session_layout migration applied: jsonl_moved={} md_moved={} pruned_dirs={} warnings={}",
                outcome.jsonl_moved,
                outcome.md_moved,
                outcome.legacy_dirs_pruned,
                outcome.warnings.len(),
            );
            for w in &outcome.warnings {
                log::warn!("[runtime] session_layout migration warning: {w}");
            }
        }
        Err(err) => {
            // Don't bring down startup over a transcript-storage migration.
            // The transcript module's legacy fallback covers the unmigrated
            // case for one release window.
            log::warn!(
                "[runtime] session_layout migration failed: {err} — \
                 falling back to in-place legacy reads"
            );
        }
    }

    // --- Socket manager bootstrap ---
    let socket_mgr = Arc::new(SocketManager::new());
    set_global_socket_manager(socket_mgr.clone());
    log::info!("[socket] SocketManager initialized and registered globally");

    // Auto-connect socket to backend if a session token is already stored.
    // This runs in the background so it doesn't block server startup.
    tokio::spawn(async move {
        log::info!("[socket] Checking for stored session to auto-connect...");
        let config = match crate::aura::config::Config::load_or_init().await {
            Ok(c) => c,
            Err(e) => {
                log::debug!("[socket] Config not available for auto-connect: {e}");
                return;
            }
        };
        let api_url = crate::api::config::effective_backend_api_url(&config.api_url);
        let token = match crate::api::jwt::get_session_token(&config) {
            Ok(Some(t)) => t,
            Ok(None) => {
                log::info!("[socket] No session token stored — skipping auto-connect (will connect after login)");
                return;
            }
            Err(e) => {
                log::warn!("[socket] Failed to read session token: {e}");
                return;
            }
        };
        log::info!(
            "[socket] Session token found — auto-connecting to {}",
            api_url
        );
        if let Err(e) = socket_mgr.connect(&api_url, &token).await {
            log::error!("[socket] Auto-connect failed: {e}");
        } else {
            log::info!("[socket] Auto-connect initiated successfully");
        }
    });
}
