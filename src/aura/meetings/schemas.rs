//! Controller schema declarations + RPC handlers for the `meetings` namespace.
//!
//! Follows `cron::schemas` exactly: [`all_controller_schemas`] /
//! [`all_registered_controllers`] feed the global controller registry
//! (`src/core/all.rs`), [`schemas`] is the exhaustive per-function schema
//! lookup, and each `handle_*` loads [`Config`](crate::aura::config::Config)
//! via `config_rpc::load_config_with_timeout()`, deserializes params, calls
//! the matching `ops::meetings_*`, and serializes the outcome via [`to_json`].

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::core::all::{ControllerFuture, RegisteredController};
use crate::core::{ControllerSchema, FieldSchema, TypeSchema};
use crate::aura::config::rpc as config_rpc;
use crate::aura::meetings::ops;
use crate::rpc::RpcOutcome;

fn meeting_id_input(comment: &'static str) -> FieldSchema {
    FieldSchema {
        name: "meeting_id",
        ty: TypeSchema::String,
        comment,
        required: true,
    }
}

pub fn all_controller_schemas() -> Vec<ControllerSchema> {
    #[allow(unused_mut)]
    let mut schemas = vec![
        schemas("create"),
        schemas("list"),
        schemas("get"),
        schemas("update_title"),
        schemas("delete"),
        schemas("append_transcript_chunk"),
        schemas("append_screenshot"),
        schemas("finish"),
        schemas("summarize"),
        schemas("search"),
        schemas("export"),
        schemas("start_local_capture"),
        schemas("stop_local_capture"),
    ];
    // Off by default: only registered at all when the `diarize` Cargo
    // feature is compiled in — see `Cargo.toml`'s `[features]` section and
    // `crate::aura::meetings::diarize`'s module doc.
    #[cfg(feature = "diarize")]
    schemas.push(self::schemas("diarize"));
    schemas
}

pub fn all_registered_controllers() -> Vec<RegisteredController> {
    #[allow(unused_mut)]
    let mut controllers = vec![
        RegisteredController {
            schema: schemas("create"),
            handler: handle_create,
        },
        RegisteredController {
            schema: schemas("list"),
            handler: handle_list,
        },
        RegisteredController {
            schema: schemas("get"),
            handler: handle_get,
        },
        RegisteredController {
            schema: schemas("update_title"),
            handler: handle_update_title,
        },
        RegisteredController {
            schema: schemas("delete"),
            handler: handle_delete,
        },
        RegisteredController {
            schema: schemas("append_transcript_chunk"),
            handler: handle_append_transcript_chunk,
        },
        RegisteredController {
            schema: schemas("append_screenshot"),
            handler: handle_append_screenshot,
        },
        RegisteredController {
            schema: schemas("finish"),
            handler: handle_finish,
        },
        RegisteredController {
            schema: schemas("summarize"),
            handler: handle_summarize,
        },
        RegisteredController {
            schema: schemas("search"),
            handler: handle_search,
        },
        RegisteredController {
            schema: schemas("export"),
            handler: handle_export,
        },
        RegisteredController {
            schema: schemas("start_local_capture"),
            handler: handle_start_local_capture,
        },
        RegisteredController {
            schema: schemas("stop_local_capture"),
            handler: handle_stop_local_capture,
        },
    ];
    #[cfg(feature = "diarize")]
    controllers.push(RegisteredController {
        schema: schemas("diarize"),
        handler: handle_diarize,
    });
    controllers
}

