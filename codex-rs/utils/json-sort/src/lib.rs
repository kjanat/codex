//! Configurable JSON key sorting utilities.
//!
//! This crate provides functions to recursively sort JSON object keys
//! with optional priority ordering and array sorting behavior.

use serde_json::Value;

/// Configuration for JSON key sorting behavior.
///
/// # Example
///
/// ```
/// use codex_utils_json_sort::SortConfig;
///
/// // Sort with priority keys first, then alphabetically
/// let config = SortConfig {
///     priority_keys: &["id", "name", "type"],
///     sorted_array_keys: &["tags"],
/// };
/// ```
#[derive(Clone, Copy, Default)]
pub struct SortConfig {
    /// Priority keys in order (lower index = higher priority).
    /// Keys not in this list are sorted alphabetically after priority keys.
    pub priority_keys: &'static [&'static str],

    /// Array keys whose string elements should be sorted alphabetically.
    /// For example, `&["examples"]` will sort the contents of any array
    /// that is the value of an "examples" key.
    pub sorted_array_keys: &'static [&'static str],
}

impl SortConfig {
    /// Returns the priority rank for a key based on its position in `priority_keys`.
    ///
    /// Lower rank = higher priority (appears first).
    /// Returns `None` for keys not in the priority list.
    fn key_rank(&self, key: &str) -> Option<usize> {
        self.priority_keys.iter().position(|&k| k == key)
    }

    /// Returns whether an array with the given parent key should be sorted.
    fn should_sort_array(&self, parent_key: Option<&str>) -> bool {
        parent_key.is_some_and(|k| self.sorted_array_keys.contains(&k))
    }
}

/// Recursively sorts all object keys in a JSON value alphabetically.
///
/// This is equivalent to `sort_json_keys_with_config(value, &SortConfig::default())`.
///
/// Array order is preserved since it may be semantically meaningful.
///
/// # Example
///
/// ```
/// use codex_utils_json_sort::sort_json_keys;
/// use serde_json::json;
///
/// let input = json!({
///     "zebra": 1,
///     "apple": 2,
///     "mango": 3
/// });
///
/// let sorted = sort_json_keys(input);
/// let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();
/// assert_eq!(keys, vec!["apple", "mango", "zebra"]);
/// ```
#[must_use]
pub fn sort_json_keys(value: Value) -> Value {
    sort_json_keys_with_config(value, &SortConfig::default())
}

/// Recursively sorts all object keys in a JSON value using the provided configuration.
///
/// Keys are ordered by:
/// 1. Priority keys in their specified order (see [`SortConfig::priority_keys`])
/// 2. All other keys alphabetically
///
/// Arrays specified in [`SortConfig::sorted_array_keys`] will have their
/// string elements sorted alphabetically. Other arrays preserve their order.
///
/// # Example
///
/// ```
/// use codex_utils_json_sort::{sort_json_keys_with_config, SortConfig};
/// use serde_json::json;
///
/// let config = SortConfig {
///     priority_keys: &["name", "type", "properties"],
///     sorted_array_keys: &["examples"],
/// };
///
/// let input = json!({
///     "properties": {},
///     "type": "object",
///     "name": "Test"
/// });
///
/// let sorted = sort_json_keys_with_config(input, &config);
/// let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();
/// assert_eq!(keys, vec!["name", "type", "properties"]);
/// ```
#[must_use]
pub fn sort_json_keys_with_config(value: Value, config: &SortConfig) -> Value {
    sort_inner(value, None, config)
}

/// Inner recursive function that tracks the parent key name.
fn sort_inner(value: Value, parent_key: Option<&str>, config: &SortConfig) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();

            // Sort by (rank, key) so priority keys come first, then alphabetical
            let default_rank = config.priority_keys.len();
            entries.sort_by(|(a, _), (b, _)| {
                let rank_a = config.key_rank(a).unwrap_or(default_rank);
                let rank_b = config.key_rank(b).unwrap_or(default_rank);
                match rank_a.cmp(&rank_b) {
                    std::cmp::Ordering::Equal => a.cmp(b),
                    other => other,
                }
            });

            let sorted: serde_json::Map<String, Value> = entries
                .into_iter()
                .map(|(k, v)| {
                    let sorted_v = sort_inner(v, Some(&k), config);
                    (k, sorted_v)
                })
                .collect();

            Value::Object(sorted)
        }
        Value::Array(mut arr) => {
            // Sort array elements if this key is in sorted_array_keys
            if config.should_sort_array(parent_key) {
                arr.sort_by(|a, b| {
                    let a_str = a.as_str().unwrap_or("");
                    let b_str = b.as_str().unwrap_or("");
                    a_str.cmp(b_str)
                });
            }
            // Recurse into array elements
            Value::Array(
                arr.into_iter()
                    .map(|v| sort_inner(v, None, config))
                    .collect(),
            )
        }
        other => other,
    }
}

