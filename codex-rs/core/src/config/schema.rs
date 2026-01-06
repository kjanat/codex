//! JSON Schema generation for config.toml.
//!
//! This module provides functionality to generate a JSON Schema for the
//! `~/.codex/config.toml` configuration file. The schema can be used for
//! validation and editor autocompletion.
//!
//! This module is only available when the `config-schema` feature is enabled.

use codex_git::find_git_dir;
use codex_git::read_default_branch;
use codex_git::read_origin_url;
use codex_utils_json_sort::sort_json_keys;
use schemars::Schema;
use schemars::generate::SchemaSettings;
use schemars::transform::RecursiveTransform;

use super::ConfigToml;

/// Configuration for schema generation.
#[derive(Debug, Clone)]
pub struct SchemaConfig {
    /// Git remote URL (e.g., "https://github.com/user/codex.git").
    pub repo_url: Option<String>,
    /// Default branch name (e.g., "main" or "master").
    pub default_branch: Option<String>,
    /// Path to schema file relative to repo root.
    pub schema_path: String,
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self {
            repo_url: Some("https://github.com/openai/codex.git".to_string()),
            default_branch: Some("main".to_string()),
            schema_path: "codex-cli/config.schema.json".to_string(),
        }
    }
}

impl SchemaConfig {
    /// Detect configuration by reading `.git/` files directly.
    ///
    /// Looks for `.git/` in current directory and parent directories.
    /// Supports both regular repos and git worktrees.
    pub fn from_git() -> Self {
        let mut config = Self::default();

        if let Some(git_dir) = find_git_dir() {
            config.repo_url = read_origin_url(&git_dir);
            if let Some(branch) = read_default_branch(&git_dir) {
                config.default_branch = Some(branch);
            }
        }

        config
    }
}

/// Parse owner and repo from a GitHub git URL.
///
/// Handles:
/// - `https://github.com/owner/repo.git`
/// - `https://github.com/owner/repo`
/// - `git@github.com:owner/repo.git`
///
/// Returns `None` for non-GitHub URLs.
fn parse_github_owner_repo(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git");

    // HTTPS: https://github.com/owner/repo
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let (owner, repo) = rest.split_once('/')?;
        return Some((owner.to_string(), repo.to_string()));
    }

    // SSH: git@github.com:owner/repo
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let (owner, repo) = rest.split_once('/')?;
        return Some((owner.to_string(), repo.to_string()));
    }

    None
}

/// Build the `$id` URL from config.
///
/// Returns `None` for non-GitHub repositories.
fn build_schema_id(config: &SchemaConfig) -> Option<String> {
    let url = config.repo_url.as_deref()?;
    let (owner, repo) = parse_github_owner_repo(url)?;
    let branch = config.default_branch.as_deref()?;

    Some(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{}",
        config.schema_path
    ))
}

