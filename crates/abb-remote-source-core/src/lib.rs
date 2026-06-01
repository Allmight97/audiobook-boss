use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
pub struct ProviderId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionStage {
    Auth,
    Library,
    License,
    Download,
    Decryption,
    ImportHandoff,
    Cleanup,
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
#[serde(rename_all = "snake_case")]
pub enum RemoteSourceDiagnosticKind {
    ProviderAuthRequired,
    ProviderProtocolGap,
    ProtectedContentUnsupported,
    MaterializedFileRejected,
    CleanupIncomplete,
}

pub fn classify_materialized_source_path(path: &Path) -> MaterializedSourceKind {
    match path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("m4b") | Some("m4a") => MaterializedSourceKind::ImportReadyM4b,
        Some("aax") => MaterializedSourceKind::EncryptedAax,
        Some("aaxc") => MaterializedSourceKind::EncryptedAaxc,
        Some("pdf") => MaterializedSourceKind::SupplementalPdf,
        _ => MaterializedSourceKind::Unsupported,
    }
}

pub fn materialized_source_is_import_ready(kind: MaterializedSourceKind) -> bool {
    matches!(kind, MaterializedSourceKind::ImportReadyM4b)
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
    fn only_m4b_family_is_import_ready() {
        assert!(materialized_source_is_import_ready(
            MaterializedSourceKind::ImportReadyM4b
        ));
        assert!(!materialized_source_is_import_ready(
            MaterializedSourceKind::EncryptedAax
        ));
        assert!(!materialized_source_is_import_ready(
            MaterializedSourceKind::EncryptedAaxc
        ));
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
}
