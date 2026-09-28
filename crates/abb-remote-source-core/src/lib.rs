use serde::{Deserialize, Serialize};
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
    #[specta(type = specta_typescript::Number)]
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

pub fn acquisition_progress(
    stage: AcquisitionStage,
    fraction: Option<f32>,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
) -> AcquisitionProgress {
    let fraction = fraction
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let (start, end, message, terminal) = match stage {
        AcquisitionStage::Auth => (0.0, 5.0, "Preparing account session.", false),
        AcquisitionStage::Library => (0.0, 5.0, "Loading remote library.", false),
        AcquisitionStage::License => (0.0, 15.0, "Requesting download license.", false),
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

/// Scopes a title's stage progress to its batch. `item_index` is 1-based; the
/// percentage covers the whole batch so a multi-title bar never restarts.
pub fn acquisition_progress_for_current_title(
    mut progress: AcquisitionProgress,
    title_id: impl Into<String>,
    item_index: u32,
    total_items: u32,
) -> AcquisitionProgress {
    if !progress.terminal && total_items > 0 {
        let completed_titles = item_index.clamp(1, total_items) - 1;
        progress.percentage =
            (completed_titles as f32 * 100.0 + progress.percentage) / total_items as f32;
    }
    progress.current_title_id = Some(title_id.into());
    progress.current_item_index = Some(item_index);
    progress.total_items = Some(total_items);
    progress
}

pub fn materialized_source_kind_for_container(
    container: MediaContainerKind,
) -> MaterializedSourceKind {
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn multi_title_progress_is_one_batch_bar() {
        let halfway = || acquisition_progress(AcquisitionStage::Download, Some(0.5), None, None);

        let first = acquisition_progress_for_current_title(halfway(), "B1", 1, 2);
        let second = acquisition_progress_for_current_title(halfway(), "B2", 2, 2);
        let only = acquisition_progress_for_current_title(halfway(), "B1", 1, 1);

        assert!((first.percentage - 20.0).abs() < 0.001);
        assert!((second.percentage - 70.0).abs() < 0.001);
        assert!((only.percentage - 40.0).abs() < 0.001);
    }

    #[test]
    fn progress_plan_rejects_non_finite_fraction_at_its_owner() {
        let progress = acquisition_progress(AcquisitionStage::Download, Some(f32::NAN), None, None);

        assert_eq!(progress.percentage, 15.0);
        assert!(progress.percentage.is_finite());
    }
}
