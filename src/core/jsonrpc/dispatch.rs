//! JSON-RPC method dispatch: the HTTP handler, method invocation tiers,
//! and the small parameter-shape / error-classification helpers they rely on.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Map, Value};

use crate::core::all;
use crate::core::types::{AppState, RpcError, RpcFailure, RpcRequest, RpcSuccess};
use crate::rpc::StructuredRpcError;

/// Axum handler for JSON-RPC POST requests.
///
/// This function:
/// 1. Receives a JSON-RPC request body.
/// 2. Extracts the method name and parameters.
/// 3. Invokes the corresponding handler via [`invoke_method`].
/// 4. Wraps the result or error in a JSON-RPC 2.0 compliant response.
///
/// # Arguments
///
/// * `state` - The application state, injected by Axum.
/// * `req` - The parsed [`RpcRequest`].
pub(super) async fn rpc_handler(
    State(state): State<AppState>,
    Json(req): Json<RpcRequest>,
) -> Response {
    let id = req.id.clone();
    let method = req.method.clone();
    let started = std::time::Instant::now();
    let result = invoke_method(state, method.as_str(), req.params).await;
    let ms = started.elapsed().as_millis();

    match result {
        Ok(value) => {
            tracing::info!("[rpc] {} -> ok ({}ms)", method, ms);
            (
                StatusCode::OK,
                Json(RpcSuccess {
                    jsonrpc: "2.0",
                    id,
                    result: value,
                }),
            )
                .into_response()
        }
        Err(raw_message) => {
            // Decode the controller-emitted structured envelope (if any)
            // here at the transport boundary. Domains opt in by emitting a
            // `StructuredRpcError` from their handlers — this layer never
            // branches on the RPC method name to recover error semantics.
            let structured = StructuredRpcError::decode(&raw_message);
            let (display_message, error_data, expected_user_state) = match structured {
                Some(envelope) => (
                    envelope.message,
                    envelope.data,
                    envelope.expected_user_state,
                ),
                None => (raw_message, None, false),
            };

            // Session-expired bubbles up as an "error" but is an expected
            // boundary condition (auth handler clears the local token and the
            // UI re-auths). Don't spam Sentry with it.
            //
            // Param-validation failures ("unknown param 'x' for ns.fn",
            // "missing required param 'x'", "invalid params: …") are also
            // pure boundary mismatches: either the caller is a frontend on a
            // different release than the running core (OPENHUMAN-TAURI-20:
            // v0.53.22 UI shipped `api_key` before the matching schema input
            // landed in #1467) or it is straight client-bug input. Sentry
            // cannot help — we can neither retro-fix already-shipped
            // installs nor learn anything from the noise — so log at info
            // and skip the report.
            //
            // Logging asymmetry between the two skip paths is intentional:
            // session-expired messages are a small set of fixed strings
            // (no caller-supplied content), so the full text is safe to
            // log. Param-validation messages embed caller-supplied param
            // names and, for the `invalid params: …` shape, can carry
            // deserialized values — log structurally with redacted body
            // to keep PII out of the sink while preserving the method
            // for grep / correlation.
            //
            // Domains that surface their own expected-user-state errors
            // (stale thread refs, etc.) set the `expected_user_state` flag
            // on their structured envelope and skip Sentry here uniformly.
            if expected_user_state {
                tracing::info!(
                    method = %method,
                    "[rpc] expected-user-state error — skipping Sentry: {}",
                    display_message
                );
            } else if is_param_validation_error(&display_message) {
                tracing::info!(
                    method = %method,
                    elapsed_ms = ms as u64,
                    "[rpc] param-validation error (message redacted; skip-report)"
                );
            } else if is_session_expired_error(&display_message) {
                tracing::info!("[rpc] {} -> err ({}ms): {}", method, ms, display_message);
            } else if crate::core::observability::is_transient_message_failure(&display_message) {
                // Downstream call (backend_api / integrations / provider) already
                // demoted the underlying transient failure to a warn. The error
                // string still propagates up to here; re-reporting at error level
                // would re-create the very Sentry noise the lower-layer demote
                // was meant to avoid (#8Z, #93, #8W, #96).
                //
                // Redact before logging — `display_message` is upstream-derived
                // (backend / provider response) and can carry URL fragments,
                // query params, or pasted-through provider error text that
                // includes tokens. `sanitize_api_error` runs the same scrub
                // used in the SessionExpired publish path below.
                let redacted = crate::openhuman::inference::provider::ops::sanitize_api_error(
                    &display_message,
                );
                tracing::warn!(
                    method = %method,
                    elapsed_ms = ms as u64,
                    error = %redacted,
                    "[rpc] transient downstream failure — not reporting to Sentry (message redacted)"
                );
            } else {
                crate::core::observability::report_error_or_expected(
                    display_message.as_str(),
                    "rpc",
                    "invoke_method",
                    &[("method", method.as_str()), ("elapsed_ms", &ms.to_string())],
                );
            }
            (
                StatusCode::OK,
                Json(RpcFailure {
                    jsonrpc: "2.0",
                    id,
                    error: RpcError {
                        code: -32000,
                        message: display_message,
                        data: error_data,
                    },
                }),
            )
                .into_response()
        }
    }
}

