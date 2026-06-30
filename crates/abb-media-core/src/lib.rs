use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const PDF_HEADER_BYTES: usize = 5;

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

/// Maximum accepted size for a Supplemental PDF asset (100 MiB). This is the
/// single canonical policy value shared by the acquisition downloader, the
/// processing handoff, and the output-artifact commit boundary.
pub const MAX_SUPPLEMENTAL_PDF_BYTES: u64 = 100 * 1024 * 1024;

/// True when the bytes begin with the PDF magic header (`%PDF-`).
pub fn has_pdf_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplementalPdfIdentity {
    pub size_bytes: u64,
    pub sha256: String,
    pub has_pdf_magic: bool,
}

pub struct SupplementalPdfIdentityBuilder {
    hasher: Sha256,
    size_bytes: u64,
    header: [u8; PDF_HEADER_BYTES],
    header_len: usize,
}

impl Default for SupplementalPdfIdentityBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SupplementalPdfIdentityBuilder {
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            size_bytes: 0,
            header: [0; PDF_HEADER_BYTES],
            header_len: 0,
        }
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn update(&mut self, chunk: &[u8]) {
        if self.header_len < PDF_HEADER_BYTES {
            let remaining = PDF_HEADER_BYTES - self.header_len;
            let copy_len = remaining.min(chunk.len());
            self.header[self.header_len..self.header_len + copy_len]
                .copy_from_slice(&chunk[..copy_len]);
            self.header_len += copy_len;
        }
        self.hasher.update(chunk);
        self.size_bytes += chunk.len() as u64;
    }

    pub fn finalize(self) -> SupplementalPdfIdentity {
        SupplementalPdfIdentity {
            size_bytes: self.size_bytes,
            sha256: format!("{:x}", self.hasher.finalize()),
            has_pdf_magic: has_pdf_magic(&self.header[..self.header_len]),
        }
    }
}

/// Lowercase hex-encoded SHA-256 digest of the given bytes. Shared content
/// identity fact used for supplemental-asset integrity checks.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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

    #[cfg(unix)]
    #[test]
    fn non_utf8_extensions_classify_as_unknown() {
        let path = PathBuf::from(OsString::from_vec(b"book.\xFFaax".to_vec()));

        assert_eq!(
            classify_media_container_path(&path),
            MediaContainerKind::Unknown
        );
    }

    #[test]
    fn pdf_magic_detects_only_pdf_header() {
        assert!(has_pdf_magic(b"%PDF-1.7\nbody"));
        assert!(!has_pdf_magic(b"not-a-pdf"));
        assert!(!has_pdf_magic(b""));
        assert!(!has_pdf_magic(b"%PDF"));
    }

    #[test]
    fn supplemental_pdf_limit_is_100_mib() {
        assert_eq!(MAX_SUPPLEMENTAL_PDF_BYTES, 104_857_600);
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn supplemental_pdf_streaming_identity_matches_chunked_pdf() {
        let mut identity = SupplementalPdfIdentityBuilder::new();
        identity.update(b"%P");
        identity.update(b"DF-1.7\nbody");

        let identity = identity.finalize();

        assert_eq!(identity.size_bytes, 13);
        assert!(identity.has_pdf_magic);
        assert_eq!(identity.sha256, sha256_hex(b"%PDF-1.7\nbody"));
    }

    #[test]
    fn supplemental_pdf_short_stream_is_not_pdf_magic() {
        let mut identity = SupplementalPdfIdentityBuilder::new();
        identity.update(b"%PDF");

        assert!(!identity.finalize().has_pdf_magic);
    }
}
