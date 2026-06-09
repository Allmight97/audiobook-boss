use super::collision::path_entry_exists;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use abb_media_core::{
    SupplementalPdfIdentity, SupplementalPdfIdentityBuilder, MAX_SUPPLEMENTAL_PDF_BYTES,
};
use std::io::Read;
use std::path::{Path, PathBuf};

const FILE_HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(crate) struct SupplementalOutputAssetsCommitRequest<'a> {
    output_kind: super::OutputKind,
    final_audio_path: &'a Path,
    assets: Vec<SupplementalOutputAssetCommitRequest<'a>>,
}

pub(crate) struct SupplementalOutputAssetCommitRequest<'a> {
    source_path: &'a Path,
    final_audio_path: &'a Path,
    expected_identity: Option<ExpectedSupplementalPdfIdentity<'a>>,
}

struct ExpectedSupplementalPdfIdentity<'a> {
    size_bytes: u64,
    sha256: &'a str,
}

impl<'a> SupplementalOutputAssetCommitRequest<'a> {
    pub(crate) fn new(source_path: &'a Path, final_audio_path: &'a Path) -> Self {
        Self {
            source_path,
            final_audio_path,
            expected_identity: None,
        }
    }

    pub(crate) fn with_expected_identity(mut self, size_bytes: u64, sha256: &'a str) -> Self {
        self.expected_identity = Some(ExpectedSupplementalPdfIdentity { size_bytes, sha256 });
        self
    }
}

impl<'a> SupplementalOutputAssetsCommitRequest<'a> {
    pub(crate) fn new(output_kind: super::OutputKind, final_audio_path: &'a Path) -> Self {
        Self {
            output_kind,
            final_audio_path,
            assets: Vec::new(),
        }
    }

    pub(crate) fn with_asset(
        mut self,
        source_path: &'a Path,
        size_bytes: u64,
        sha256: &'a str,
    ) -> Self {
        self.assets.push(
            SupplementalOutputAssetCommitRequest::new(source_path, self.final_audio_path)
                .with_expected_identity(size_bytes, sha256),
        );
        self
    }
}

fn supplemental_output_destination(final_audio_path: &Path) -> PathBuf {
    let parent = final_audio_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = final_audio_path
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "Book".into());
    parent.join(format!("{stem} - Supplemental PDF.pdf"))
}

fn next_available_supplemental_path(candidate: &Path) -> Result<PathBuf> {
    if !path_entry_exists(candidate)? {
        return Ok(candidate.to_path_buf());
    }

    let parent = candidate.parent().unwrap_or_else(|| Path::new("."));
    let stem = candidate
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "Supplemental PDF".into());

    for suffix in 2..=999 {
        let next = parent.join(format!("{stem} ({suffix}).pdf"));
        if !path_entry_exists(&next)? {
            return Ok(next);
        }
    }

    Err(AppError::FileValidation(format!(
        "Could not find an available Supplemental PDF destination next to '{}'.",
        sanitize_path_for_display(candidate)
    )))
}

fn inspect_supplemental_pdf_source(source_path: &Path) -> Result<u64> {
    let metadata = std::fs::symlink_metadata(source_path).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot inspect Supplemental PDF source '{}': {}",
            sanitize_path_for_display(source_path),
            error
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::FileValidation(
            "Supplemental PDF source must be a regular file.".to_string(),
        ));
    }
    if metadata.len() > MAX_SUPPLEMENTAL_PDF_BYTES {
        return Err(AppError::FileValidation(
            "Supplemental PDF source exceeds the 100 MiB size limit.".to_string(),
        ));
    }
    Ok(metadata.len())
}

