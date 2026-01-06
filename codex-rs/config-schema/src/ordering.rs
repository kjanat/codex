//! Property ordering for Tombi's "schema" sorting strategy.
//!
//! These constants and functions control the order of properties in the generated
//! JSON Schema so that Tombi's `x-tombi-table-keys-order: "schema"` directive
//! produces the desired key order in formatted TOML files.

/// Desired property order for ConfigToml root in Tombi's "schema" sorting.
/// Groups related settings together, matching the official sample config.
/// See: https://developers.openai.com/codex/config-sample
pub const CONFIG_TOML_KEY_ORDER: &[&str] = &[
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
pub const MCP_SERVER_CONFIG_KEY_ORDER: &[&str] = &[
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
pub const NOTICE_KEY_ORDER: &[&str] = &[
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
/// Must be called after `sort_json_keys_with_config` since that function alphabetizes everything.
pub fn reorder_mcp_server_config_properties(schema: &mut serde_json::Value) {
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
/// Must be called after `sort_json_keys_with_config` since that function alphabetizes everything.
pub fn reorder_config_toml_properties(schema: &mut serde_json::Value) {
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
/// Must be called after `sort_json_keys_with_config` since that function alphabetizes everything.
pub fn reorder_notice_properties(schema: &mut serde_json::Value) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::SchemaConfig;
    use crate::generate::generate_config_schema;

    fn test_config() -> SchemaConfig {
        SchemaConfig::default()
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
        let prop_keys: Vec<&str> = properties.keys().map(std::string::String::as_str).collect();

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
        let prop_keys: Vec<&str> = properties.keys().map(std::string::String::as_str).collect();

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
        let prop_keys: Vec<&str> = properties.keys().map(std::string::String::as_str).collect();

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
}