/// Generate a JSON Schema for the Codex config.toml file.
///
/// This schema describes the structure of `~/.codex/config.toml` and can be
/// used by editors for validation and autocompletion.
///
/// The schema conforms to JSON Schema Draft 7 for broad tooling compatibility.
///
/// The returned schema has all object keys sorted using "house style" ordering:
/// priority keys (`$schema`, `title`, `type`, `properties`, etc.) first,
/// then remaining keys alphabetically. This ensures readable, deterministic output.
///
/// If `config` contains GitHub repo info, a `$id` field is added pointing to
/// the raw schema URL. Non-GitHub repos will not have a `$id` field.
///
/// Model field examples are derived from the model presets (show_in_picker: true).
///
/// # Example
///
/// ```rust,ignore
/// use codex_core::config::schema::{generate_config_schema, SchemaConfig};
///
/// let config = SchemaConfig::from_git();
/// let schema = generate_config_schema(&config);
/// let json = serde_json::to_string_pretty(&schema).unwrap();
/// std::fs::write("config.schema.json", json).unwrap();
/// ```
pub fn generate_config_schema(config: &SchemaConfig) -> serde_json::Value {
    let settings =
        SchemaSettings::draft07().with_transform(RecursiveTransform(flatten_mcp_server_config));
    let generator = settings.into_generator();
    let schema = generator.into_root_schema_for::<ConfigToml>();

    // Convert to serde_json::Value
    let mut value: serde_json::Value = schema.into();

    // Insert $id if we have GitHub repo info
    if let Some(id) = build_schema_id(config)
        && let Some(obj) = value.as_object_mut()
    {
        obj.insert("$id".to_string(), serde_json::json!(id));
    }

    // Add Tombi TOML version hint for v1.1.0 features (trailing commas, etc.)
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "x-tombi-toml-version".to_string(),
            serde_json::json!("v1.1.0"),
        );
    }

    // Add model examples derived from presets
    inject_model_examples(&mut value);

    // Add feature properties with descriptions derived from Feature enum
    inject_feature_properties(&mut value);

    // Sort all keys for deterministic output
    let mut sorted = sort_json_keys(value);

    // Reorder properties for Tombi's "schema" sorting strategy.
    // This must happen after sort_json_keys since it alphabetizes everything.
    reorder_config_toml_properties(&mut sorted);
    reorder_mcp_server_config_properties(&mut sorted);
    reorder_notice_properties(&mut sorted);

    sorted
}

/// Inject model examples into the `model` and `review_model` fields.
///
/// Examples are derived from the model presets with `show_in_picker: true`.
fn inject_model_examples(schema: &mut serde_json::Value) {
    let model_ids = crate::models_manager::model_presets::picker_model_ids();
    let examples: Vec<serde_json::Value> =
        model_ids.iter().map(|id| serde_json::json!(id)).collect();

    if examples.is_empty() {
        return;
    }

    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    let Some(props) = obj.get_mut("properties") else {
        return;
    };
    let Some(props_obj) = props.as_object_mut() else {
        return;
    };

    // Add examples to `model` field
    if let Some(model_prop) = props_obj.get_mut("model")
        && let Some(model_obj) = model_prop.as_object_mut()
    {
        model_obj.insert("examples".to_string(), serde_json::json!(examples));
    }

    // Add examples to `review_model` field (same examples)
    if let Some(review_prop) = props_obj.get_mut("review_model")
        && let Some(review_obj) = review_prop.as_object_mut()
    {
        review_obj.insert("examples".to_string(), serde_json::json!(examples));
    }
}

/// Inject feature properties with descriptions into the FeaturesToml definition.
///
/// Each feature from `FEATURES` gets a property with its description derived from
/// `Feature::description()` and default value from `FeatureSpec::default_enabled`.
fn inject_feature_properties(schema: &mut serde_json::Value) {
    use crate::features::all_feature_specs;

    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    let Some(defs) = obj.get_mut("definitions") else {
        return;
    };
    let Some(defs_obj) = defs.as_object_mut() else {
        return;
    };
    let Some(features_toml) = defs_obj.get_mut("FeaturesToml") else {
        return;
    };
    let Some(features_obj) = features_toml.as_object_mut() else {
        return;
    };

    // Build properties object from FEATURES
    let mut properties = serde_json::Map::new();
    for spec in all_feature_specs() {
        let description = format!("[{}] {}", spec.stage.schema_label(), spec.id.description());
        properties.insert(
            spec.key.to_string(),
            serde_json::json!({
                "type": "boolean",
                "description": description,
                "default": spec.default_enabled
            }),
        );
    }

    features_obj.insert(
        "properties".to_string(),
        serde_json::Value::Object(properties),
    );
}

