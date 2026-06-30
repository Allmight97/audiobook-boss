use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

pub use abb_media_core::{MediaContainerKind, MediaProtectionKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
pub struct ProviderId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AcquisitionStage {
    Auth,
    Library,
    License,
    Download,
    Decryption,
    Validation,
    ImportHandoff,
    Cleanup,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MaterializedSourceKind {
    ImportReadyM4b,
    EncryptedAax,
    EncryptedAaxc,
    SupplementalPdf,
    Unsupported,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AcquisitionStrategy {
    DownloadImportReady,
    DownloadThenDecryptAax,
    DownloadThenDecryptAaxc,
    DownloadThenDecryptDash,
    ProtectedUnsupported,
    ProviderProtocolFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionProgress {
    pub stage: AcquisitionStage,
    pub percentage: f32,
    pub message: String,
    pub bytes_downloaded: Option<u64>,
    pub bytes_total: Option<u64>,
    pub current_title_id: Option<String>,
    pub current_item_index: Option<u32>,
    pub total_items: Option<u32>,
    pub terminal: bool,
}

pub fn classify_materialized_source_path(path: &Path) -> MaterializedSourceKind {
    materialized_source_kind_for_container(abb_media_core::classify_media_container_path(path))
}

pub fn materialized_source_is_import_ready(kind: MaterializedSourceKind) -> bool {
    matches!(kind, MaterializedSourceKind::ImportReadyM4b)
}

pub fn license_facts_from_value(value: &Value) -> LicenseFacts {
    let content_url = find_first_string_for_keys(
        value,
        &[
            "content_url",
            "contentUrl",
            "download_url",
            "downloadUrl",
            "offline_url",
            "offlineUrl",
            "url",
        ],
    );
    let media_container = content_url
        .as_deref()
        .map(classify_media_container_url)
        .unwrap_or(MediaContainerKind::Unknown);
    let drm_kind = find_first_string_for_keys(value, &["drm_type", "drmType", "drm"]);
    let media_protection =
        abb_media_core::protection_for_container(media_container, drm_kind.as_deref());
    let content_kind = materialized_source_kind_for_container(media_container);
    let supplemental_pdf_url = find_first_string_for_keys(value, &["pdf_url", "pdfUrl"]);
    let decryption_material_present = find_first_string_for_keys(
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
        || find_first_non_empty_object_for_keys(
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

pub fn acquisition_progress(
    stage: AcquisitionStage,
    fraction: Option<f32>,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
) -> AcquisitionProgress {
    let fraction = fraction.unwrap_or(0.0).clamp(0.0, 1.0);
    let (start, end, message, terminal) = match stage {
        AcquisitionStage::Auth => (0.0, 5.0, "Preparing account session.", false),
        AcquisitionStage::Library => (0.0, 5.0, "Loading remote library.", false),
        AcquisitionStage::License => (0.0, 15.0, "Requesting Audible license.", false),
        AcquisitionStage::Download => (15.0, 65.0, "Downloading audiobook.", false),
        AcquisitionStage::Decryption => (65.0, 90.0, "Decrypting audiobook.", false),
        AcquisitionStage::Validation => (90.0, 97.0, "Validating acquired audiobook.", false),
        AcquisitionStage::ImportHandoff => (97.0, 100.0, "Importing acquired audiobook.", false),
        AcquisitionStage::Cleanup => (100.0, 100.0, "Cleaning acquired session.", false),
        AcquisitionStage::Complete => (100.0, 100.0, "Acquisition complete.", true),
        AcquisitionStage::Failed => (100.0, 100.0, "Acquisition failed.", true),
        AcquisitionStage::Cancelled => (100.0, 100.0, "Acquisition cancelled.", true),
    };

    AcquisitionProgress {
        stage,
        percentage: start + ((end - start) * fraction),
        message: message.to_string(),
        bytes_downloaded,
        bytes_total,
        current_title_id: None,
        current_item_index: None,
        total_items: None,
        terminal,
    }
}

pub fn acquisition_progress_for_current_title(
    mut progress: AcquisitionProgress,
    title_id: impl Into<String>,
    item_index: u32,
    total_items: u32,
) -> AcquisitionProgress {
    progress.current_title_id = Some(title_id.into());
    progress.current_item_index = Some(item_index);
    progress.total_items = Some(total_items);
    progress
}

fn classify_media_container_url(url: &str) -> MediaContainerKind {
    let without_query = url.split('?').next().unwrap_or(url);
    abb_media_core::classify_media_container_path(Path::new(without_query))
}

fn materialized_source_kind_for_container(container: MediaContainerKind) -> MaterializedSourceKind {
    match container {
        MediaContainerKind::M4b | MediaContainerKind::M4a => MaterializedSourceKind::ImportReadyM4b,
        MediaContainerKind::Aax => MaterializedSourceKind::EncryptedAax,
        MediaContainerKind::Aaxc => MaterializedSourceKind::EncryptedAaxc,
        MediaContainerKind::SupplementalPdf => MaterializedSourceKind::SupplementalPdf,
        MediaContainerKind::Dash | MediaContainerKind::Unknown => {
            MaterializedSourceKind::Unsupported
        }
    }
}

fn find_first_string_for_keys(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(Value::as_str) {
                    if !found.is_empty() {
                        return Some(found.to_string());
                    }
                }
            }
            map.values()
                .find_map(|entry| find_first_string_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_first_string_for_keys(entry, keys)),
        _ => None,
    }
}

fn find_first_non_empty_object_for_keys(value: &Value, keys: &[&str]) -> bool {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found @ Value::Object(object)) = map.get(*key) {
                    if !object.is_empty() {
                        return true;
                    }
                    if find_first_non_empty_object_for_keys(found, keys) {
                        return true;
                    }
                }
            }
            map.values()
                .any(|entry| find_first_non_empty_object_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .any(|entry| find_first_non_empty_object_for_keys(entry, keys)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[cfg(unix)]
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    #[cfg(unix)]
    use std::path::PathBuf;

    #[test]
    fn classifies_import_ready_and_protected_source_extensions() {
        assert_eq!(
            classify_materialized_source_path(Path::new("book.m4b")),
            MaterializedSourceKind::ImportReadyM4b
        );
        assert_eq!(
            classify_materialized_source_path(Path::new("book.aax")),
            MaterializedSourceKind::EncryptedAax
        );
        assert_eq!(
            classify_materialized_source_path(Path::new("book.aaxc")),
            MaterializedSourceKind::EncryptedAaxc
        );
        assert_eq!(
            classify_materialized_source_path(Path::new("book.pdf")),
            MaterializedSourceKind::SupplementalPdf
        );
    }

    #[test]
    fn only_m4b_family_is_import_ready_for_materialized_handoff() {
        let cases = [
            (MaterializedSourceKind::ImportReadyM4b, true),
            (MaterializedSourceKind::EncryptedAax, false),
            (MaterializedSourceKind::EncryptedAaxc, false),
            (MaterializedSourceKind::SupplementalPdf, false),
            (MaterializedSourceKind::Unsupported, false),
        ];

        for (kind, expected) in cases {
            assert_eq!(
                materialized_source_is_import_ready(kind),
                expected,
                "{kind:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_extensions_are_classified_as_unsupported_without_failing_open() {
        let path = PathBuf::from(OsString::from_vec(b"book.\xFFaax".to_vec()));

        assert_eq!(
            classify_materialized_source_path(&path),
            MaterializedSourceKind::Unsupported
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

    #[test]
    fn progress_plan_uses_truthful_stage_bands() {
        let licensing = acquisition_progress(AcquisitionStage::License, Some(0.5), None, None);
        let downloading =
            acquisition_progress(AcquisitionStage::Download, Some(0.5), Some(50), Some(100));
        let decrypting = acquisition_progress(AcquisitionStage::Decryption, Some(0.5), None, None);
        let validating = acquisition_progress(AcquisitionStage::Validation, Some(0.5), None, None);
        let importing =
            acquisition_progress(AcquisitionStage::ImportHandoff, Some(0.5), None, None);

        assert!((0.0..=15.0).contains(&licensing.percentage));
        assert!((15.0..=65.0).contains(&downloading.percentage));
        assert!((65.0..=90.0).contains(&decrypting.percentage));
        assert!((90.0..=97.0).contains(&validating.percentage));
        assert!((97.0..=100.0).contains(&importing.percentage));
        assert_eq!(downloading.bytes_downloaded, Some(50));
        assert_eq!(downloading.bytes_total, Some(100));
        assert!(decrypting.message.to_lowercase().contains("decrypt"));

        let scoped_progress =
            acquisition_progress_for_current_title(downloading, "B000000001", 2, 3);

        assert_eq!(
            scoped_progress.current_title_id.as_deref(),
            Some("B000000001")
        );
        assert_eq!(scoped_progress.current_item_index, Some(2));
        assert_eq!(scoped_progress.total_items, Some(3));
        assert_eq!(scoped_progress.stage, AcquisitionStage::Download);
    }
}
