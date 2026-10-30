use abb_media_core::{MediaContainerKind, MediaProtectionKind};
use abb_remote_source_core::{
    materialized_source_kind_for_container, AcquisitionStrategy, MaterializedSourceKind,
};
use serde_json::Value;
use std::path::Path;

use crate::json_probe::{find_nearest_non_empty_string_for_keys, has_non_empty_object_for_keys};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseFacts {
    pub content_url: Option<String>,
    pub content_kind: MaterializedSourceKind,
    pub media_container: MediaContainerKind,
    pub media_protection: MediaProtectionKind,
    pub decryption_material_present: bool,
    pub drm_kind: Option<String>,
    pub supplemental_pdf_url: Option<String>,
    pub denial: Option<LicenseDenial>,
}

/// Audible answered the license request but refused it (`status_code: Denied`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseDenial {
    pub reasons: Vec<LicenseDenialReason>,
}

/// One refused eligibility check, as Audible's reason codes only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseDenialReason {
    pub rejection_reason: String,
    pub validation_type: String,
}

impl LicenseDenial {
    /// Audible is rate-limiting the account; the same request works later.
    pub fn is_throttled(&self) -> bool {
        self.reasons
            .iter()
            .any(|reason| reason.rejection_reason == "CustomerThrottled")
    }
}

fn license_denial(value: &Value) -> Option<LicenseDenial> {
    let license = value.get("content_license")?;
    let denied = license
        .get("status_code")
        .and_then(Value::as_str)
        .is_some_and(|status| status.eq_ignore_ascii_case("Denied"));
    if !denied {
        return None;
    }
    let text = |reason: &Value, key: &str| {
        reason
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let reasons = license
        .get("license_denial_reasons")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|reason| LicenseDenialReason {
            rejection_reason: text(reason, "rejectionReason"),
            validation_type: text(reason, "validationType"),
        })
        .collect();
    Some(LicenseDenial { reasons })
}

pub fn license_facts_from_value(value: &Value) -> LicenseFacts {
    // An audio-specific key outranks an unrelated generic URL anywhere in the
    // response. Search each key separately while still skipping empty strings.
    let content_url = [
        "content_url",
        "contentUrl",
        "download_url",
        "downloadUrl",
        "offline_url",
        "offlineUrl",
        "url",
    ]
    .iter()
    .find_map(|key| find_nearest_non_empty_string_for_keys(value, &[*key]));
    let media_container = content_url
        .as_deref()
        .map(classify_media_container_url)
        .unwrap_or(MediaContainerKind::Unknown);
    let drm_kind = find_nearest_non_empty_string_for_keys(value, &["drm_type", "drmType", "drm"]);
    let media_protection =
        abb_media_core::protection_for_container(media_container, drm_kind.as_deref());
    let content_kind = materialized_source_kind_for_container(media_container);
    let supplemental_pdf_url =
        find_nearest_non_empty_string_for_keys(value, &["pdf_url", "pdfUrl"]);
    let decryption_material_present = find_nearest_non_empty_string_for_keys(
        value,
        &[
            "voucher",
            "license",
            "license_response",
            "licenseResponse",
            "license_key",
            "licenseKey",
            "content_license",
            "contentLicense",
        ],
    )
    .is_some()
        || has_non_empty_object_for_keys(
            value,
            &["voucher", "license", "content_license", "contentLicense"],
        );

    LicenseFacts {
        content_url,
        content_kind,
        media_container,
        media_protection,
        decryption_material_present,
        drm_kind,
        supplemental_pdf_url,
        denial: license_denial(value),
    }
}

pub fn choose_acquisition_strategy(facts: &LicenseFacts) -> AcquisitionStrategy {
    if facts.content_url.is_none() {
        return AcquisitionStrategy::ProviderProtocolFailed;
    }

    if matches!(facts.media_protection, MediaProtectionKind::Widevine) {
        return AcquisitionStrategy::ProtectedUnsupported;
    }

    match facts.media_protection {
        MediaProtectionKind::None
            if abb_media_core::container_is_import_ready_audio(facts.media_container) =>
        {
            AcquisitionStrategy::DownloadImportReady
        }
        MediaProtectionKind::AudibleAax if facts.decryption_material_present => {
            AcquisitionStrategy::DownloadThenDecryptAax
        }
        MediaProtectionKind::AudibleAaxc if facts.decryption_material_present => {
            AcquisitionStrategy::DownloadThenDecryptAaxc
        }
        MediaProtectionKind::AudibleDash if facts.decryption_material_present => {
            AcquisitionStrategy::DownloadThenDecryptDash
        }
        MediaProtectionKind::AudibleAax
        | MediaProtectionKind::AudibleAaxc
        | MediaProtectionKind::AudibleDash
        | MediaProtectionKind::Widevine
        | MediaProtectionKind::UnknownProtected => AcquisitionStrategy::ProtectedUnsupported,
        MediaProtectionKind::None => AcquisitionStrategy::ProviderProtocolFailed,
    }
}

fn classify_media_container_url(url: &str) -> MediaContainerKind {
    let without_query = url.split('?').next().unwrap_or(url);
    abb_media_core::classify_media_container_path(Path::new(without_query))
}

#[cfg(test)]
#[path = "license_facts_tests.rs"]
mod tests;