/// Desired property order for ConfigToml root in Tombi's "schema" sorting.
/// Groups related settings together, matching the official sample config.
/// See: https://developers.openai.com/codex/config-sample
const CONFIG_TOML_KEY_ORDER: &[&str] = &[
    // Core Model Selection
    "model",
    "review_model",
    "model_provider",
    "model_context_window",
    "model_auto_compact_token_limit",
    "tool_output_token_limit",
    // Reasoning & Verbosity
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "model_supports_reasoning_summaries",
    // Instruction Overrides
    "developer_instructions",
    "instructions",
    "compact_prompt",
    "experimental_instructions_file",
    "experimental_compact_prompt_file",
    // Notifications
    "notify",
    // Approval & Sandbox
    "approval_policy",
    "sandbox_mode",
    "sandbox_workspace_write",
    // Shell Environment Policy
    "shell_environment_policy",
    // History & File Opener
    "history",
    "file_opener",
    // UI, Notifications, Misc
    "tui",
    "hide_agent_reasoning",
    "show_raw_agent_reasoning",
    "disable_paste_burst",
    "windows_wsl_setup_acknowledged",
    "notice",
    // Authentication & Login
    "cli_auth_credentials_store",
    "chatgpt_base_url",
    "forced_chatgpt_workspace_id",
    "forced_login_method",
    "mcp_oauth_credentials_store",
    // Project Documentation Controls
    "project_doc_max_bytes",
    "project_doc_fallback_filenames",
    "project_root_markers",
    // Tools (legacy)
    "tools",
    // Feature Flags
    "features",
    // Experimental toggles
    "experimental_use_unified_exec_tool",
    "experimental_use_freeform_apply_patch",
    // Ghost Snapshot
    "ghost_snapshot",
    // Startup
    "check_for_update_on_startup",
    // MCP Servers
    "mcp_servers",
    // Model Providers
    "model_providers",
    "oss_provider",
    // Profiles
    "profile",
    "profiles",
    // Projects
    "projects",
    // OTEL
    "otel",
];

/// Desired property order for McpServerConfig in Tombi's "schema" sorting.
/// Order: transport identifier first, then transport-specific fields, then shared fields.
const MCP_SERVER_CONFIG_KEY_ORDER: &[&str] = &[
    // Transport identifiers (oneOf)
    "command",
    "url",
    // Stdio-specific
    "args",
    "env",
    "env_vars",
    "cwd",
    // HTTP-specific
    "bearer_token_env_var",
    "http_headers",
    "env_http_headers",
    // Shared settings
    "enabled",
    "startup_timeout_sec",
    "tool_timeout_sec",
    "enabled_tools",
    "disabled_tools",
];

/// Desired property order for Notice in Tombi's "schema" sorting.
/// Groups: security/access warnings, rate limit, migration prompts, then table at end.
const NOTICE_KEY_ORDER: &[&str] = &[
    // Security/access warnings
    "hide_full_access_warning",
    "hide_world_writable_warning",
    // Rate limit
    "hide_rate_limit_model_nudge",
    // Migration prompts
    "hide_gpt5_1_migration_prompt",
    "hide_gpt-5.1-codex-max_migration_prompt",
    // Table at end
    "model_migrations",
];

/// Reorder McpServerConfig properties for Tombi's "schema" sorting strategy.
///
/// This function reorders the properties of the McpServerConfig definition
/// so that Tombi's "schema" sorting strategy produces the desired key order
/// in formatted TOML files.
///
/// Must be called after `sort_json_keys` since that function alphabetizes everything.
fn reorder_mcp_server_config_properties(schema: &mut serde_json::Value) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    let Some(defs) = obj.get_mut("definitions") else {
        return;
    };
    let Some(defs_obj) = defs.as_object_mut() else {
        return;
    };
    let Some(mcp_config) = defs_obj.get_mut("McpServerConfig") else {
        return;
    };
    let Some(mcp_obj) = mcp_config.as_object_mut() else {
        return;
    };
    let Some(props) = mcp_obj.get_mut("properties") else {
        return;
    };
    let Some(props_obj) = props.as_object_mut() else {
        return;
    };

    // Reorder properties according to MCP_SERVER_CONFIG_KEY_ORDER
    let mut ordered = serde_json::Map::new();
    for key in MCP_SERVER_CONFIG_KEY_ORDER {
        if let Some((k, v)) = props_obj.remove_entry(*key) {
            ordered.insert(k, v);
        }
    }
    // Append any remaining properties not in MCP_SERVER_CONFIG_KEY_ORDER
    // (shouldn't happen if the constant is complete - test will catch this)
    for (k, v) in props_obj.iter() {
        if !ordered.contains_key(k) {
            ordered.insert(k.clone(), v.clone());
        }
    }

    *props_obj = ordered;
}