pub fn schemas(function: &str) -> ControllerSchema {
    match function {
        "create" => ControllerSchema {
            namespace: "meetings",
            function: "create",
            description: "Create a new meeting recording session.",
            inputs: vec![
                FieldSchema {
                    name: "title",
                    ty: TypeSchema::Option(Box::new(TypeSchema::String)),
                    comment: "Meeting title; defaults to 'Untitled meeting'.",
                    required: false,
                },
                FieldSchema {
                    name: "source",
                    ty: TypeSchema::Option(Box::new(TypeSchema::String)),
                    comment: "Origin of the meeting recording; defaults to 'manual'.",
                    required: false,
                },
            ],
            outputs: vec![FieldSchema {
                name: "meeting",
                ty: TypeSchema::Ref("Meeting"),
                comment: "Newly created meeting, in 'recording' status.",
                required: true,
            }],
        },
        "list" => ControllerSchema {
            namespace: "meetings",
            function: "list",
            description: "List all meetings ordered by creation time (most recent first).",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "meetings",
                ty: TypeSchema::Array(Box::new(TypeSchema::Ref("Meeting"))),
                comment: "Meetings currently stored in the workspace.",
                required: true,
            }],
        },
        "get" => ControllerSchema {
            namespace: "meetings",
            function: "get",
            description: "Fetch a meeting along with its ordered transcript chunks.",
            inputs: vec![meeting_id_input("Identifier of the meeting to fetch.")],
            outputs: vec![
                FieldSchema {
                    name: "meeting",
                    ty: TypeSchema::Ref("Meeting"),
                    comment: "The requested meeting.",
                    required: true,
                },
                FieldSchema {
                    name: "chunks",
                    ty: TypeSchema::Array(Box::new(TypeSchema::Ref("TranscriptChunk"))),
                    comment: "Transcript chunks ordered by start time ascending.",
                    required: true,
                },
                FieldSchema {
                    name: "screenshots",
                    ty: TypeSchema::Array(Box::new(TypeSchema::Ref("MeetingScreenshot"))),
                    comment: "Captured slide screenshots ordered by capture time ascending.",
                    required: true,
                },
            ],
        },
        "update_title" => ControllerSchema {
            namespace: "meetings",
            function: "update_title",
            description: "Rename a meeting.",
            inputs: vec![
                meeting_id_input("Identifier of the meeting to rename."),
                FieldSchema {
                    name: "title",
                    ty: TypeSchema::String,
                    comment: "New meeting title.",
                    required: true,
                },
            ],
            outputs: vec![FieldSchema {
                name: "meeting",
                ty: TypeSchema::Ref("Meeting"),
                comment: "Updated meeting.",
                required: true,
            }],
        },
        "delete" => ControllerSchema {
            namespace: "meetings",
            function: "delete",
            description: "Delete a meeting and its transcript chunks (cascade).",
            inputs: vec![meeting_id_input("Identifier of the meeting to delete.")],
            outputs: vec![FieldSchema {
                name: "result",
                ty: TypeSchema::Object {
                    fields: vec![FieldSchema {
                        name: "deleted",
                        ty: TypeSchema::Bool,
                        comment: "True when a meeting row was removed.",
                        required: true,
                    }],
                },
                comment: "Deletion result payload.",
                required: true,
            }],
        },
        "append_transcript_chunk" => ControllerSchema {
            namespace: "meetings",
            function: "append_transcript_chunk",
            description: "Append one transcript chunk to a recording meeting.",
            inputs: vec![
                meeting_id_input("Identifier of the meeting being recorded."),
                FieldSchema {
                    name: "start_ms",
                    ty: TypeSchema::I64,
                    comment: "Chunk start offset in milliseconds.",
                    required: true,
                },
                FieldSchema {
                    name: "end_ms",
                    ty: TypeSchema::I64,
                    comment: "Chunk end offset in milliseconds.",
                    required: true,
                },
                FieldSchema {
                    name: "text",
                    ty: TypeSchema::String,
                    comment: "Transcribed text for this chunk.",
                    required: true,
                },
                FieldSchema {
                    name: "speaker",
                    ty: TypeSchema::Option(Box::new(TypeSchema::String)),
                    comment: "Speaker label, if known.",
                    required: false,
                },
            ],
            outputs: vec![FieldSchema {
                name: "chunk",
                ty: TypeSchema::Ref("TranscriptChunk"),
                comment: "Persisted transcript chunk with its assigned id.",
                required: true,
            }],
        },
        "append_screenshot" => ControllerSchema {
            namespace: "meetings",
            function: "append_screenshot",
            description:
                "Append one OCR'd slide screenshot to a recording meeting, from a base64-encoded PNG.",
            inputs: vec![
                meeting_id_input("Identifier of the meeting being recorded."),
                FieldSchema {
                    name: "captured_at_ms",
                    ty: TypeSchema::I64,
                    comment: "Capture offset in milliseconds, since the meeting started.",
                    required: true,
                },
                FieldSchema {
                    name: "image_base64",
                    ty: TypeSchema::String,
                    comment: "Base64-encoded PNG screenshot of the meeting slide.",
                    required: true,
                },
            ],
            outputs: vec![FieldSchema {
                name: "screenshot",
                ty: TypeSchema::Ref("MeetingScreenshot"),
                comment: "Persisted screenshot with its OCR'd text and assigned id.",
                required: true,
            }],
        },
        "finish" => ControllerSchema {
            namespace: "meetings",
            function: "finish",
            description: "Mark a recording meeting as completed.",
            inputs: vec![
                meeting_id_input("Identifier of the meeting to finish."),
                FieldSchema {
                    name: "duration_ms",
                    ty: TypeSchema::I64,
                    comment: "Total recorded duration in milliseconds.",
                    required: true,
                },
            ],
            outputs: vec![FieldSchema {
                name: "meeting",
                ty: TypeSchema::Ref("Meeting"),
                comment: "Completed meeting.",
                required: true,
            }],
        },
        "summarize" => ControllerSchema {
            namespace: "meetings",
            function: "summarize",
            description:
                "Summarize a meeting's transcript via the configured LLM and persist the result.",
            inputs: vec![meeting_id_input("Identifier of the meeting to summarize.")],
            outputs: vec![FieldSchema {
                name: "result",
                ty: TypeSchema::Object {
                    fields: vec![FieldSchema {
                        name: "summary",
                        ty: TypeSchema::String,
                        comment: "Generated meeting summary.",
                        required: true,
                    }],
                },
                comment: "Summarization result payload.",
                required: true,
            }],
        },
        "search" => ControllerSchema {
            namespace: "meetings",
            function: "search",
            description: "Full-text search across every meeting transcript.",
            inputs: vec![FieldSchema {
                name: "query",
                ty: TypeSchema::String,
                comment:
                    "Search text; supports FTS5 query syntax (AND/OR/NOT, \"phrases\", prefix*).",
                required: true,
            }],
            outputs: vec![FieldSchema {
                name: "results",
                ty: TypeSchema::Array(Box::new(TypeSchema::Object {
                    fields: vec![
                        FieldSchema {
                            name: "meeting_id",
                            ty: TypeSchema::String,
                            comment: "Identifier of the meeting the match belongs to.",
                            required: true,
                        },
                        FieldSchema {
                            name: "meeting_title",
                            ty: TypeSchema::String,
                            comment: "Title of that meeting.",
                            required: true,
                        },
                        FieldSchema {
                            name: "chunk_id",
                            ty: TypeSchema::I64,
                            comment: "Identifier of the matched transcript chunk.",
                            required: true,
                        },
                        FieldSchema {
                            name: "start_ms",
                            ty: TypeSchema::I64,
                            comment: "Start offset of the matched chunk in milliseconds.",
                            required: true,
                        },
                        FieldSchema {
                            name: "snippet",
                            ty: TypeSchema::String,
                            comment: "Matched text excerpt with <mark></mark> around hits.",
                            required: true,
                        },
                    ],
                })),
                comment: "Search hits ordered by relevance, most relevant first.",
                required: true,
            }],
        },
        "export" => ControllerSchema {
            namespace: "meetings",
            function: "export",
            description:
                "Render a meeting's transcript and summary to PDF or DOCX and write it under the workspace, returning the file's path.",
            inputs: vec![
                meeting_id_input("Identifier of the meeting to export."),
                FieldSchema {
                    name: "format",
                    ty: TypeSchema::String,
                    comment: "Export file format: 'pdf' or 'docx'.",
                    required: true,
                },
            ],
            outputs: vec![FieldSchema {
                name: "result",
                ty: TypeSchema::Object {
                    fields: vec![FieldSchema {
                        name: "path",
                        ty: TypeSchema::String,
                        comment: "Absolute path of the written export file.",
                        required: true,
                    }],
                },
                comment: "Export result payload.",
                required: true,
            }],
        },
        "start_local_capture" => ControllerSchema {
            namespace: "meetings",
            function: "start_local_capture",
            description:
                "Start local live audio capture (microphone + best-effort system-audio loopback) for a recording meeting, streaming transcript chunks in as they're transcribed. Also starts periodic slide-screenshot capture + OCR for the same session unless 'capture_screenshots' is false.",
            inputs: vec![
                meeting_id_input("Identifier of the recording meeting to capture audio for."),
                FieldSchema {
                    name: "capture_screenshots",
                    ty: TypeSchema::Option(Box::new(TypeSchema::Bool)),
                    comment:
                        "Whether to also periodically capture + OCR slide screenshots for this session; defaults to true.",
                    required: false,
                },
            ],
            outputs: vec![FieldSchema {
                name: "result",
                ty: TypeSchema::Object {
                    fields: vec![FieldSchema {
                        name: "capturing",
                        ty: TypeSchema::Bool,
                        comment: "True once the capture backend has started.",
                        required: true,
                    }],
                },
                comment: "Capture start result payload.",
                required: true,
            }],
        },
        "stop_local_capture" => ControllerSchema {
            namespace: "meetings",
            function: "stop_local_capture",
            description: "Stop an in-flight local audio capture session for a meeting, if one is running.",
            inputs: vec![meeting_id_input(
                "Identifier of the meeting whose local capture should stop.",
            )],
            outputs: vec![FieldSchema {
                name: "result",
                ty: TypeSchema::Object {
                    fields: vec![FieldSchema {
                        name: "stopped",
                        ty: TypeSchema::Bool,
                        comment: "True when an active capture session was found and stopped; false when none was running.",
                        required: true,
                    }],
                },
                comment: "Capture stop result payload.",
                required: true,
            }],
        },
        #[cfg(feature = "diarize")]
        "diarize" => ControllerSchema {
            namespace: "meetings",
            function: "diarize",
            description:
                "Run real per-speaker diarization over a completed meeting's recorded system-audio \
                 track, splitting the 'system_audio' speaker label into 'system_audio:speaker_N' \
                 per distinct remote voice. Requires the optional 'diarize' build feature; only \
                 available for meetings recorded while that feature was compiled in.",
            inputs: vec![meeting_id_input(
                "Identifier of the completed meeting to diarize.",
            )],
            outputs: vec![
                FieldSchema {
                    name: "speakers_found",
                    ty: TypeSchema::U64,
                    comment: "Number of distinct speakers identified in the system-audio track.",
                    required: true,
                },
                FieldSchema {
                    name: "chunks_updated",
                    ty: TypeSchema::U64,
                    comment: "Number of transcript chunks whose speaker label was rewritten.",
                    required: true,
                },
            ],
        },
        _other => ControllerSchema {
            namespace: "meetings",
            function: "unknown",
            description: "Unknown meetings controller function.",
            inputs: vec![FieldSchema {
                name: "function",
                ty: TypeSchema::String,
                comment: "Unknown function requested for schema lookup.",
                required: true,
            }],
            outputs: vec![FieldSchema {
                name: "error",
                ty: TypeSchema::String,
                comment: "Lookup error details.",
                required: true,
            }],
        },
    }
}

