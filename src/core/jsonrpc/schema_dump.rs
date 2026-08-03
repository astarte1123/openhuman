//! Aggregation of all registered controller schemas into a JSON-serializable
//! dump, served by the `/schema` HTTP endpoint.

use serde::Serialize;

use crate::core::all;

/// JSON-serializable wrapper for the entire RPC schema dump.
#[derive(Serialize)]
pub(super) struct HttpSchemaDump {
    /// List of all available RPC methods and their schemas.
    pub(super) methods: Vec<HttpMethodSchema>,
}

/// JSON-serializable schema for a single RPC method.
#[derive(Serialize)]
pub(super) struct HttpMethodSchema {
    /// Fully qualified JSON-RPC method name.
    pub(super) method: String,
    /// Namespace of the function.
    pub(super) namespace: String,
    /// Function name within the namespace.
    function: String,
    /// Human-readable description of what the method does.
    description: String,
    /// List of input parameters.
    inputs: Vec<crate::core::FieldSchema>,
    /// List of output fields.
    outputs: Vec<crate::core::FieldSchema>,
}

/// Aggregates schemas from all registered controllers into a single dump.
///
/// Also includes built-in core methods like `core.ping` and `core.version`.
pub(super) fn build_http_schema_dump() -> HttpSchemaDump {
    let mut methods: Vec<HttpMethodSchema> = all::all_http_method_schemas()
        .into_iter()
        .map(|method| HttpMethodSchema {
            method: method.method,
            namespace: method.namespace.to_string(),
            function: method.function.to_string(),
            description: method.description.to_string(),
            inputs: method.inputs,
            outputs: method.outputs,
        })
        .collect();

    // Sort methods alphabetically for consistent output.
    methods.sort_by(|a, b| a.method.cmp(&b.method));

    HttpSchemaDump { methods }
}
