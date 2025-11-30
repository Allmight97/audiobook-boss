//! Shared input path validation for audio and image files
//!
//! Ensures a provided path is a regular file, has a supported extension,
//! contains no disallowed characters, and returns a canonical absolute path.

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};

/// Validate a single input audio file path and return its canonical absolute path.
pub fn validate_input_audio_path(path: &Path) -> Result<PathBuf> {
    // Strip invalid chars (CR/LF/NUL)
    validate_path_characters(path)?;

    // Check exists and is regular file
    validate_file_existence_and_type(path)?;

    // Extension whitelist check
    validate_audio_extension(path)?;

    // Check if it's a symlink before canonicalization (for logging)
    let is_symlink = path
        .symlink_metadata()
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false);

    if is_symlink {
        log::warn!(
            "Input file is a symlink: {} -> resolving to target",
            path.display()
        );
    }

    // Canonicalize to prevent path traversal and resolve symlinks
    let canonical = path.canonicalize().map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot canonicalize path '{}': {}",
            path.display(),
            e
        ))
    })?;

    if is_symlink {
        log::warn!("Symlink resolved to: {}", canonical.display());
    }

    Ok(canonical)
}

/// Validate a single input image file path and return its canonical absolute path.
/// Used for cover art loading to prevent path traversal attacks.
pub fn validate_input_image_path(path: &Path) -> Result<PathBuf> {
    validate_path_characters(path)?;
    validate_file_existence_and_type(path)?;
    validate_image_extension(path)?;

    let is_symlink = path
        .symlink_metadata()
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false);

    if is_symlink {
        log::warn!(
            "Input image is a symlink: {} -> resolving to target",
            path.display()
        );
    }

    let canonical = path.canonicalize().map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot canonicalize path '{}': {}",
            path.display(),
            e
        ))
    })?;

    if is_symlink {
        log::warn!("Symlink resolved to: {}", canonical.display());
    }

    Ok(canonical)
}

/// Validates path doesn't contain invalid characters (CR/LF/NUL)
fn validate_path_characters(path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();
    if path_str.contains('\n') || path_str.contains('\r') || path_str.contains('\0') {
        return Err(AppError::FileValidation(
            "Path contains invalid characters (CR/LF/NUL)".to_string(),
        ));
    }
    Ok(())
}

/// Validates file exists and is a regular file
fn validate_file_existence_and_type(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot read file metadata for '{}': {}",
            path.display(),
            e
        ))
    })?;

    if !metadata.is_file() {
        return Err(AppError::FileValidation(format!(
            "Path is not a regular file: {}",
            path.display()
        )));
    }

    Ok(())
}

/// Validates audio file extension against whitelist
fn validate_audio_extension(path: &Path) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| AppError::InvalidInput("File has no extension".to_string()))?;

    if !super::constants::ALLOWED_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::InvalidInput(format!(
            "Unsupported audio format: {ext}"
        )));
    }

    Ok(())
}