/// Reorder ConfigToml root properties for Tombi's "schema" sorting strategy.
///
/// This function reorders the root properties of the ConfigToml schema
/// so that Tombi's "schema" sorting strategy produces the desired key order
/// in formatted TOML files, matching the official sample config groupings.
///
/// Must be called after `sort_json_keys` since that function alphabetizes everything.
fn reorder_config_toml_properties(schema: &mut serde_json::Value) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    let Some(props) = obj.get_mut("properties") else {
        return;
    };
    let Some(props_obj) = props.as_object_mut() else {
        return;
    };

    // Reorder properties according to CONFIG_TOML_KEY_ORDER
    let mut ordered = serde_json::Map::new();
    for key in CONFIG_TOML_KEY_ORDER {
        if let Some((k, v)) = props_obj.remove_entry(*key) {
            ordered.insert(k, v);
        }
    }
    // Append any remaining properties not in CONFIG_TOML_KEY_ORDER
    // (shouldn't happen if the constant is complete - test will catch this)
    for (k, v) in props_obj.iter() {
        if !ordered.contains_key(k) {
            ordered.insert(k.clone(), v.clone());
        }
    }

    *props_obj = ordered;
}

/// Reorder Notice properties for Tombi's "schema" sorting strategy.
///
/// This function reorders the Notice definition properties so that
/// Tombi's "schema" sorting strategy produces the desired key order.
/// Also adds ascending sort for model_migrations additionalProperties.
///
/// Must be called after `sort_json_keys` since that function alphabetizes everything.
fn reorder_notice_properties(schema: &mut serde_json::Value) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    let Some(defs) = obj.get_mut("definitions") else {
        return;
    };
    let Some(defs_obj) = defs.as_object_mut() else {
        return;
    };
    let Some(notice) = defs_obj.get_mut("Notice") else {
        return;
    };
    let Some(notice_obj) = notice.as_object_mut() else {
        return;
    };
    let Some(props) = notice_obj.get_mut("properties") else {
        return;
    };
    let Some(props_obj) = props.as_object_mut() else {
        return;
    };

    // Reorder properties according to NOTICE_KEY_ORDER
    let mut ordered = serde_json::Map::new();
    for key in NOTICE_KEY_ORDER {
        if let Some((k, v)) = props_obj.remove_entry(*key) {
            ordered.insert(k, v);
        }
    }
    // Append any remaining properties not in NOTICE_KEY_ORDER
    for (k, v) in props_obj.iter() {
        if !ordered.contains_key(k) {
            ordered.insert(k.clone(), v.clone());
        }
    }

    *props_obj = ordered;

    // Add ascending sort for model_migrations keys (additionalProperties)
    if let Some(model_migrations) = props_obj.get_mut("model_migrations")
        && let Some(mm_obj) = model_migrations.as_object_mut()
    {
        mm_obj.insert(
            "x-tombi-table-keys-order".to_string(),
            serde_json::json!("ascending"),
        );
    }
}

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
fn flatten_mcp_server_config(schema: &mut Schema) {
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
    // (reordered by reorder_mcp_server_config_properties after sort_json_keys)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SchemaConfig {
        SchemaConfig::default()
    }

    #[test]
    fn test_generate_config_schema() {
        let schema = generate_config_schema(&test_config());

        // Verify it can be serialized to JSON
        let json = serde_json::to_string_pretty(&schema).expect("schema should serialize to JSON");

        // Verify it contains expected top-level properties
        assert!(json.contains("\"model\""));
        assert!(json.contains("\"approval_policy\""));
        assert!(json.contains("\"sandbox_mode\""));
        assert!(json.contains("\"mcp_servers\""));
        assert!(json.contains("\"profiles\""));
        assert!(json.contains("\"features\""));
    }

    #[test]
    fn test_schema_is_deterministic() {
        // Generate schema twice and verify they're identical
        let schema1 = generate_config_schema(&test_config());
        let schema2 = generate_config_schema(&test_config());

        let json1 = serde_json::to_string_pretty(&schema1).unwrap();
        let json2 = serde_json::to_string_pretty(&schema2).unwrap();

        assert_eq!(json1, json2, "Schema output should be deterministic");
    }

    #[test]
    fn test_schema_keys_use_house_style_ordering() {
        let schema = generate_config_schema(&test_config());

        // Verify top-level keys follow house style ordering
        if let Some(obj) = schema.as_object() {
            let keys: Vec<_> = obj.keys().collect();

            // Find positions of key house-style keys
            let schema_pos = keys.iter().position(|&k| k == "$schema");
            let title_pos = keys.iter().position(|&k| k == "title");
            let desc_pos = keys.iter().position(|&k| k == "description");
            let type_pos = keys.iter().position(|&k| k == "type");
            let props_pos = keys.iter().position(|&k| k == "properties");
            let defs_pos = keys.iter().position(|&k| k == "definitions");

            // $schema should come before title
            if let (Some(s), Some(t)) = (schema_pos, title_pos) {
                assert!(s < t, "$schema should come before title");
            }

            // title should come before description
            if let (Some(t), Some(d)) = (title_pos, desc_pos) {
                assert!(t < d, "title should come before description");
            }

            // description should come before type
            if let (Some(d), Some(t)) = (desc_pos, type_pos) {
                assert!(d < t, "description should come before type");
            }

            // type should come before properties
            if let (Some(t), Some(p)) = (type_pos, props_pos) {
                assert!(t < p, "type should come before properties");
            }

            // properties should come before definitions
            if let (Some(p), Some(d)) = (props_pos, defs_pos) {
                assert!(p < d, "properties should come before definitions");
            }
        }
    }

    #[test]
    fn test_mcp_server_config_is_flattened() {
        let schema = generate_config_schema(&test_config());
        let json = serde_json::to_string_pretty(&schema).unwrap();

        // McpServerConfig should have flattened properties including both transport types
        assert!(
            json.contains("\"command\""),
            "should have stdio command property"
        );
        assert!(json.contains("\"url\""), "should have http url property");

        // Should have oneOf for transport requirements, not anyOf
        let schema_obj = schema.as_object().unwrap();
        let defs = schema_obj.get("definitions").unwrap().as_object().unwrap();
        let mcp_config = defs.get("McpServerConfig").unwrap().as_object().unwrap();

        assert!(
            mcp_config.contains_key("oneOf"),
            "McpServerConfig should have oneOf constraint"
        );
        assert!(
            !mcp_config.contains_key("anyOf"),
            "McpServerConfig should not have anyOf after transform"
        );
    }

    #[test]
    fn test_features_toml_allows_additional_properties() {
        let schema = generate_config_schema(&test_config());

        let schema_obj = schema.as_object().unwrap();
        let defs = schema_obj.get("definitions").unwrap().as_object().unwrap();
        let features = defs.get("FeaturesToml").unwrap().as_object().unwrap();

        assert!(
            features.contains_key("additionalProperties"),
            "FeaturesToml should have additionalProperties"
        );

        let add_props = features.get("additionalProperties").unwrap();
        let add_props_obj = add_props.as_object().unwrap();
        assert_eq!(
            add_props_obj.get("type").unwrap(),
            "boolean",
            "additionalProperties should be boolean type"
        );
    }

    #[test]
    fn test_schema_id_with_github_https() {
        let config = SchemaConfig {
            repo_url: Some("https://github.com/kjanat/codex.git".to_string()),
            default_branch: Some("master".to_string()),
            schema_path: "codex-cli/config.schema.json".to_string(),
        };
        let schema = generate_config_schema(&config);

        let id = schema.get("$id").expect("should have $id");
        assert_eq!(
            id,
            "https://raw.githubusercontent.com/kjanat/codex/master/codex-cli/config.schema.json"
        );
    }

    #[test]
    fn test_schema_id_with_github_ssh() {
        let config = SchemaConfig {
            repo_url: Some("git@github.com:openai/codex.git".to_string()),
            default_branch: Some("main".to_string()),
            schema_path: "codex-cli/config.schema.json".to_string(),
        };
        let schema = generate_config_schema(&config);

        let id = schema.get("$id").expect("should have $id");
        assert_eq!(
            id,
            "https://raw.githubusercontent.com/openai/codex/main/codex-cli/config.schema.json"
        );
    }

    #[test]
    fn test_schema_id_omitted_for_non_github() {
        let config = SchemaConfig {
            repo_url: Some("https://gitlab.com/user/repo.git".to_string()),
            default_branch: Some("main".to_string()),
            schema_path: "codex-cli/config.schema.json".to_string(),
        };
        let schema = generate_config_schema(&config);

        assert!(
            schema.get("$id").is_none(),
            "should not have $id for non-GitHub repo"
        );
    }

    #[test]
    fn test_schema_id_omitted_when_no_repo_url() {
        let config = SchemaConfig {
            repo_url: None,
            ..Default::default()
        };
        let schema = generate_config_schema(&config);

        assert!(
            schema.get("$id").is_none(),
            "should not have $id without repo_url"
        );
    }

    #[test]
    fn test_schema_id_uses_defaults() {
        // SchemaConfig::default() uses openai/codex upstream
        let config = SchemaConfig::default();
        let schema = generate_config_schema(&config);

        let id = schema.get("$id").expect("should have $id");
        assert_eq!(
            id,
            "https://raw.githubusercontent.com/openai/codex/main/codex-cli/config.schema.json"
        );
    }

    #[test]
    fn test_schema_id_omitted_when_no_branch() {
        let config = SchemaConfig {
            repo_url: Some("https://github.com/user/repo".to_string()),
            default_branch: None,
            schema_path: "schema.json".to_string(),
        };
        let schema = generate_config_schema(&config);

        assert!(
            schema.get("$id").is_none(),
            "should not have $id without default_branch"
        );
    }

    #[test]
    fn test_parse_github_owner_repo() {
        // HTTPS with .git
        assert_eq!(
            parse_github_owner_repo("https://github.com/openai/codex.git"),
            Some(("openai".to_string(), "codex".to_string()))
        );

        // HTTPS without .git
        assert_eq!(
            parse_github_owner_repo("https://github.com/openai/codex"),
            Some(("openai".to_string(), "codex".to_string()))
        );

        // SSH with .git
        assert_eq!(
            parse_github_owner_repo("git@github.com:user/repo.git"),
            Some(("user".to_string(), "repo".to_string()))
        );

        // SSH without .git
        assert_eq!(
            parse_github_owner_repo("git@github.com:user/repo"),
            Some(("user".to_string(), "repo".to_string()))
        );

        // Non-GitHub
        assert_eq!(
            parse_github_owner_repo("https://gitlab.com/user/repo"),
            None
        );
        assert_eq!(parse_github_owner_repo("git@gitlab.com:user/repo"), None);
    }

    #[test]
    fn test_model_examples_derived_from_presets() {
        let schema = generate_config_schema(&test_config());

        let schema_obj = schema.as_object().unwrap();
        let props = schema_obj.get("properties").unwrap().as_object().unwrap();

        // Check model field has examples
        let model = props.get("model").unwrap().as_object().unwrap();
        let examples = model.get("examples").expect("model should have examples");
        let examples_arr = examples.as_array().unwrap();

        // Should have the picker models from presets
        assert!(
            examples_arr.contains(&serde_json::json!("gpt-5.2-codex")),
            "should contain gpt-5.2-codex"
        );
        assert!(
            examples_arr.contains(&serde_json::json!("gpt-5.1-codex-max")),
            "should contain gpt-5.1-codex-max"
        );

        // Check review_model field also has examples
        let review_model = props.get("review_model").unwrap().as_object().unwrap();
        let review_examples = review_model
            .get("examples")
            .expect("review_model should have examples");
        assert!(
            !review_examples.as_array().unwrap().is_empty(),
            "review_model should have non-empty examples"
        );
    }

    #[test]
    fn test_mcp_server_config_properties_are_ordered() {
        // This test ensures that McpServerConfig properties are in the expected order
        // for Tombi's "schema" sorting strategy. The order is defined in flatten_mcp_server_config.
        let schema = generate_config_schema(&test_config());

        let schema_obj = schema.as_object().unwrap();
        let defs = schema_obj.get("definitions").unwrap().as_object().unwrap();
        let mcp_config = defs.get("McpServerConfig").unwrap().as_object().unwrap();

        // Verify x-tombi-table-keys-order is "schema"
        let key_order = mcp_config
            .get("x-tombi-table-keys-order")
            .expect("should have x-tombi-table-keys-order");
        assert_eq!(key_order, "schema", "should use schema ordering strategy");

        // Get property keys in their current order
        let properties = mcp_config.get("properties").unwrap().as_object().unwrap();
        let prop_keys: Vec<&str> = properties.keys().map(|s| s.as_str()).collect();

        // Expected order is defined in MCP_SERVER_CONFIG_KEY_ORDER constant
        let expected_order = MCP_SERVER_CONFIG_KEY_ORDER;

        // Verify all expected keys are present (catches new properties not added to constant)
        let mut expected_sorted = expected_order.to_vec();
        expected_sorted.sort();
        let mut actual_sorted = prop_keys.clone();
        actual_sorted.sort();
        assert_eq!(
            actual_sorted,
            expected_sorted,
            "McpServerConfig properties mismatch - update MCP_SERVER_CONFIG_KEY_ORDER constant.\n\
             Missing from constant: {:?}\n\
             Extra in constant: {:?}",
            actual_sorted
                .iter()
                .filter(|k| !expected_sorted.contains(k))
                .collect::<Vec<_>>(),
            expected_sorted
                .iter()
                .filter(|k| !actual_sorted.contains(k))
                .collect::<Vec<_>>()
        );

        // Verify the order matches expected
        assert_eq!(
            prop_keys, expected_order,
            "McpServerConfig properties should be in the expected order for Tombi schema sorting"
        );
    }

    #[test]
    fn test_config_toml_properties_are_ordered() {
        // This test ensures that ConfigToml root properties are in the expected order
        // for Tombi's "schema" sorting strategy, matching the official sample config.
        let schema = generate_config_schema(&test_config());

        let schema_obj = schema.as_object().unwrap();

        // Get property keys in their current order
        let properties = schema_obj.get("properties").unwrap().as_object().unwrap();
        let prop_keys: Vec<&str> = properties.keys().map(|s| s.as_str()).collect();

        // Expected order is defined in CONFIG_TOML_KEY_ORDER constant
        let expected_order = CONFIG_TOML_KEY_ORDER;

        // Verify all expected keys are present (catches new properties not added to constant)
        let mut expected_sorted = expected_order.to_vec();
        expected_sorted.sort();
        let mut actual_sorted = prop_keys.clone();
        actual_sorted.sort();
        assert_eq!(
            actual_sorted,
            expected_sorted,
            "ConfigToml properties mismatch - update CONFIG_TOML_KEY_ORDER constant.\n\
             Missing from constant: {:?}\n\
             Extra in constant: {:?}",
            actual_sorted
                .iter()
                .filter(|k| !expected_sorted.contains(k))
                .collect::<Vec<_>>(),
            expected_sorted
                .iter()
                .filter(|k| !actual_sorted.contains(k))
                .collect::<Vec<_>>()
        );

        // Verify the order matches expected
        assert_eq!(
            prop_keys, expected_order,
            "ConfigToml properties should be in the expected order for Tombi schema sorting"
        );
    }

    #[test]
    fn test_notice_properties_are_ordered() {
        // This test ensures that Notice properties are in the expected order
        // for Tombi's "schema" sorting strategy.
        let schema = generate_config_schema(&test_config());

        let schema_obj = schema.as_object().unwrap();
        let defs = schema_obj.get("definitions").unwrap().as_object().unwrap();
        let notice = defs.get("Notice").unwrap().as_object().unwrap();

        // Verify x-tombi-table-keys-order is "schema"
        let key_order = notice
            .get("x-tombi-table-keys-order")
            .expect("should have x-tombi-table-keys-order");
        assert_eq!(key_order, "schema", "should use schema ordering strategy");

        // Get property keys in their current order
        let properties = notice.get("properties").unwrap().as_object().unwrap();
        let prop_keys: Vec<&str> = properties.keys().map(|s| s.as_str()).collect();

        // Expected order is defined in NOTICE_KEY_ORDER constant
        let expected_order = NOTICE_KEY_ORDER;

        // Verify all expected keys are present (catches new properties not added to constant)
        let mut expected_sorted = expected_order.to_vec();
        expected_sorted.sort();
        let mut actual_sorted = prop_keys.clone();
        actual_sorted.sort();
        assert_eq!(
            actual_sorted,
            expected_sorted,
            "Notice properties mismatch - update NOTICE_KEY_ORDER constant.\n\
             Missing from constant: {:?}\n\
             Extra in constant: {:?}",
            actual_sorted
                .iter()
                .filter(|k| !expected_sorted.contains(k))
                .collect::<Vec<_>>(),
            expected_sorted
                .iter()
                .filter(|k| !actual_sorted.contains(k))
                .collect::<Vec<_>>()
        );

        // Verify the order matches expected
        assert_eq!(
            prop_keys, expected_order,
            "Notice properties should be in the expected order for Tombi schema sorting"
        );

        // Verify model_migrations has ascending sort for its keys
        let model_migrations = properties
            .get("model_migrations")
            .unwrap()
            .as_object()
            .unwrap();
        assert_eq!(
            model_migrations.get("x-tombi-table-keys-order"),
            Some(&serde_json::json!("ascending")),
            "model_migrations should have ascending key order"
        );
    }

    #[test]
    fn test_deprecated_tools_field_in_schema() {
        let schema = generate_config_schema(&test_config());

        let schema_obj = schema.as_object().unwrap();
        let props = schema_obj.get("properties").unwrap().as_object().unwrap();

        // tools field should exist and be deprecated
        let tools = props
            .get("tools")
            .expect("tools field should exist in schema");
        let tools_obj = tools.as_object().unwrap();
        assert_eq!(
            tools_obj.get("deprecated"),
            Some(&serde_json::json!(true)),
            "tools should be marked deprecated"
        );

        // ToolsToml definition should exist
        let defs = schema_obj.get("definitions").unwrap().as_object().unwrap();
        let tools_toml = defs
            .get("ToolsToml")
            .expect("ToolsToml definition should exist");
        let tools_toml_obj = tools_toml.as_object().unwrap();
        assert_eq!(
            tools_toml_obj.get("deprecated"),
            Some(&serde_json::json!(true)),
            "ToolsToml should be marked deprecated"
        );
    }
}
