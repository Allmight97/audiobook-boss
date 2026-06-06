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
        cover_url: find_first_string_for_key(value, "image_url")
            .or_else(|| find_first_string_for_key(value, "cover_url")),
        supplemental_pdf_available: find_first_string_for_key(value, "pdf_url").is_some()
            || find_first_string_for_key(value, "pdfUrl").is_some(),
        acquired: false,
        unsupported_reasons: unsupported_reasons_for_availability(&availability),
        availability,
    })
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
        assert_eq!(
            title.availability.status,
            RemoteTitleAvailabilityStatus::Available
        );
        assert!(title.availability.acquirable);
        assert!(title.unsupported_reasons.is_empty());
    }

    #[test]
    fn parse_library_titles_marks_non_playable_library_items_unsupported() {
        let payload = json!({
            "items": [
                {
                    "asin": "B000000001",
                    "title": "Subscription Visible Book",
                    "is_playable": false,
                    "is_listenable": true,
                    "is_ayce": true
                }
            ]
        });

        let titles = parse_library_titles(&payload);

        assert_eq!(titles.len(), 1);
        assert_eq!(
            titles[0].unsupported_reasons,
            vec![RemoteAcquisitionFailureKind::ProtectedUnsupported]
        );
        assert_eq!(
            titles[0].availability.status,
            RemoteTitleAvailabilityStatus::CatalogOnly
        );
        assert!(!titles[0].availability.acquirable);
        assert_eq!(titles[0].availability.label, "Audible catalog title");
    }

    #[test]
    fn parse_library_titles_marks_revoked_items_as_returned_or_refunded() {
        let payload = json!({
            "items": [
                {
                    "asin": "B000000001",
                    "title": "Returned Book",
                    "status": "Revoked",
                    "is_playable": false
                }
            ]
        });

        let titles = parse_library_titles(&payload);

        assert_eq!(titles.len(), 1);
        assert_eq!(
            titles[0].availability.status,
            RemoteTitleAvailabilityStatus::Revoked
        );
        assert_eq!(titles[0].availability.label, "Returned/refunded in Audible");
    }
}
