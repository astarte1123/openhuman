use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::core::all::{ControllerFuture, RegisteredController};
use crate::core::{ControllerSchema, FieldSchema, TypeSchema};
use crate::rpc::RpcOutcome;

use super::resolve::SUPPORTED_LOCALES;
use super::types::LocaleContext;

pub fn all_controller_schemas() -> Vec<ControllerSchema> {
    vec![schemas("resolve"), schemas("supported")]
}

pub fn all_registered_controllers() -> Vec<RegisteredController> {
    vec![
        RegisteredController {
            schema: schemas("resolve"),
            handler: handle_resolve,
        },
        RegisteredController {
            schema: schemas("supported"),
            handler: handle_supported,
        },
    ]
}

pub fn schemas(function: &str) -> ControllerSchema {
    match function {
        "resolve" => ControllerSchema {
            namespace: "locale",
            function: "resolve",
            description: "Resolve an arbitrary locale/language tag to the app's supported \
                locale, following the same fallback cascade as the frontend (exact match → \
                regional-variant alias → base-language mapping → en-GB). Never errors.",
            inputs: vec![FieldSchema {
                name: "tag",
                ty: TypeSchema::String,
                comment: "A BCP-47-ish tag, e.g. 'it-IT', 'de-AT', 'pt-BR', or a bare 'fr'.",
                required: false,
            }],
            outputs: vec![FieldSchema {
                name: "context",
                ty: TypeSchema::Ref("LocaleContext"),
                comment: "The resolved locale context (locale, language, response_language).",
                required: true,
            }],
        },
        "supported" => ControllerSchema {
            namespace: "locale",
            function: "supported",
            description: "List every locale the app supports, with native and English names.",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "locales",
                ty: TypeSchema::Array(Box::new(TypeSchema::Ref("LocaleDefinition"))),
                comment: "Supported locale registry, in display order.",
                required: true,
            }],
        },
        other => panic!("unknown locale controller function: {other}"),
    }
}

fn handle_resolve(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let tag = read_optional::<String>(&params, "tag")?;
        let context = LocaleContext::resolve(tag.as_deref());
        to_json(RpcOutcome::new(context, vec![]))
    })
}

fn handle_supported(_params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let locales: Vec<Value> = SUPPORTED_LOCALES
            .iter()
            .map(|l| {
                serde_json::json!({
                    "code": l.code,
                    "nativeName": l.native_name,
                    "englishName": l.english_name,
                })
            })
            .collect();
        to_json(RpcOutcome::new(
            serde_json::json!({ "locales": locales }),
            vec![],
        ))
    })
}

fn read_optional<T: DeserializeOwned>(
    params: &Map<String, Value>,
    key: &str,
) -> Result<Option<T>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
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

    #[test]
    fn all_schemas_returns_two() {
        assert_eq!(all_controller_schemas().len(), 2);
    }

    #[test]
    fn all_registered_controllers_returns_two() {
        assert_eq!(all_registered_controllers().len(), 2);
    }

    #[tokio::test]
    async fn handle_resolve_maps_regional_variant() {
        let mut params = Map::new();
        params.insert("tag".to_string(), Value::String("de-AT".to_string()));
        let value = handle_resolve(params).await.expect("resolve succeeds");
        assert_eq!(value["locale"], "de-DE");
        assert_eq!(value["language"], "German");
    }

    #[tokio::test]
    async fn handle_resolve_defaults_when_tag_absent() {
        let value = handle_resolve(Map::new()).await.expect("resolve succeeds");
        assert_eq!(value["locale"], "en-GB");
    }

    #[tokio::test]
    async fn handle_supported_lists_all_25_locales() {
        let value = handle_supported(Map::new())
            .await
            .expect("supported succeeds");
        let locales = value["locales"].as_array().expect("array");
        assert_eq!(locales.len(), 25);
    }
}
