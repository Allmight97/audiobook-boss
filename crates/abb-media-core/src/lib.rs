use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const PDF_HEADER_BYTES: usize = 5;

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

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
            sha256: lower_hex(&self.hasher.finalize()),
            has_pdf_magic: has_pdf_magic(&self.header[..self.header_len]),
        }
    }
}

/// Lowercase hex-encoded SHA-256 digest of the given bytes. Shared content
/// identity fact used for supplemental-asset integrity checks.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    lower_hex(&hasher.finalize())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
