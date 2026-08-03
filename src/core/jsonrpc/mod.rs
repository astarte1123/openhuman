//! JSON-RPC 2.0 server implementation for OpenHuman.
//!
//! This module provides:
//! - An Axum-based HTTP server for handling JSON-RPC requests.
//! - Method dispatching to registered controllers.
//! - SSE (Server-Sent Events) for real-time event streaming.
//! - Helper routes for health checks, schema discovery, and Telegram authentication.

mod dictation_ws;
mod dispatch;
mod router;
mod schema_dump;
mod server;
mod telegram_auth;

pub use dispatch::{default_state, invoke_method, parse_json_params};
pub use router::build_core_http_router;
pub use server::{bootstrap_core_runtime, run_server, run_server_embedded};

// Re-bound here (not re-exported beyond this module tree) purely so the
// `tests` submodule below can reach them via `super::*` exactly as it did
// when everything lived in a single `jsonrpc.rs` file.
#[cfg(test)]
use crate::core::types::AppState;
#[cfg(test)]
use dispatch::{
    is_param_validation_error, is_session_expired_error, params_to_object, rpc_handler, type_name,
};
#[cfg(test)]
use schema_dump::build_http_schema_dump;
#[cfg(test)]
use telegram_auth::escape_html;

#[cfg(test)]
mod tests;
