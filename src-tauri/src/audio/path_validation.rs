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
