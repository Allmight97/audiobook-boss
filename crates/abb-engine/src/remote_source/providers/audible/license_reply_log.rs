use abb_audible_core::{LicenseDenial, LicenseFacts, OwnedPathStep};
use serde_json::Value;

/// Sanitized shape of a license reply: codes and key names only, never URLs,
/// license material, or Audible's free-text messages.
pub(super) fn license_reply_log_fields(reply: &Value, facts: &LicenseFacts) -> String {
    let license = &reply["content_license"];
    format!(
        "status_code={} denial_reasons={} drm_type={} content_format={} reply_keys={} content_license_keys={} content_metadata_keys={} url_key_path={}",
        optional_code(facts.status_code.as_deref()),
        denial_reasons(facts.denial.as_ref()),
        optional_code(facts.drm_kind.as_deref()),
        optional_code(facts.content_format.as_deref()),
        key_names(reply),
        key_names(license),
        key_names(&license["content_metadata"]),
        url_key_path(facts.content_url_path.as_deref()),
    )
}

fn optional_code(text: Option<&str>) -> &str {
    text.filter(|code| is_code(code)).unwrap_or("none")
}

fn is_code(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn denial_reasons(denial: Option<&LicenseDenial>) -> String {
    let reasons: Vec<String> = denial
        .into_iter()
        .flat_map(|denial| &denial.reasons)
        .map(|reason| {
            format!(
                "{}/{}",
                optional_code(Some(&reason.rejection_reason)),
                optional_code(Some(&reason.validation_type))
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

fn url_key_path(path: Option<&[OwnedPathStep]>) -> String {
    let Some(path) = path else {
        return "none".to_string();
    };
    path.iter().fold("$".to_string(), |text, step| match step {
        OwnedPathStep::Key(name) => {
            format!("{text}.{}", if is_code(name) { name.as_str() } else { "?" })
        }
        OwnedPathStep::Index(index) => format!("{text}[{index}]"),
    })
}

#[cfg(test)]
#[path = "license_reply_log_tests.rs"]
mod tests;
