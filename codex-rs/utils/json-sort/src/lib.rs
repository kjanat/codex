//! JSON key sorting utilities.
//!
//! This crate provides functions to recursively sort JSON object keys
//! using a deterministic "house style" ordering for human-readable output.
//!
//! Priority keys (like `$schema`, `name`, `title`, `type`, `properties`)
//! appear first in a defined order, followed by remaining keys alphabetically.

use serde_json::Value;

/// Returns the priority rank for a JSON Schema key.
///
/// Keys with lower ranks appear first. Keys not in this list return `None`
/// and are sorted alphabetically after the prioritized keys.
///
/// # Priority Order
///
/// 1. Meta/identifier keys: `$schema`, `$id`
/// 2. Name/title keys: `name`, `title`, `description`
/// 3. Type constraints: `type`
/// 4. Property constraints: `additionalProperties`, `unevaluatedProperties`
/// 5. Structure: `properties`, `required`
/// 6. Schema-specific: `enum`, `const`, `default`
/// 7. Input/output schemas: `inputSchema`, `outputSchema`
/// 8. Definitions (last): `definitions`, `$defs`
#[must_use]
pub fn key_rank(key: &str) -> Option<usize> {
    match key {
        // Meta/identifier keys first
        "$schema" => Some(0),
        "$id" => Some(1),
        // Name/title keys
        "name" => Some(10),
        "title" => Some(11),
        "description" => Some(12),
        // Type constraints
        "type" => Some(20),
        // Property constraints
        "additionalProperties" => Some(30),
        "unevaluatedProperties" => Some(31),
        // Structure
        "properties" => Some(40),
        "required" => Some(41),
        // Schema-specific
        "enum" => Some(45),
        "const" => Some(46),
        "default" => Some(47),
        // Input/output schemas (for MCP tools)
        "inputSchema" => Some(50),
        "outputSchema" => Some(51),
        "annotations" => Some(52),
        // Definitions near the end to reduce noise (Draft 07 uses "definitions")
        "definitions" => Some(100),
        "$defs" => Some(100),
        // Everything else sorted alphabetically
        _ => None,
    }
}

/// Recursively sorts all object keys in a JSON value using "house style" ordering.
///
/// Keys are ordered by:
/// 1. Priority keys in a specific order (see [`key_rank`])
/// 2. All other keys alphabetically
///
/// Most array order is preserved since it's meaningful in JSON Schema (e.g., `required`).
/// However, `examples` arrays are sorted alphabetically for deterministic output.
///
/// # Example
///
/// ```
/// use codex_utils_json_sort::sort_json_keys;
/// use serde_json::json;
///
/// let input = json!({
///     "properties": {},
///     "type": "object",
///     "title": "Config"
/// });
///
/// let sorted = sort_json_keys(input);
/// let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();
/// assert_eq!(keys, vec!["title", "type", "properties"]);
/// ```
#[must_use]
pub fn sort_json_keys(value: Value) -> Value {
    sort_json_keys_inner(value, None)
}

