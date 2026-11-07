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
fn nearest_search_takes_the_first_subtree_with_any_key_and_skips_empty_strings() {
    let value = json!({
        "a": { "url": "", "fallback": { "url": "generic" } },
        "b": { "content_url": "audio" }
    });
    assert_eq!(
        find_nearest_non_empty_string_for_keys(&value, &["content_url", "url"]),
        Some("generic".to_string())
    );
    assert_eq!(
        find_nearest_non_empty_string_for_keys(&value, &["content_url"]),
        Some("audio".to_string())
    );
}

#[test]
fn the_walk_offers_an_object_before_its_children_and_reports_the_path() {
    let value = json!({
        "a": [{ "x": 1 }, { "target": "found" }],
        "target": "outer"
    });
    let mut visited = Vec::new();
    let found = find_in_tree(&value, &mut |map, path| {
        visited.push(path.to_vec());
        map.get("target")
            .and_then(Value::as_str)
            .map(|_| path.to_vec())
    });

    assert_eq!(found, Some(vec![]));
    assert_eq!(visited.len(), 1);

    let nested =
        find_nearest_non_empty_string_with_path(&json!({"a": [{"x": 1}, {"url": "u"}]}), &["url"]);
    assert_eq!(
        nested,
        Some((
            "u".to_string(),
            vec![
                OwnedPathStep::Key("a".into()),
                OwnedPathStep::Index(1),
                OwnedPathStep::Key("url".into())
            ]
        ))
    );
}