/// Invokes a JSON-RPC method by name.
///
/// This is a high-level wrapper around [`invoke_method_inner`] that adds
/// automatic session management logic. If a call fails with a 401 Unauthorized
/// error from the backend, it will automatically clear the local session.
///
/// # Arguments
///
/// * `state` - The application state.
/// * `method` - The name of the method to invoke.
/// * `params` - The JSON parameters for the method.
pub async fn invoke_method(state: AppState, method: &str, params: Value) -> Result<Value, String> {
    let result = invoke_method_inner(state, method, params).await;

    // Session auto-cleanup: if the backend says we're unauthorized, publish
    // a `SessionExpired` event. The credentials subscriber clears the stored
    // token, flips the scheduler-gate signed-out override so background
    // workers stand down, and (eventually) pushes a sign-out to the UI.
    // Centralising via the event bus means 401 detection from any path
    // (this one, `llm_provider.api_error`, …) gets the same teardown.
    if let Err(ref msg) = result {
        if is_session_expired_error(msg) {
            log::warn!(
                "[jsonrpc] backend returned 401 for method '{}' — publishing SessionExpired",
                method
            );
            // Scrub before publishing — subscribers log `reason`, and the
            // upstream error string could include API keys / tokens from
            // pasted-through provider replies. `sanitize_api_error` runs
            // `scrub_secret_patterns` and truncates.
            crate::core::event_bus::publish_global(
                crate::core::event_bus::DomainEvent::SessionExpired {
                    source: format!("jsonrpc.invoke_method:{method}"),
                    reason: crate::openhuman::inference::provider::ops::sanitize_api_error(msg),
                },
            );
        }
    }

    result
}

/// Helper to determine if an error message indicates an expired or invalid session.
///
/// Deliberately **looser** than
/// [`crate::core::observability::is_session_expired_message`]: this
/// dispatch-site predicate also matches the generic `"401 + unauthorized"` /
/// `"invalid token"` pair so token cleanup +
/// `DomainEvent::SessionExpired` publish fire on *any* 401, including
/// BYO-key provider failures (which clear the stale local token even if
/// the user mis-configured an OpenAI / Anthropic key). The strict
/// classifier in `observability` is for the agent / web-channel
/// `report_error_or_expected` call sites, where matching too loosely would
/// silence actionable BYO-key configuration errors (OPENHUMAN-TAURI-26
/// rationale: the agent-layer demote must NOT also swallow generic
/// provider 401s).
///
/// "No backend session token" is also treated as a session-expired signal: the
/// auth profile is missing entirely (the user was never signed in, or their
/// stored profile was wiped between login and the next RPC). The frontend may
/// still believe it holds a session token from an optimistic post-login patch,
/// so we want the same auto-cleanup + UI-level re-auth path to fire instead of
/// repeatedly reporting this as a hard error to Sentry. See #1465-ish: users
/// stuck on the onboarding `SkillsStep` would spam `composio_list_connections`
/// failures every 5 s without ever being bounced back to the login screen.
///
/// "session JWT required" covers the case where a prior 401 already cleared the
/// token and the very next RPC call (e.g. `channels_telegram_login_start`) finds
/// no JWT in the store. This is the same auth-boundary condition, just surfaced
/// as a local guard rather than a backend response.
pub(super) fn is_session_expired_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    (lower.contains("401") && lower.contains("unauthorized"))
        || lower.contains("invalid token")
        || lower.contains("no backend session token")
        || lower.contains("session jwt required")
        || msg.contains("SESSION_EXPIRED")
}

