use serde_json::{json, Value};

use super::library::parse_library_titles;
use super::library_request_params;

#[cfg(test)]
pub(in crate::remote_source::providers::audible) struct LibraryProbeSummary {
    pub raw_items: usize,
    pub parsed_titles: usize,
    pub supplemental_pdf_available: usize,
    pub total_hint: Option<u64>,
    pub state_token_present: bool,
}

#[cfg(test)]
pub(in crate::remote_source::providers::audible) fn library_probe_params(
    page: Option<u16>,
    status: Option<&str>,
    include_pending: Option<bool>,
) -> Value {
    let mut params = library_request_params(page);
    if let Some(status) = status {
        params["status"] = json!(status);
    }
    if let Some(include_pending) = include_pending {
        params["include_pending"] = json!(include_pending);
    }
    params
}

#[cfg(test)]
pub(in crate::remote_source::providers::audible) fn library_probe_summary(
    payload: &Value,
) -> LibraryProbeSummary {
    let titles = parse_library_titles(payload);
    LibraryProbeSummary {
        raw_items: first_array_len_for_keys(payload, &["items", "products"]).unwrap_or(0),
        parsed_titles: titles.len(),
        supplemental_pdf_available: titles
            .iter()
            .filter(|title| title.supplemental_pdf_available)
            .count(),
        total_hint: first_u64_for_keys(
            payload,
            &[
                "total_results",
                "totalResults",
                "total_count",
                "totalCount",
                "num_results",
                "numResults",
                "count",
            ],
        ),
        state_token_present: first_string_for_keys(payload, &["state_token", "stateToken"])
            .is_some(),
    }
}

#[cfg(test)]
pub(in crate::remote_source::providers::audible) fn first_array_len_for_keys(
    value: &Value,
    keys: &[&str],
) -> Option<usize> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(array) = map.get(*key).and_then(Value::as_array) {
                    return Some(array.len());
                }
            }
            map.values()
                .find_map(|entry| first_array_len_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| first_array_len_for_keys(entry, keys)),
        _ => None,
    }
}

#[cfg(test)]
pub(in crate::remote_source::providers::audible) fn first_u64_for_keys(
    value: &Value,
    keys: &[&str],
) -> Option<u64> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(Value::as_u64) {
                    return Some(found);
                }
            }
            map.values()
                .find_map(|entry| first_u64_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| first_u64_for_keys(entry, keys)),
        _ => None,
    }
}

#[cfg(test)]
pub(in crate::remote_source::providers::audible) fn first_string_for_keys(
    value: &Value,
    keys: &[&str],
) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(Value::as_str) {
                    return Some(found.to_string());
                }
            }
            map.values()
                .find_map(|entry| first_string_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| first_string_for_keys(entry, keys)),
        _ => None,
    }
}
