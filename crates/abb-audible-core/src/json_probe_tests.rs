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