/// Inner recursive function that tracks the parent key name.
fn sort_json_keys_inner(value: Value, parent_key: Option<&str>) -> Value {
    match value {
        Value::Object(map) => {
            // Collect entries and sort by (rank, key) so unknowns are alphabetical after priority keys
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| {
                let rank_a = key_rank(a).unwrap_or(50);
                let rank_b = key_rank(b).unwrap_or(50);
                match rank_a.cmp(&rank_b) {
                    std::cmp::Ordering::Equal => a.cmp(b),
                    other => other,
                }
            });

            let sorted: serde_json::Map<String, Value> = entries
                .into_iter()
                .map(|(k, v)| {
                    let sorted_v = sort_json_keys_inner(v, Some(&k));
                    (k, sorted_v)
                })
                .collect();

            Value::Object(sorted)
        }
        Value::Array(mut arr) => {
            // Sort `examples` arrays alphabetically for deterministic output
            if parent_key == Some("examples") {
                arr.sort_by(|a, b| {
                    let a_str = a.as_str().unwrap_or("");
                    let b_str = b.as_str().unwrap_or("");
                    a_str.cmp(b_str)
                });
            }
            // Recurse into array elements
            Value::Array(
                arr.into_iter()
                    .map(|v| sort_json_keys_inner(v, None))
                    .collect(),
            )
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_key_rank_ordering() {
        // Verify priority keys have correct relative ordering
        assert!(key_rank("$schema").unwrap() < key_rank("$id").unwrap());
        assert!(key_rank("$id").unwrap() < key_rank("name").unwrap());
        assert!(key_rank("name").unwrap() < key_rank("title").unwrap());
        assert!(key_rank("title").unwrap() < key_rank("description").unwrap());
        assert!(key_rank("description").unwrap() < key_rank("type").unwrap());
        assert!(key_rank("type").unwrap() < key_rank("additionalProperties").unwrap());
        assert!(key_rank("properties").unwrap() < key_rank("required").unwrap());
        assert!(key_rank("required").unwrap() < key_rank("definitions").unwrap());

        // Non-priority keys should return None
        assert!(key_rank("foo").is_none());
        assert!(key_rank("bar").is_none());
    }

    #[test]
    fn test_sort_json_keys_basic() {
        let input = json!({
            "properties": {"a": 1},
            "type": "object",
            "title": "Test"
        });

        let sorted = sort_json_keys(input);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        // title (11), type (20), properties (40)
        assert_eq!(keys, vec!["title", "type", "properties"]);
    }

    #[test]
    fn test_sort_json_keys_with_schema_meta() {
        let input = json!({
            "type": "object",
            "$id": "https://example.com/schema",
            "$schema": "https://json-schema.org/draft-07/schema"
        });

        let sorted = sort_json_keys(input);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        // $schema (0), $id (1), type (20)
        assert_eq!(keys, vec!["$schema", "$id", "type"]);
    }

    #[test]
    fn test_sort_json_keys_nested() {
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

        let sorted = sort_json_keys(input);

        // Check nested object is also sorted
        // description (12), type (20), default (47)
        let name_prop = &sorted["properties"]["name"];
        let nested_keys: Vec<_> = name_prop.as_object().unwrap().keys().collect();

        assert_eq!(nested_keys, vec!["description", "type", "default"]);
    }

    #[test]
    fn test_sort_json_keys_mcp_tool() {
        let input = json!({
            "inputSchema": {"type": "object"},
            "description": "A tool",
            "title": "My Tool",
            "name": "my-tool"
        });

        let sorted = sort_json_keys(input);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        // name (10), title (11), description (12), inputSchema (50)
        assert_eq!(keys, vec!["name", "title", "description", "inputSchema"]);
    }

    #[test]
    fn test_sort_json_keys_examples_sorted() {
        let input = json!({
            "examples": ["zebra", "apple", "mango"]
        });

        let sorted = sort_json_keys(input);
        let examples = sorted["examples"].as_array().unwrap();

        assert_eq!(
            examples,
            &vec![json!("apple"), json!("mango"), json!("zebra")]
        );
    }

    #[test]
    fn test_sort_json_keys_preserves_non_example_arrays() {
        let input = json!({
            "required": ["zebra", "apple", "mango"]
        });

        let sorted = sort_json_keys(input);
        let required = sorted["required"].as_array().unwrap();

        // Order should be preserved for non-examples arrays
        assert_eq!(
            required,
            &vec![json!("zebra"), json!("apple"), json!("mango")]
        );
    }

    #[test]
    fn test_sort_json_keys_unknown_keys_alphabetical() {
        let input = json!({
            "zebra": 1,
            "apple": 2,
            "banana": 3
        });

        let sorted = sort_json_keys(input);
        let keys: Vec<_> = sorted.as_object().unwrap().keys().collect();

        assert_eq!(keys, vec!["apple", "banana", "zebra"]);
    }
}
