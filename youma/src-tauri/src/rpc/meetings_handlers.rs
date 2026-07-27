// rpc/meetings_handlers.rs — Meeting record handlers.

use crate::meetings;
use serde_json::Value;

pub async fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "meetings.list"              => meetings::handle_list(),
        "meetings.get"               => meetings::handle_get(params),
        "meetings.delete"            => meetings::handle_delete(params),
        "meetings.search"            => meetings::handle_search(params),
        "meetings.save"              => meetings::handle_save(params).await,
        "meetings.update"            => meetings::handle_update(params),
        "meetings.export"            => meetings::handle_export(params),
        "meetings.update_body"       => meetings::handle_update_body(params),
        "meetings.calendar_hint"     => meetings::handle_calendar_hint().await,
        "meetings.share"             => meetings::handle_share(params).await,
        "meetings.transcribe_diarize" => meetings::handle_transcribe_diarize(params).await,
        _ => Err(format!("[rpc::meetings] unknown method: {method}")),
    }
}
