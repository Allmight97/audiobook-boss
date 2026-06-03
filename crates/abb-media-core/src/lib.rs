use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MediaContainerKind {
    M4b,
    M4a,
    Aax,
    Aaxc,
    Dash,
    SupplementalPdf,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MediaProtectionKind {
    None,
    AudibleAax,
    AudibleAaxc,
    AudibleDash,
    Widevine,
    UnknownProtected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MediaSourceKind {
    LocalFile,
    RemoteProvider,
    RemoteProviderProtected,
    SupplementalAsset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MediaErrorKind {
    UnsupportedContainer,
    ProtectedContent,
    MissingDownloadUrl,
    MissingDecryptionFacts,
    ProviderProtocolFailed,
    MaterializationFailed,
    ValidationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MediaFileFacts {
    pub container: MediaContainerKind,
    pub protection: MediaProtectionKind,
    pub source: MediaSourceKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MediaProgress {
    pub operation: String,
    pub percentage: f32,
    pub bytes_done: Option<u64>,
    pub bytes_total: Option<u64>,
}

pub fn classify_media_container_path(path: &Path) -> MediaContainerKind {
    match path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("m4b") => MediaContainerKind::M4b,
        Some("m4a") => MediaContainerKind::M4a,
        Some("aax") => MediaContainerKind::Aax,
        Some("aaxc") => MediaContainerKind::Aaxc,
        Some("mpd") | Some("dash") => MediaContainerKind::Dash,
        Some("pdf") => MediaContainerKind::SupplementalPdf,
        _ => MediaContainerKind::Unknown,
    }
}

pub fn protection_for_container(
    container: MediaContainerKind,
    drm_kind: Option<&str>,
) -> MediaProtectionKind {
    if drm_kind.is_some_and(|kind| kind.eq_ignore_ascii_case("widevine")) {
        return MediaProtectionKind::Widevine;
    }

    match container {
        MediaContainerKind::M4b | MediaContainerKind::M4a | MediaContainerKind::SupplementalPdf => {
            MediaProtectionKind::None
        }
        MediaContainerKind::Aax => MediaProtectionKind::AudibleAax,
        MediaContainerKind::Aaxc => MediaProtectionKind::AudibleAaxc,
        MediaContainerKind::Dash => MediaProtectionKind::AudibleDash,
        MediaContainerKind::Unknown => {
            if drm_kind.is_some() {
                MediaProtectionKind::UnknownProtected
            } else {
                MediaProtectionKind::None
            }
        }
    }
}

pub fn container_is_import_ready_audio(container: MediaContainerKind) -> bool {
    matches!(container, MediaContainerKind::M4b | MediaContainerKind::M4a)
}

pub fn protection_requires_materializer(protection: MediaProtectionKind) -> bool {
    matches!(
        protection,
        MediaProtectionKind::AudibleAax
            | MediaProtectionKind::AudibleAaxc
            | MediaProtectionKind::AudibleDash
            | MediaProtectionKind::Widevine
            | MediaProtectionKind::UnknownProtected
    )
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
    fn classifies_media_containers_by_extension() {
        assert_eq!(
            classify_media_container_path(Path::new("book.m4b")),
            MediaContainerKind::M4b
        );
        assert_eq!(
            classify_media_container_path(Path::new("book.aax")),
            MediaContainerKind::Aax
        );
        assert_eq!(
            classify_media_container_path(Path::new("book.aaxc")),
            MediaContainerKind::Aaxc
        );
        assert_eq!(
            classify_media_container_path(Path::new("book.mpd")),
            MediaContainerKind::Dash
        );
    }

    #[test]
    fn maps_container_and_drm_to_protection_kind() {
        assert_eq!(
            protection_for_container(MediaContainerKind::M4b, None),
            MediaProtectionKind::None
        );
        assert_eq!(
            protection_for_container(MediaContainerKind::Aax, Some("Mpeg")),
            MediaProtectionKind::AudibleAax
        );
        assert_eq!(
            protection_for_container(MediaContainerKind::Aaxc, Some("Mpeg")),
            MediaProtectionKind::AudibleAaxc
        );
        assert_eq!(
            protection_for_container(MediaContainerKind::M4b, Some("Widevine")),
            MediaProtectionKind::Widevine
        );
    }

    #[test]
    fn only_m4b_family_is_import_ready_audio() {
        assert!(container_is_import_ready_audio(MediaContainerKind::M4b));
        assert!(container_is_import_ready_audio(MediaContainerKind::M4a));
        assert!(!container_is_import_ready_audio(MediaContainerKind::Aax));
        assert!(!container_is_import_ready_audio(MediaContainerKind::Aaxc));
        assert!(!container_is_import_ready_audio(MediaContainerKind::Dash));
    }

    #[test]
    fn protected_media_requires_materializer() {
        assert!(protection_requires_materializer(
            MediaProtectionKind::AudibleAax
        ));
        assert!(protection_requires_materializer(
            MediaProtectionKind::AudibleAaxc
        ));
        assert!(protection_requires_materializer(
            MediaProtectionKind::AudibleDash
        ));
        assert!(!protection_requires_materializer(MediaProtectionKind::None));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_extensions_classify_as_unknown() {
        let path = PathBuf::from(OsString::from_vec(b"book.\xFFaax".to_vec()));

        assert_eq!(
            classify_media_container_path(&path),
            MediaContainerKind::Unknown
        );
    }
}
