//! WebSocket upgrade handler for streaming voice dictation.

use std::sync::Arc;

use axum::extract::WebSocketUpgrade;
use axum::response::Response;

/// WebSocket upgrade handler for streaming voice dictation.
pub(super) async fn dictation_ws_handler(ws: WebSocketUpgrade) -> Response {
    log::info!("[ws] dictation WebSocket upgrade requested");
    ws.on_upgrade(|socket| async move {
        let config = match crate::openhuman::config::rpc::load_config_with_timeout().await {
            Ok(c) => Arc::new(c),
            Err(e) => {
                log::error!("[ws] failed to load config for dictation: {e}");
                return;
            }
        };
        crate::openhuman::voice::streaming::handle_dictation_ws(socket, config).await;
    })
}
