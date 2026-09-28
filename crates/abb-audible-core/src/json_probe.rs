use serde_json::Value;

/// Depth-first search for the first string value stored under `key` anywhere in
/// the JSON tree. Used to read provider response fields without binding to an
/// exact response shape.
pub fn find_first_string_for_key(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(Value::as_str) {
                return Some(found.to_string());
            }
            map.values()
                .find_map(|entry| find_first_string_for_key(entry, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_first_string_for_key(entry, key)),
        _ => None,
    }
}

/// Like [`find_first_string_for_key`] but tries each key in order.
pub fn find_first_string_for_keys(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| find_first_string_for_key(value, key))
}

/// Tries every key at an object before descending depth-first, skipping empty
/// strings. Sibling traversal order wins across subtrees; callers needing
/// global key priority search one key at a time.
pub fn find_nearest_non_empty_string_for_keys(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(Value::as_str) {
                    if !found.is_empty() {
                        return Some(found.to_string());
                    }
                }
            }
            map.values()
                .find_map(|entry| find_nearest_non_empty_string_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_nearest_non_empty_string_for_keys(entry, keys)),
        _ => None,
    }
}

/// Whether any key in `keys` holds a non-empty object anywhere in the tree.
pub fn has_non_empty_object_for_keys(value: &Value, keys: &[&str]) -> bool {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found @ Value::Object(object)) = map.get(*key) {
                    if !object.is_empty() {
                        return true;
                    }
                    if has_non_empty_object_for_keys(found, keys) {
                        return true;
                    }
                }
            }
            map.values()
                .any(|entry| has_non_empty_object_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .any(|entry| has_non_empty_object_for_keys(entry, keys)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_nested_string_value() {
        let value = json!({ "outer": { "inner": { "title": "Book" } } });
        assert_eq!(
            find_first_string_for_key(&value, "title"),
            Some("Book".to_string())
        );
    }

    #[test]
    fn first_present_key_wins() {
        let value = json!({ "pdfUrl": "b" });
        assert_eq!(
            find_first_string_for_keys(&value, &["pdf_url", "pdfUrl"]),
            Some("b".to_string())
        );
        assert_eq!(find_first_string_for_keys(&value, &["missing"]), None);
    }
}