fn handle_create(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let title = read_optional::<String>(&params, "title")?;
        let source = read_optional::<String>(&params, "source")?;
        to_json(ops::meetings_create(&config, title, source).await?)
    })
}

fn handle_list(_params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async {
        let config = config_rpc::load_config_with_timeout().await?;
        to_json(ops::meetings_list(&config).await?)
    })
}

fn handle_get(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        to_json(ops::meetings_get(&config, meeting_id.trim()).await?)
    })
}

fn handle_update_title(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        let title = read_required::<String>(&params, "title")?;
        to_json(ops::meetings_update_title(&config, meeting_id.trim(), &title).await?)
    })
}

fn handle_delete(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        to_json(ops::meetings_delete(&config, meeting_id.trim()).await?)
    })
}

fn handle_append_transcript_chunk(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        let start_ms = read_required::<i64>(&params, "start_ms")?;
        let end_ms = read_required::<i64>(&params, "end_ms")?;
        let text = read_required::<String>(&params, "text")?;
        let speaker = read_optional::<String>(&params, "speaker")?;
        to_json(
            ops::meetings_append_transcript_chunk(
                &config,
                meeting_id.trim(),
                start_ms,
                end_ms,
                &text,
                speaker,
            )
            .await?,
        )
    })
}

fn handle_append_screenshot(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        let captured_at_ms = read_required::<i64>(&params, "captured_at_ms")?;
        let image_base64 = read_required::<String>(&params, "image_base64")?;
        to_json(
            ops::meetings_append_screenshot(
                &config,
                meeting_id.trim(),
                captured_at_ms,
                &image_base64,
            )
            .await?,
        )
    })
}

