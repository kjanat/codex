//! Schema transforms for config types.
//!
//! These transforms are applied via `#[schemars(transform = ...)]` attributes
//! on types to customize their generated JSON Schema output.

use schemars::Schema;

/// Flatten McpServerConfig's transport enum into a single object schema.
///
/// The actual TOML structure accepts either:
/// - `command` + `args` + `env` + `env_vars` + `cwd` (stdio transport)
/// - `url` + `bearer_token_env_var` + `http_headers` + `env_http_headers` (http transport)
///
/// Plus shared fields: `enabled`, `startup_timeout_sec`, `tool_timeout_sec`,
/// `enabled_tools`, `disabled_tools`.
///
/// Schemars generates an `anyOf` with nested transport variants, but we want a flat
/// object with all properties merged and a `oneOf` constraint for the required fields.
///
/// Detection is by structure: we look for schemas that have both:
/// - `properties` containing shared MCP fields (`enabled`, `enabled_tools`)
/// - `anyOf` with variants containing `command` (stdio) or `url` (http)
pub fn flatten_mcp_server_config(schema: &mut Schema) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };

    // Detect McpServerConfig by structure:
    // 1. Has "properties" with shared fields (enabled, enabled_tools)
    // 2. Has "anyOf" array
    // 3. The anyOf variants have properties with "command" or "url"
    let has_shared_props = obj
        .get("properties")
        .and_then(|p| p.as_object())
        .is_some_and(|props| props.contains_key("enabled") && props.contains_key("enabled_tools"));

    if !has_shared_props {
        return;
    }

    let Some(any_of) = obj.get("anyOf") else {
        return;
    };

    let Some(variants) = any_of.as_array() else {
        return;
    };

    // Verify the anyOf contains transport variants (one with "command", one with "url")
    let has_stdio_variant = variants.iter().any(|v| {
        v.as_object()
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object())
            .is_some_and(|props| props.contains_key("command"))
    });

    let has_http_variant = variants.iter().any(|v| {
        v.as_object()
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object())
            .is_some_and(|props| props.contains_key("url"))
    });

    if !has_stdio_variant || !has_http_variant {
        return;
    }

    // Now we're confident this is McpServerConfig - perform the transform
    let Some(any_of) = obj.remove("anyOf") else {
        return;
    };
    let Some(variants) = any_of.as_array() else {
        return;
    };

    let mut all_properties = serde_json::Map::new();
    let mut stdio_required = Vec::new();
    let mut http_required = Vec::new();

    for variant in variants {
        if let Some(variant_obj) = variant.as_object()
            && let Some(props) = variant_obj.get("properties")
            && let Some(props_obj) = props.as_object()
        {
            let is_stdio = props_obj.contains_key("command");
            let is_http = props_obj.contains_key("url");

            // Collect required fields
            if let Some(req) = variant_obj.get("required")
                && let Some(req_arr) = req.as_array()
            {
                let reqs: Vec<String> = req_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if is_stdio {
                    stdio_required = reqs;
                } else if is_http {
                    http_required = reqs;
                }
            }

            // Merge properties
            for (key, value) in props_obj {
                all_properties.insert(key.clone(), value.clone());
            }
        }
    }

    // Merge with existing properties (shared fields)
    if let Some(existing_props) = obj.get("properties")
        && let Some(existing_obj) = existing_props.as_object()
    {
        for (key, value) in existing_obj {
            if !all_properties.contains_key(key) {
                all_properties.insert(key.clone(), value.clone());
            }
        }
    }

    // Set merged properties (ordering is done later by reorder_mcp_server_config_properties)
    obj.insert(
        "properties".to_string(),
        serde_json::Value::Object(all_properties),
    );

    // Add oneOf for transport requirement
    obj.insert(
        "oneOf".to_string(),
        serde_json::json!([
            {"required": stdio_required},
            {"required": http_required}
        ]),
    );

    // Use "schema" ordering - Tombi will use the order from our properties object
    // (reordered by reorder_mcp_server_config_properties after sort_json_keys_with_config)
    obj.insert(
        "x-tombi-table-keys-order".to_string(),
        serde_json::json!("schema"),
    );

    // Set type to object
    obj.insert("type".to_string(), serde_json::json!("object"));

    // Remove additionalProperties: false if present (allow forward compatibility)
    obj.remove("additionalProperties");

    // Clear top-level required (the oneOf handles transport requirements)
    obj.remove("required");

    // Update description
    obj.insert(
        "description".to_string(),
        serde_json::json!("MCP server configuration. Must specify either 'command' (stdio transport) or 'url' (HTTP transport)."),
    );
}