/// Returns `true` when the error message comes from JSON-RPC params validation
/// rather than the underlying handler.
///
/// Three shapes, all emitted before the handler ever runs:
///   * `"unknown param '<key>' for <ns>.<fn>"`       — `all::validate_params` (extra field)
///   * `"missing required param '<key>': <comment>"` — `all::validate_params` (omitted required field)
///   * `"invalid params: expected object or null, got <type>"` — `params_to_object` (wrong params shape)
///
/// These only fire when caller and server schemas drift at the transport layer
/// — either a frontend on a different release than the running core, or a buggy
/// external client. Reporting them to Sentry produces unactionable noise (we
/// cannot patch an already-shipped install, and the message itself already
/// names the bad field).
///
/// Note: domain-level validation errors (e.g. type/format checks emitted *inside*
/// a controller's `rpc.rs` handler such as `"param 'x' must be a UUID"`) are
/// intentionally *not* matched here — only the three shapes emitted by the
/// transport-layer validators before the handler runs. Longer-term a typed
/// `RpcError::ParamValidation` variant would remove the string-matching
/// brittleness; the unit tests in `jsonrpc_tests.rs` lock the exact prefixes
/// against the emit sites in `all::validate_params` and `params_to_object`.
///
/// `starts_with` (not `.contains()`) is deliberate: validator errors are always
/// emitted as the full message body, so an anchored match avoids false positives
/// from upstream handler text that happens to mention `"unknown param"`. The
/// session-expired predicate uses `.contains()` because session-expired markers
/// can appear mid-message — flip these to match and the test
/// `is_param_validation_error_does_not_match_unrelated_errors` will break.
pub(super) fn is_param_validation_error(msg: &str) -> bool {
    msg.starts_with("unknown param '")
        || msg.starts_with("missing required param '")
        || msg.starts_with("invalid params: ")
}

/// Internal method invocation logic.
///
/// It first attempts to match the method name against the static controller
/// registry (schemas). If a schema is found, it validates the input parameters
/// before execution. If no schema matches, it falls back to the dynamic
/// [`crate::core::dispatch::dispatch`] system.
async fn invoke_method_inner(
    state: AppState,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    // Phase 1: Check static controller registry.
    if let Some(schema) = all::schema_for_rpc_method(method) {
        let params_obj = params_to_object(params.clone())?;
        // Validate inputs against the schema before calling the handler.
        all::validate_params(&schema, &params_obj)?;
        if let Some(result) = all::try_invoke_registered_rpc(method, params_obj).await {
            return result;
        }
        log::debug!(
            "[jsonrpc] schema matched without registered handler; falling back method={}",
            method
        );
    }

    // Phase 2: Fall back to dynamic dispatch (internal core methods or legacy paths).
    crate::core::dispatch::dispatch(state, method, params).await
}

/// Converts JSON parameters into a map, ensuring they are in object format.
///
/// JSON-RPC allows parameters to be an Object, an Array, or Null. This implementation
/// primarily supports Object parameters for named-argument style calls.
pub(super) fn params_to_object(params: Value) -> Result<Map<String, Value>, String> {
    match params {
        Value::Object(map) => Ok(map),
        Value::Null => Ok(Map::new()),
        other => Err(format!(
            "invalid params: expected object or null, got {}",
            type_name(&other)
        )),
    }
}

/// Returns a human-readable string representation of a JSON value's type.
pub(super) fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Parses a JSON string into a `Value`.
pub fn parse_json_params(raw: &str) -> Result<Value, String> {
    serde_json::from_str(raw).map_err(|e| format!("invalid JSON params: {e}"))
}

/// Returns the default application state.
pub fn default_state() -> AppState {
    AppState {
        core_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}