fn handle_finish(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        let duration_ms = read_required::<i64>(&params, "duration_ms")?;
        to_json(ops::meetings_finish(&config, meeting_id.trim(), duration_ms).await?)
    })
}

fn handle_summarize(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        to_json(ops::meetings_summarize(&config, meeting_id.trim()).await?)
    })
}

fn handle_search(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let query = read_required::<String>(&params, "query")?;
        to_json(ops::meetings_search(&config, query).await?)
    })
}

fn handle_export(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        let format = read_required::<String>(&params, "format")?;
        to_json(ops::meetings_export(&config, meeting_id.trim(), &format).await?)
    })
}

fn handle_start_local_capture(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        let capture_screenshots = read_optional::<bool>(&params, "capture_screenshots")?;
        to_json(
            ops::meetings_start_local_capture(&config, meeting_id.trim(), capture_screenshots)
                .await?,
        )
    })
}

fn handle_stop_local_capture(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        to_json(ops::meetings_stop_local_capture(&config, meeting_id.trim()).await?)
    })
}

#[cfg(feature = "diarize")]
fn handle_diarize(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let config = config_rpc::load_config_with_timeout().await?;
        let meeting_id = read_required::<String>(&params, "meeting_id")?;
        to_json(ops::meetings_diarize(&config, meeting_id.trim()).await?)
    })
}