fn supplemental_pdf_identity_from_inspected_file(
    source_path: &Path,
    inspected_size: u64,
) -> Result<SupplementalPdfIdentity> {
    let mut file = std::fs::File::open(source_path).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot read Supplemental PDF source '{}': {}",
            sanitize_path_for_display(source_path),
            error
        ))
    })?;
    let mut identity = SupplementalPdfIdentityBuilder::new();
    let mut buffer = [0_u8; FILE_HASH_BUFFER_BYTES];
    loop {
        let bytes_read = file.read(&mut buffer).map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot read Supplemental PDF source '{}': {}",
                sanitize_path_for_display(source_path),
                error
            ))
        })?;
        if bytes_read == 0 {
            break;
        }
        identity.update(&buffer[..bytes_read]);
        if identity.size_bytes() > inspected_size {
            return Err(AppError::FileValidation(
                "Supplemental PDF source size changed before output commit.".to_string(),
            ));
        }
    }
    let identity = identity.finalize();
    if identity.size_bytes != inspected_size {
        return Err(AppError::FileValidation(
            "Supplemental PDF source size changed before output commit.".to_string(),
        ));
    }
    Ok(identity)
}

fn ensure_pdf_source(request: &SupplementalOutputAssetCommitRequest<'_>) -> Result<()> {
    let inspected_size = inspect_supplemental_pdf_source(request.source_path)?;
    if request
        .source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("pdf"))
        != Some(true)
    {
        return Err(AppError::FileValidation(
            "Supplemental asset must have a .pdf extension.".to_string(),
        ));
    }

    if request
        .expected_identity
        .as_ref()
        .is_some_and(|expected| inspected_size != expected.size_bytes)
    {
        return Err(AppError::FileValidation(
            "Supplemental PDF source size changed before output commit.".to_string(),
        ));
    }

    let identity =
        supplemental_pdf_identity_from_inspected_file(request.source_path, inspected_size)?;
    if !identity.has_pdf_magic {
        return Err(AppError::FileValidation(
            "Supplemental asset did not pass PDF magic-byte validation.".to_string(),
        ));
    }
    if request
        .expected_identity
        .as_ref()
        .is_some_and(|expected| identity.sha256 != expected.sha256)
    {
        return Err(AppError::FileValidation(
            "Supplemental PDF source hash changed before output commit.".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn commit_supplemental_output_asset(
    request: SupplementalOutputAssetCommitRequest<'_>,
) -> Result<PathBuf> {
    ensure_pdf_source(&request)?;

    let requested = supplemental_output_destination(request.final_audio_path);
    let destination = next_available_supplemental_path(&requested)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot create Supplemental PDF output directory '{}': {}",
                sanitize_path_for_display(parent),
                error
            ))
        })?;
    }

    let mut source = std::fs::File::open(request.source_path).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot open Supplemental PDF source '{}': {}",
            sanitize_path_for_display(request.source_path),
            error
        ))
    })?;
    let mut dest = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot create Supplemental PDF output '{}': {}",
                sanitize_path_for_display(&destination),
                error
            ))
        })?;
    if let Err(error) = std::io::copy(&mut source, &mut dest) {
        let _ = std::fs::remove_file(&destination);
        return Err(AppError::FileValidation(format!(
            "Cannot copy Supplemental PDF to '{}': {}",
            sanitize_path_for_display(&destination),
            error
        )));
    }
    if let Err(error) = dest.sync_all() {
        let _ = std::fs::remove_file(&destination);
        return Err(AppError::FileValidation(format!(
            "Failed to flush Supplemental PDF '{}': {}",
            sanitize_path_for_display(&destination),
            error
        )));
    }

    let committed_bytes = std::fs::metadata(&destination)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    log::info!("supplemental_pdf_commit status=ok bytes={committed_bytes}");
    Ok(destination)
}

fn supplemental_commit_failure(final_audio_path: &Path, error: AppError) -> AppError {
    let detail = match error {
        AppError::FileValidation(message) => message,
        other => other.to_string(),
    };
    AppError::FileValidation(format!(
        "Audiobook output '{}' was created, but one or more requested Supplemental PDFs could not be committed: {detail}",
        sanitize_path_for_display(final_audio_path)
    ))
}

