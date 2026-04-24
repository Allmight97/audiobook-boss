//! Shared input path validation for audio and image files
//!
//! Ensures a provided path is a regular file, has a supported extension,
//! contains no disallowed characters, and returns a canonical absolute path.

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{sanitize_path_for_display, AppError, Result};

/// Validate a single input audio file path and return its canonical absolute path.
pub fn validate_input_audio_path(path: &Path) -> Result<PathBuf> {
    // Strip invalid chars (CR/LF/NUL)
    validate_path_characters(path)?;

    // Reject symlinks before any other checks to prevent extension bypass.
    reject_symlink(path, "audio")?;

    // Check exists and is regular file
    validate_file_existence_and_type(path)?;

    // Extension whitelist check
    validate_audio_extension(path)?;

    // Canonicalize to prevent path traversal and resolve symlinks
    let canonical = path.canonicalize().map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot canonicalize path '{}': {}",
            sanitize_path_for_display(path),
            e
        ))
    })?;

    Ok(canonical)
}

/// Validate a single input image file path and return its canonical absolute path.
/// Used for cover art loading to prevent path traversal attacks.
pub fn validate_input_image_path(path: &Path) -> Result<PathBuf> {
    validate_path_characters(path)?;
    reject_symlink(path, "image")?;
    validate_file_existence_and_type(path)?;
    validate_image_extension(path)?;

    let canonical = path.canonicalize().map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot canonicalize path '{}': {}",
            sanitize_path_for_display(path),
            e
        ))
    })?;

    Ok(canonical)
}

/// Rejects symlink inputs to avoid extension whitelist bypass via symlink targets.
fn reject_symlink(path: &Path, kind: &str) -> Result<()> {
    let is_symlink = path
        .symlink_metadata()
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false);

    if is_symlink {
        return Err(AppError::InvalidInput(format!(
            "Symlinks are not supported for {kind} files. Please use the original file path."
        )));
    }

    Ok(())
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
            sanitize_path_for_display(path),
            e
        ))
    })?;

    if !metadata.is_file() {
        return Err(AppError::FileValidation(format!(
            "Path is not a regular file: {}",
            sanitize_path_for_display(path)
        )));
    }

    Ok(())
}

/// Validates audio file extension against whitelist
fn validate_audio_extension(path: &Path) -> Result<()> {
    super::extensions::audio_format_for_path(path)?;
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