fn read_required<T: DeserializeOwned>(params: &Map<String, Value>, key: &str) -> Result<T, String> {
    let value = params
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing required param '{key}'"))?;
    serde_json::from_value(value).map_err(|e| format!("invalid '{key}': {e}"))
}

fn read_optional<T: DeserializeOwned>(
    params: &Map<String, Value>,
    key: &str,
) -> Result<Option<T>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|e| format!("invalid '{key}': {e}")),
    }
}

fn to_json<T: serde::Serialize>(outcome: RpcOutcome<T>) -> Result<Value, String> {
    outcome.into_cli_compatible_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Base function list shared by both schema-coverage tests below.
    /// `#[cfg(feature = "diarize")]` appends `"diarize"` when that optional
    /// feature is compiled in, so these tests stay accurate under both the
    /// default build and `cargo test --features diarize`.
    fn expected_function_names() -> Vec<&'static str> {
        #[allow(unused_mut)]
        let mut names = vec![
            "create",
            "list",
            "get",
            "update_title",
            "delete",
            "append_transcript_chunk",
            "append_screenshot",
            "finish",
            "summarize",
            "search",
            "export",
            "start_local_capture",
            "stop_local_capture",
        ];
        #[cfg(feature = "diarize")]
        names.push("diarize");
        names
    }

    #[test]
    fn all_controller_schemas_covers_every_supported_function() {
        let names: Vec<_> = all_controller_schemas()
            .into_iter()
            .map(|s| s.function)
            .collect();
        assert_eq!(names, expected_function_names());
    }

    #[test]
    fn all_registered_controllers_has_handler_per_schema() {
        let controllers = all_registered_controllers();
        assert_eq!(controllers.len(), expected_function_names().len());
        let names: Vec<_> = controllers.iter().map(|c| c.schema.function).collect();
        assert_eq!(names, expected_function_names());
    }

    #[test]
    fn schemas_create_inputs_are_optional() {
        let s = schemas("create");
        assert!(s.inputs.iter().all(|f| !f.required));
    }

    #[test]
    fn schemas_get_outputs_include_meeting_and_chunks() {
        let s = schemas("get");
        let names: Vec<_> = s.outputs.iter().map(|f| f.name).collect();
        assert!(names.contains(&"meeting"));
        assert!(names.contains(&"chunks"));
        assert!(names.contains(&"screenshots"));
    }

    #[test]
    fn schemas_append_screenshot_inputs_are_all_required() {
        let s = schemas("append_screenshot");
        assert!(s.inputs.iter().all(|f| f.required));
        let names: Vec<_> = s.inputs.iter().map(|f| f.name).collect();
        assert!(names.contains(&"meeting_id"));
        assert!(names.contains(&"captured_at_ms"));
        assert!(names.contains(&"image_base64"));
    }

    #[test]
    fn schemas_append_transcript_chunk_speaker_is_optional() {
        let s = schemas("append_transcript_chunk");
        let speaker = s.inputs.iter().find(|f| f.name == "speaker").unwrap();
        assert!(!speaker.required);
    }

    #[test]
    fn schemas_export_format_and_meeting_id_are_required() {
        let s = schemas("export");
        assert!(s.inputs.iter().all(|f| f.required));
        let names: Vec<_> = s.inputs.iter().map(|f| f.name).collect();
        assert!(names.contains(&"meeting_id"));
        assert!(names.contains(&"format"));
    }

    #[test]
    fn schemas_start_local_capture_requires_meeting_id() {
        let s = schemas("start_local_capture");
        assert_eq!(s.inputs[0].name, "meeting_id");
        assert!(s.inputs[0].required);
        assert_eq!(s.outputs[0].name, "result");
    }

    #[test]
    fn schemas_start_local_capture_capture_screenshots_is_optional() {
        let s = schemas("start_local_capture");
        let capture_screenshots = s
            .inputs
            .iter()
            .find(|f| f.name == "capture_screenshots")
            .expect("capture_screenshots field must be declared");
        assert!(!capture_screenshots.required);
    }

    #[test]
    fn handle_start_local_capture_capture_screenshots_param_rejects_wrong_type() {
        // `read_optional::<bool>` — used by `handle_start_local_capture` —
        // must reject a non-boolean `capture_screenshots` value with a clear
        // error rather than silently coercing it.
        let mut params = Map::new();
        params.insert("capture_screenshots".into(), json!("not-a-bool"));
        let err = read_optional::<bool>(&params, "capture_screenshots").unwrap_err();
        assert!(err.contains("invalid 'capture_screenshots'"));
    }

    #[test]
    fn handle_start_local_capture_capture_screenshots_param_defaults_to_none() {
        let params = Map::new();
        assert_eq!(
            read_optional::<bool>(&params, "capture_screenshots").unwrap(),
            None
        );
    }

    #[test]
    fn schemas_stop_local_capture_requires_meeting_id() {
        let s = schemas("stop_local_capture");
        assert!(s.inputs.iter().all(|f| f.required));
        assert_eq!(s.inputs[0].name, "meeting_id");
        assert_eq!(s.outputs[0].name, "result");
    }

    #[cfg(feature = "diarize")]
    #[test]
    fn schemas_diarize_requires_meeting_id_and_documents_outputs() {
        let s = schemas("diarize");
        assert_eq!(s.function, "diarize");
        assert!(s.inputs.iter().all(|f| f.required));
        assert_eq!(s.inputs[0].name, "meeting_id");
        let names: Vec<_> = s.outputs.iter().map(|f| f.name).collect();
        assert!(names.contains(&"speakers_found"));
        assert!(names.contains(&"chunks_updated"));
    }

    #[test]
    fn schemas_unknown_function_returns_placeholder_with_error_output() {
        let s = schemas("does-not-exist");
        assert_eq!(s.function, "unknown");
        assert_eq!(s.outputs[0].name, "error");
    }

    #[test]
    fn read_required_errors_when_key_missing() {
        let params = Map::new();
        let err = read_required::<String>(&params, "meeting_id").unwrap_err();
        assert!(err.contains("missing required param 'meeting_id'"));
    }

    #[test]
    fn read_optional_returns_none_for_absent_or_null() {
        let mut params = Map::new();
        assert_eq!(read_optional::<String>(&params, "title").unwrap(), None);
        params.insert("title".into(), Value::Null);
        assert_eq!(read_optional::<String>(&params, "title").unwrap(), None);
    }

    #[test]
    fn read_optional_returns_value_when_present() {
        let mut params = Map::new();
        params.insert("title".into(), json!("Standup"));
        assert_eq!(
            read_optional::<String>(&params, "title").unwrap(),
            Some("Standup".to_string())
        );
    }
}
