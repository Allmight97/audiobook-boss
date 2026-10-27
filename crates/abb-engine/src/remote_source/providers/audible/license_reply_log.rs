use serde_json::Value;

const URL_KEYS: [&str; 7] = [
    "content_url",
    "contentUrl",
    "download_url",
    "downloadUrl",
    "offline_url",
    "offlineUrl",
    "url",
];

/// Sanitized shape of a license reply: codes and key names only, never URLs,
/// license material, or Audible's free-text messages.
pub(super) fn license_reply_log_fields(reply: &Value) -> String {
    let license = &reply["content_license"];
    format!(
        "status_code={} denial_reasons={} drm_type={} content_format={} reply_keys={} content_license_keys={} content_metadata_keys={} url_key_path={}",
        code(&license["status_code"]),
        denial_reasons(&license["license_denial_reasons"]),
        code(&license["drm_type"]),
        code(&license["content_metadata"]["content_reference"]["content_format"]),
        key_names(reply),
        key_names(license),
        key_names(&license["content_metadata"]),
        url_key_path(reply).unwrap_or_else(|| "none".to_string()),
    )
}

fn code(value: &Value) -> &str {
    value
        .as_str()
        .filter(|code| is_code(code))
        .unwrap_or("none")
}

fn is_code(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn denial_reasons(value: &Value) -> String {
    let Some(reasons) = value.as_array() else {
        return "none".to_string();
    };
    let reasons: Vec<String> = reasons
        .iter()
        .map(|reason| {
            format!(
                "{}/{}",
                code(&reason["rejectionReason"]),
                code(&reason["validationType"])
            )
        })
        .collect();
    if reasons.is_empty() {
        "none".to_string()
    } else {
        reasons.join(",")
    }
}

fn key_names(value: &Value) -> String {
    match value.as_object() {
        Some(map) if !map.is_empty() => map
            .keys()
            .map(|key| if is_code(key) { key.as_str() } else { "?" })
            .collect::<Vec<_>>()
            .join(","),
        _ => "none".to_string(),
    }
}

fn url_key_path(reply: &Value) -> Option<String> {
    URL_KEYS
        .iter()
        .find_map(|key| path_to_non_empty_string(reply, key, "$"))
}

fn path_to_non_empty_string(value: &Value, key: &str, path: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if map
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
            {
                return Some(format!("{path}.{key}"));
            }
            map.iter().find_map(|(name, entry)| {
                let name = if is_code(name) { name.as_str() } else { "?" };
                path_to_non_empty_string(entry, key, &format!("{path}.{name}"))
            })
        }
        Value::Array(values) => values.iter().enumerate().find_map(|(index, entry)| {
            path_to_non_empty_string(entry, key, &format!("{path}[{index}]"))
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn denied_reply_logs_status_and_reason_codes_without_messages() {
        let fields = license_reply_log_fields(&json!({
            "content_license": {
                "status_code": "Denied",
                "drm_type": "Adrm",
                "message": "secret-free-text",
                "license_denial_reasons": [{
                    "rejectionReason": "RequesterNotEligible",
                    "validationType": "DeviceLimit",
                    "message": "You have reached the limit"
                }]
            },
            "response_groups": ["always-returned"]
        }));

        assert!(fields.contains("status_code=Denied"));
        assert!(fields.contains("denial_reasons=RequesterNotEligible/DeviceLimit"));
        assert!(fields.contains("drm_type=Adrm"));
        assert!(fields.contains("reply_keys=content_license,response_groups"));
        assert!(fields.contains("url_key_path=none"));
        assert!(!fields.contains("secret-free-text"));
        assert!(!fields.contains("reached the limit"));
    }

    #[test]
    fn granted_reply_logs_url_path_but_never_the_url() {
        let fields = license_reply_log_fields(&json!({
            "content_license": {
                "status_code": "Granted",
                "license_response": "fake-license-material",
                "content_metadata": {
                    "content_reference": {"content_format": "AAX_44_128"},
                    "content_url": {"offline_url": "https://cdn.example.test/book.aax?Signature=fake"}
                }
            }
        }));

        assert!(fields.contains("status_code=Granted"));
        assert!(fields.contains("content_format=AAX_44_128"));
        assert!(fields
            .contains("url_key_path=$.content_license.content_metadata.content_url.offline_url"));
        assert!(!fields.contains("https://"));
        assert!(!fields.contains("fake-license-material"));
    }
}
