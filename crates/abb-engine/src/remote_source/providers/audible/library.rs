use abb_audible_core::find_first_string_for_key;
use serde_json::Value;

use crate::remote_source::{
    ProviderId, RemoteAcquisitionFailureKind, RemoteTitle, RemoteTitleAvailability,
    RemoteTitleAvailabilityStatus,
};

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
    let availability = title_availability(value);
    Some(RemoteTitle {
        provider_id: ProviderId::Audible,
        title_id,
        title,
        authors: collect_person_names(value, "authors"),
        narrators: collect_person_names(value, "narrators"),
        duration_seconds: find_first_u64_for_key(value, "runtime_length_min")
            .map(|minutes| minutes.saturating_mul(60) as u32)
            .or_else(|| find_first_u64_for_key(value, "duration").map(|duration| duration as u32)),
        cover_url: title_cover_url(value),
        supplemental_pdf_available: find_first_string_for_key(value, "pdf_url").is_some()
            || find_first_string_for_key(value, "pdfUrl").is_some(),
        acquired: false,
        unsupported_reasons: unsupported_reasons_for_availability(&availability),
        availability,
    })
}

fn title_cover_url(value: &Value) -> Option<String> {
    find_first_string_for_key(value, "image_url")
        .or_else(|| find_first_string_for_key(value, "cover_url"))
        .or_else(|| find_first_product_image_url(value))
}

fn title_availability(value: &Value) -> RemoteTitleAvailability {
    if find_first_string_for_key(value, "status").as_deref() == Some("Revoked") {
        return RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::Revoked,
            acquirable: false,
            label: "Returned/refunded in Audible".to_string(),
            detail: Some(
                "Audible reports this title is no longer playable or downloadable for this account."
                    .to_string(),
            ),
        };
    }

    if find_first_bool_for_key(value, "is_playable") == Some(false) {
        if find_first_bool_for_key(value, "is_ayce") == Some(true) {
            return RemoteTitleAvailability {
                status: RemoteTitleAvailabilityStatus::CatalogOnly,
                acquirable: false,
                label: "Audible catalog title".to_string(),
                detail: Some(
                    "Audible reports this title is not downloadable for this account.".to_string(),
                ),
            };
        }
        return RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::ProviderUnavailable,
            acquirable: false,
            label: "Unavailable from Audible".to_string(),
            detail: Some(
                "Audible reports this title is not playable or downloadable for this account."
                    .to_string(),
            ),
        };
    }

    RemoteTitleAvailability {
        status: RemoteTitleAvailabilityStatus::Available,
        acquirable: true,
        label: "Available".to_string(),
        detail: None,
    }
}

fn unsupported_reasons_for_availability(
    availability: &RemoteTitleAvailability,
) -> Vec<RemoteAcquisitionFailureKind> {
    if availability.acquirable {
        Vec::new()
    } else {
        vec![RemoteAcquisitionFailureKind::ProtectedUnsupported]
    }
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

fn find_first_product_image_url(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map
                .get("product_images")
                .or_else(|| map.get("productImages"))
                .and_then(product_image_url_from_value)
            {
                return Some(found);
            }
            map.values().find_map(find_first_product_image_url)
        }
        Value::Array(values) => values.iter().find_map(find_first_product_image_url),
        _ => None,
    }
}

fn product_image_url_from_value(value: &Value) -> Option<String> {
    const PREFERRED_IMAGE_KEYS: &[&str] = &[
        "500", "568", "1000", "1215", "882", "764", "732", "558", "536", "480", "348", "340",
        "252", "240", "120",
    ];

    match value {
        Value::String(url) => Some(url.clone()),
        Value::Object(map) => PREFERRED_IMAGE_KEYS
            .iter()
            .find_map(|key| map.get(*key).and_then(Value::as_str))
            .map(str::to_string)
            .or_else(|| map.values().find_map(product_image_url_from_value)),
        Value::Array(values) => values.iter().find_map(product_image_url_from_value),
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

fn find_first_bool_for_key(value: &Value, key: &str) -> Option<bool> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(Value::as_bool) {
                return Some(found);
            }
            map.values()
                .find_map(|entry| find_first_bool_for_key(entry, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_first_bool_for_key(entry, key)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