/// Pre-configured settings for JSON Schema output.
///
/// Priority ordering:
/// 1. Meta/identifier keys: `$schema`, `$id`
/// 2. Name/title keys: `name`, `title`, `description`
/// 3. Type constraints: `type`
/// 4. Property constraints: `additionalProperties`, `unevaluatedProperties`
/// 5. Structure: `properties`, `required`
/// 6. Schema-specific: `enum`, `const`, `default`
/// 7. Input/output schemas (MCP): `inputSchema`, `outputSchema`, `annotations`
/// 8. Definitions: `definitions`, `$defs`
///
/// Additionally, `examples` arrays are sorted alphabetically for deterministic output.
pub const JSON_SCHEMA_SORT_CONFIG: SortConfig = SortConfig {
    priority_keys: &[
        // Meta/identifier keys first
        "$schema",
        "$id",
        // Name/title keys
        "name",
        "title",
        "description",
        // Type constraints
        "type",
        // Property constraints
        "additionalProperties",
        "unevaluatedProperties",
        // Structure
        "properties",
        "required",
        // Schema-specific
        "enum",
        "const",
        "default",
        // Input/output schemas (for MCP tools)
        "inputSchema",
        "outputSchema",
        "annotations",
        // Definitions near the end
        "definitions",
        "$defs",
    ],
    sorted_array_keys: &["examples"],
};

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_sort_json_keys_alphabetical() {
        let input = json!({
            "zebra": 1,
            "apple": 2,
            "banana": 3
        });

        let sorted = sort_json_keys(input);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        assert_eq!(keys, vec!["apple", "banana", "zebra"]);
    }

    #[test]
    fn test_sort_json_keys_nested() {
        let input = json!({
            "outer": {
                "zebra": 1,
                "apple": 2
            },
            "another": 3
        });

        let sorted = sort_json_keys(input);

        let outer_keys: Vec<_> = sorted["outer"].as_object().unwrap().keys().collect();
        assert_eq!(outer_keys, vec!["apple", "zebra"]);
    }

    #[test]
    fn test_sort_json_keys_preserves_array_order() {
        let input = json!({
            "items": ["zebra", "apple", "mango"]
        });

        let sorted = sort_json_keys(input);
        let items = sorted["items"].as_array().unwrap();

        // Order should be preserved
        assert_eq!(items, &vec![json!("zebra"), json!("apple"), json!("mango")]);
    }

    #[test]
    fn test_sort_with_priority_keys() {
        let config = SortConfig {
            priority_keys: &["id", "name", "value"],
            sorted_array_keys: &[],
        };

        let input = json!({
            "value": 42,
            "other": "x",
            "name": "test",
            "id": 1
        });

        let sorted = sort_json_keys_with_config(input, &config);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        // Priority keys first (in order), then alphabetical
        assert_eq!(keys, vec!["id", "name", "value", "other"]);
    }

    #[test]
    fn test_sort_with_sorted_array_keys() {
        let config = SortConfig {
            priority_keys: &[],
            sorted_array_keys: &["tags"],
        };

        let input = json!({
            "tags": ["zebra", "apple", "mango"],
            "items": ["z", "a", "m"]
        });

        let sorted = sort_json_keys_with_config(input, &config);

        // "tags" should be sorted
        let tags = sorted["tags"].as_array().unwrap();
        assert_eq!(tags, &vec![json!("apple"), json!("mango"), json!("zebra")]);

        // "items" should preserve order
        let items = sorted["items"].as_array().unwrap();
        assert_eq!(items, &vec![json!("z"), json!("a"), json!("m")]);
    }

    #[test]
    fn test_json_schema_config_ordering() {
        let input = json!({
            "properties": {"a": 1},
            "type": "object",
            "title": "Test",
            "$schema": "https://json-schema.org/draft-07/schema"
        });

        let sorted = sort_json_keys_with_config(input, &JSON_SCHEMA_SORT_CONFIG);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        assert_eq!(keys, vec!["$schema", "title", "type", "properties"]);
    }

    #[test]
    fn test_json_schema_config_examples_sorted() {
        let input = json!({
            "examples": ["zebra", "apple", "mango"]
        });

        let sorted = sort_json_keys_with_config(input, &JSON_SCHEMA_SORT_CONFIG);
        let examples = sorted["examples"].as_array().unwrap();

        assert_eq!(
            examples,
            &vec![json!("apple"), json!("mango"), json!("zebra")]
        );
    }

    #[test]
    fn test_json_schema_config_mcp_tool() {
        let input = json!({
            "inputSchema": {"type": "object"},
            "description": "A tool",
            "title": "My Tool",
            "name": "my-tool"
        });

        let sorted = sort_json_keys_with_config(input, &JSON_SCHEMA_SORT_CONFIG);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        // name (2), title (3), description (4), inputSchema (14)
        assert_eq!(keys, vec!["name", "title", "description", "inputSchema"]);
    }

    #[test]
    fn test_json_schema_config_nested() {
        let input = json!({
            "properties": {
                "name": {
                    "default": "test",
                    "type": "string",
                    "description": "The name"
                }
            },
            "type": "object"
        });

        let sorted = sort_json_keys_with_config(input, &JSON_SCHEMA_SORT_CONFIG);

        let name_prop = &sorted["properties"]["name"];
        let nested_keys: Vec<_> = name_prop.as_object().unwrap().keys().collect();

        // description, type, default (in priority order)
        assert_eq!(nested_keys, vec!["description", "type", "default"]);
    }

    #[test]
    fn test_json_schema_config_preserves_required_order() {
        let input = json!({
            "required": ["zebra", "apple", "mango"]
        });

        let sorted = sort_json_keys_with_config(input, &JSON_SCHEMA_SORT_CONFIG);
        let required = sorted["required"].as_array().unwrap();

        // Order should be preserved (required is not in sorted_array_keys)
        assert_eq!(
            required,
            &vec![json!("zebra"), json!("apple"), json!("mango")]
        );
    }
}
