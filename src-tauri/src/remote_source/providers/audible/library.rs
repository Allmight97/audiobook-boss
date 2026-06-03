use serde_json::Value;

use crate::remote_source::{ProviderId, RemoteTitle};

pub(super) fn parse_library_titles(payload: &Value) -> Vec<RemoteTitle> {
    let empty = Vec::new();
    find_array_for_key(payload, "items")
        .or_else(|| find_array_for_key(payload, "products"))
        .unwrap_or(&empty)
        .iter()
        .filter_map(parse_title)
        .collect()
}

fn parse_title(value: &Value) -> Option<RemoteTitle> {
    let title_id = value
        .get("asin")
        .and_then(Value::as_str)
        .or_else(|| value.get("sku").and_then(Value::as_str))?
        .to_string();
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled Audible title")
        .to_string();
    Some(RemoteTitle {
        provider_id: ProviderId::Audible,
        title_id,
        title,
        authors: collect_person_names(value, "authors"),
        narrators: collect_person_names(value, "narrators"),
        duration_seconds: find_first_u64_for_key(value, "runtime_length_min")
            .map(|minutes| minutes.saturating_mul(60) as u32)
            .or_else(|| find_first_u64_for_key(value, "duration").map(|duration| duration as u32)),
        cover_url: find_first_string_for_key(value, "image_url")
            .or_else(|| find_first_string_for_key(value, "cover_url")),
        supplemental_pdf_available: find_first_string_for_key(value, "pdf_url").is_some()
            || find_first_string_for_key(value, "pdfUrl").is_some(),
        acquired: false,
        unsupported_reasons: Vec::new(),
    })
}

fn collect_person_names(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("name")
                        .and_then(Value::as_str)
                        .or_else(|| item.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn find_array_for_key<'a>(value: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    match value {
        Value::Object(map) => {
            if let Some(array) = map.get(key).and_then(Value::as_array) {
                return Some(array);
            }
            map.values()
                .find_map(|entry| find_array_for_key(entry, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_array_for_key(entry, key)),
        _ => None,
    }
}

fn find_first_string_for_key(value: &Value, key: &str) -> Option<String> {
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

fn find_first_u64_for_key(value: &Value, key: &str) -> Option<u64> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(Value::as_u64) {
                return Some(found);
            }
            map.values()
                .find_map(|entry| find_first_u64_for_key(entry, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_first_u64_for_key(entry, key)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_library_titles_extracts_pdf_availability_without_raw_payload_leak() {
        let payload = json!({
            "items": [
                {
                    "asin": "B000000001",
                    "title": "Remote Book",
                    "authors": [{"name": "Author One"}],
                    "narrators": [{"name": "Narrator One"}],
                    "runtime_length_min": 90,
                    "product_images": {"500": "https://example.test/cover.jpg"},
                    "details": {"pdf_url": "https://example.test/supplement.pdf"}
                }
            ]
        });

        let titles = parse_library_titles(&payload);

        assert_eq!(titles.len(), 1);
        let title = &titles[0];
        assert_eq!(title.provider_id, ProviderId::Audible);
        assert_eq!(title.title_id, "B000000001");
        assert_eq!(title.authors, vec!["Author One"]);
        assert_eq!(title.narrators, vec!["Narrator One"]);
        assert_eq!(title.duration_seconds, Some(5_400));
        assert!(title.supplemental_pdf_available);
        assert!(title.unsupported_reasons.is_empty());
    }
}