/// Validates image file extension against whitelist
fn validate_image_extension(path: &Path) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| AppError::InvalidInput("File has no extension".to_string()))?;

    if !super::constants::ALLOWED_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::InvalidInput(format!(
            "Unsupported image format: {ext}. Supported formats: jpg, jpeg, png, webp"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn test_rejects_nonexistent_path() {
        let bogus = std::path::PathBuf::from("/this/path/does/not/exist.mp3");
        let err = validate_input_audio_path(&bogus)
            .expect_err("non-existent path should fail validation");
        let msg = err.to_string();
        assert!(msg.contains("Cannot read file metadata"));
    }

    #[test]
    fn test_rejects_directory_path() {
        let dir = tempdir().expect("create temp dir");
        let err = validate_input_audio_path(dir.path())
            .expect_err("directory path should fail validation");
        let msg = err.to_string();
        assert!(msg.contains("not a regular file"));
    }

    #[test]
    fn test_rejects_invalid_extension() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("file.txt");
        File::create(&path).expect("create temp file");
        let err = validate_input_audio_path(&path)
            .expect_err("invalid extensions should fail validation");
        let msg = err.to_string();
        assert!(msg.contains("Unsupported audio format") || msg.contains("no extension"));
    }

    #[test]
    fn test_accepts_supported_extension_and_canonicalizes() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("audio.mp3");
        File::create(&path).expect("create temp file");
        let canon =
            validate_input_audio_path(&path).expect("supported extensions should pass validation");
        assert!(canon.is_absolute());
    }

    #[test]
    fn test_symlink_is_accepted_and_resolved() {
        let dir = tempdir().expect("create temp dir");
        let target = dir.path().join("real.m4a");
        File::create(&target).expect("create symlink target");
        let link = dir.path().join("link.m4a");
        symlink(&target, &link).expect("create symlink");
        let resolved = validate_input_audio_path(&link)
            .expect("symlinked files should be accepted and canonicalized");
        // Canonical path should point to the target
        assert_eq!(
            resolved,
            target
                .canonicalize()
                .expect("canonicalize symlink target should succeed")
        );
    }

    #[test]
    fn test_rejects_cr_lf_nul_in_path() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("bad\nname.mp3");
        // Try creating the file; even if OS disallows, our function should reject prior to metadata.
        let _ = File::create(&path);
        let err = validate_input_audio_path(&path)
            .expect_err("paths with invalid characters should fail validation");
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_accepts_all_supported_extensions() {
        let dir = tempdir().expect("create temp dir");
        let extensions = ["mp3", "m4a", "m4b", "aac", "wav", "flac"];

        for ext in extensions {
            let path = dir.path().join(format!("test.{}", ext));
            File::create(&path).expect("create temp file for supported extension");
            let result = validate_input_audio_path(&path);
            assert!(result.is_ok(), "Extension {} should be supported", ext);
        }
    }

    #[test]
    fn test_rejects_empty_path() {
        let empty_path = std::path::PathBuf::new();
        let err =
            validate_input_audio_path(&empty_path).expect_err("empty path should fail validation");
        let msg = err.to_string();
        assert!(msg.contains("Cannot read file metadata") || msg.contains("no extension"));
    }

    #[test]
    fn test_accepts_unicode_in_path() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("测试文件.mp3");
        File::create(&path).expect("create unicode-named file");
        let result = validate_input_audio_path(&path);
        assert!(result.is_ok(), "Unicode in path should be accepted");
    }

    #[test]
    fn test_case_insensitive_extension() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("test.MP3");
        File::create(&path).expect("create uppercase extension file");
        let result = validate_input_audio_path(&path);
        assert!(result.is_ok(), "Uppercase extension should be accepted");
    }

    #[test]
    fn test_broken_symlink_rejected() {
        let dir = tempdir().expect("create temp dir");
        let target = dir.path().join("nonexistent.mp3");
        let link = dir.path().join("broken_link.mp3");
        symlink(&target, &link).expect("create symlink to nonexistent file");

        let err =
            validate_input_audio_path(&link).expect_err("broken symlink should fail validation");
        let msg = err.to_string();
        assert!(msg.contains("Cannot read file metadata") || msg.contains("canonicalize"));
    }

    // Image path validation tests

    #[test]
    fn test_image_rejects_nonexistent_path() {
        let bogus = std::path::PathBuf::from("/this/path/does/not/exist.jpg");
        let err = validate_input_image_path(&bogus)
            .expect_err("non-existent image path should fail validation");
        let msg = err.to_string();
        assert!(msg.contains("Cannot read file metadata"));
    }

    #[test]
    fn test_image_rejects_invalid_extension() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("file.txt");
        File::create(&path).expect("create temp file");
        let err = validate_input_image_path(&path)
            .expect_err("invalid image extension should fail validation");
        let msg = err.to_string();
        assert!(msg.contains("Unsupported image format"));
    }

    #[test]
    fn test_image_accepts_supported_extensions() {
        let dir = tempdir().expect("create temp dir");
        let extensions = ["jpg", "jpeg", "png", "webp"];

        for ext in extensions {
            let path = dir.path().join(format!("cover.{}", ext));
            File::create(&path).expect("create temp image file");
            let result = validate_input_image_path(&path);
            assert!(
                result.is_ok(),
                "Image extension {} should be supported",
                ext
            );
        }
    }

    #[test]
    fn test_image_rejects_audio_extension() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("audio.mp3");
        File::create(&path).expect("create temp file");
        let err = validate_input_image_path(&path)
            .expect_err("audio extension should fail image validation");
        let msg = err.to_string();
        assert!(msg.contains("Unsupported image format"));
    }

    #[test]
    fn test_image_canonicalizes_path() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("cover.png");
        File::create(&path).expect("create temp image file");
        let canon =
            validate_input_image_path(&path).expect("valid image path should be canonicalized");
        assert!(canon.is_absolute());
    }

    #[test]
    fn test_image_rejects_traversal_attempt() {
        let dir = tempdir().expect("create temp dir");
        // Attempt path traversal
        let traversal = dir.path().join("..").join("..").join("etc").join("passwd");
        let err = validate_input_image_path(&traversal)
            .expect_err("traversal path should fail validation");
        // Should fail on either extension check or file not found
        let msg = err.to_string();
        assert!(
            msg.contains("Unsupported image format")
                || msg.contains("Cannot read file metadata")
                || msg.contains("no extension")
        );
    }
}
