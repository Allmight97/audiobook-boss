//! Audible license-response interpretation: which content to fetch, how it is
//! protected, and which acquisition strategy follows.

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
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn audio_url_priority_survives_competing_branches_and_empty_candidates() {
        let book = "https://cdn.example.test/book.aaxc";
        for response in [
            json!({"a": {"url": "https://cdn.example.test/cover.jpg"},
                "z": {"content_url": book}, "license": "voucher"}),
            json!({"url": "https://cdn.example.test/sample.m4b",
                "content_license": {"content_metadata": {"content_url": {"offline_url": book}}},
                "license": "voucher"}),
            json!({"a": {"content_url": ""}, "z": [{"content_url": book}],
                "license": "voucher"}),
        ] {
            let facts = license_facts_from_value(&response);
            assert_eq!(facts.content_url.as_deref(), Some(book));
            assert_eq!(
                choose_acquisition_strategy(&facts),
                AcquisitionStrategy::DownloadThenDecryptAaxc
            );
        }
        let fallback = license_facts_from_value(&json!({
            "content_url": "", "nested": {"url": "https://cdn.example.test/book.m4b"}
        }));
        assert_eq!(
            choose_acquisition_strategy(&fallback),
            AcquisitionStrategy::DownloadImportReady
        );
    }

    #[test]
    fn license_response_facts_classify_content_protection_and_voucher() {
        let response = json!({
            "content_license": {
                "content_metadata": {
                    "content_url": {
                        "offline_url": "https://cdn.example.test/book.aaxc?Signature=fake-secret"
                    }
                },
                "drm_type": "Mpeg",
                "voucher": "fake-voucher-material",
                "license": "fake-license-material"
            },
            "details": {
                "pdf_url": "https://cdn.example.test/book.pdf?token=fake-pdf-token"
            }
        });

        let facts = license_facts_from_value(&response);

        assert_eq!(facts.content_kind, MaterializedSourceKind::EncryptedAaxc);
        assert_eq!(facts.media_container, MediaContainerKind::Aaxc);
        assert_eq!(facts.media_protection, MediaProtectionKind::AudibleAaxc);
        assert!(facts.content_url.is_some());
        assert!(facts.decryption_material_present);
        assert_eq!(facts.drm_kind.as_deref(), Some("Mpeg"));
        assert!(facts.supplemental_pdf_url.is_some());
    }

    #[test]
    fn license_facts_choose_acquisition_strategy() {
        let import_ready = LicenseFacts {
            content_url: Some("https://cdn.example.test/book.m4b".to_string()),
            content_kind: MaterializedSourceKind::ImportReadyM4b,
            media_container: MediaContainerKind::M4b,
            media_protection: MediaProtectionKind::None,
            decryption_material_present: false,
            drm_kind: None,
            supplemental_pdf_url: None,
        };
        assert_eq!(
            choose_acquisition_strategy(&import_ready),
            AcquisitionStrategy::DownloadImportReady
        );

        let encrypted_aax = LicenseFacts {
            content_url: Some("https://cdn.example.test/book.aax".to_string()),
            content_kind: MaterializedSourceKind::EncryptedAax,
            media_container: MediaContainerKind::Aax,
            media_protection: MediaProtectionKind::AudibleAax,
            decryption_material_present: true,
            drm_kind: Some("Mpeg".to_string()),
            supplemental_pdf_url: None,
        };
        assert_eq!(
            choose_acquisition_strategy(&encrypted_aax),
            AcquisitionStrategy::DownloadThenDecryptAax
        );

        let protected_without_decrypt_material = LicenseFacts {
            content_url: Some("https://cdn.example.test/book.aaxc".to_string()),
            content_kind: MaterializedSourceKind::EncryptedAaxc,
            media_container: MediaContainerKind::Aaxc,
            media_protection: MediaProtectionKind::Widevine,
            decryption_material_present: false,
            drm_kind: Some("Widevine".to_string()),
            supplemental_pdf_url: None,
        };
        assert_eq!(
            choose_acquisition_strategy(&protected_without_decrypt_material),
            AcquisitionStrategy::ProtectedUnsupported
        );

        let missing_url = LicenseFacts {
            content_url: None,
            content_kind: MaterializedSourceKind::Unsupported,
            media_container: MediaContainerKind::Unknown,
            media_protection: MediaProtectionKind::None,
            decryption_material_present: false,
            drm_kind: None,
            supplemental_pdf_url: None,
        };
        assert_eq!(
            choose_acquisition_strategy(&missing_url),
            AcquisitionStrategy::ProviderProtocolFailed
        );
    }

    #[test]
    fn dash_license_facts_choose_dash_materializer_lane() {
        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/manifest.mpd",
                "drm_type": "Mpeg",
                "license": { "key_id": "fake-key-id", "key": "fake-key" }
            }
        });

        let facts = license_facts_from_value(&response);

        assert_eq!(facts.media_container, MediaContainerKind::Dash);
        assert_eq!(facts.media_protection, MediaProtectionKind::AudibleDash);
        assert_eq!(
            choose_acquisition_strategy(&facts),
            AcquisitionStrategy::DownloadThenDecryptDash
        );
    }
}
