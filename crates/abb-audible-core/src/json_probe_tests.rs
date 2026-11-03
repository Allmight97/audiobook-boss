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