pub(crate) fn commit_supplemental_output_assets_for_output(
    request: SupplementalOutputAssetsCommitRequest<'_>,
) -> Result<()> {
    if request.output_kind != super::OutputKind::Final {
        return Ok(());
    }

    for asset in request.assets {
        commit_supplemental_output_asset(asset)
            .map_err(|error| supplemental_commit_failure(request.final_audio_path, error))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn supplemental_output_asset_commits_next_to_final_audio() {
        let root = TempDir::new().expect("temp root");
        let source = root.path().join("source.pdf");
        std::fs::write(&source, b"%PDF-1.7\nbody").expect("write pdf");
        let final_audio = root.path().join("Book One.m4b");

        let committed = commit_supplemental_output_asset(
            SupplementalOutputAssetCommitRequest::new(&source, &final_audio),
        )
        .expect("commit pdf");

        assert_eq!(
            committed,
            root.path().join("Book One - Supplemental PDF.pdf")
        );
        assert_eq!(
            std::fs::read(committed).expect("read committed"),
            b"%PDF-1.7\nbody"
        );
    }

    #[test]
    fn supplemental_output_asset_renames_without_replacing_existing_pdf() {
        let root = TempDir::new().expect("temp root");
        let source = root.path().join("source.pdf");
        std::fs::write(&source, b"%PDF-1.7\nbody").expect("write pdf");
        let final_audio = root.path().join("Book One.m4b");
        std::fs::write(
            root.path().join("Book One - Supplemental PDF.pdf"),
            b"existing",
        )
        .expect("write existing");

        let committed = commit_supplemental_output_asset(
            SupplementalOutputAssetCommitRequest::new(&source, &final_audio),
        )
        .expect("commit pdf");

        assert_eq!(
            committed,
            root.path().join("Book One - Supplemental PDF (2).pdf")
        );
        assert_eq!(
            std::fs::read(root.path().join("Book One - Supplemental PDF.pdf"))
                .expect("read existing"),
            b"existing"
        );
    }

    #[test]
    fn supplemental_output_asset_rejects_non_pdf_magic() {
        let root = TempDir::new().expect("temp root");
        let source = root.path().join("source.pdf");
        std::fs::write(&source, b"not-pdf").expect("write bad pdf");
        let final_audio = root.path().join("Book One.m4b");

        let error = commit_supplemental_output_asset(SupplementalOutputAssetCommitRequest::new(
            &source,
            &final_audio,
        ))
        .expect_err("bad pdf should fail");

        assert!(error.to_string().contains("magic-byte validation"));
    }

    #[test]
    fn supplemental_output_asset_rejects_oversized_pdf() {
        let root = TempDir::new().expect("temp root");
        let source = root.path().join("source.pdf");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&source)
            .expect("create source");
        file.set_len(abb_media_core::MAX_SUPPLEMENTAL_PDF_BYTES + 1)
            .expect("make sparse oversized pdf");
        std::fs::write(&source, b"%PDF-").expect("write header");
        file.set_len(abb_media_core::MAX_SUPPLEMENTAL_PDF_BYTES + 1)
            .expect("restore sparse oversized size");
        let final_audio = root.path().join("Book One.m4b");

        let error = commit_supplemental_output_asset(SupplementalOutputAssetCommitRequest::new(
            &source,
            &final_audio,
        ))
        .expect_err("oversized pdf should fail");

        assert!(error.to_string().contains("100 MiB size limit"));
    }

    #[test]
    fn supplemental_output_asset_rejects_stale_expected_hash() {
        let root = TempDir::new().expect("temp root");
        let original_bytes = b"%PDF-1.7\nbody";
        let changed_bytes = b"%PDF-1.7\nchanged";
        let source = root.path().join("source.pdf");
        std::fs::write(&source, original_bytes).expect("write source pdf");
        let request_hash = abb_media_core::sha256_hex(original_bytes);
        std::fs::write(&source, changed_bytes).expect("change source pdf");
        let final_audio = root.path().join("Book One.m4b");

        let error = commit_supplemental_output_asset(
            SupplementalOutputAssetCommitRequest::new(&source, &final_audio)
                .with_expected_identity(changed_bytes.len() as u64, &request_hash),
        )
        .expect_err("stale hash should fail");

        assert!(error.to_string().contains("hash changed"));
        assert!(
            !root.path().join("Book One - Supplemental PDF.pdf").exists(),
            "stale supplemental asset must not be committed"
        );
    }
}
