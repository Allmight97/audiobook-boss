use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{sanitize_path_for_display, AppError, Result};

pub fn validate_input_audio_path(path: &Path) -> Result<PathBuf> {
    validate_path_characters(path)?;

    reject_symlink(path, "audio")?;

    validate_file_existence_and_type(path)?;

    validate_audio_extension(path)?;

    let canonical = path.canonicalize().map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot canonicalize path '{}': {}",
            sanitize_path_for_display(path),
            e
        ))
    })?;

    Ok(canonical)
}

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

pub(super) fn validate_input_directory_path(path: &Path) -> Result<PathBuf> {
    validate_path_characters(path)?;
    reject_symlink(path, "directory")?;

    let metadata = fs::metadata(path).map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot read directory metadata for '{}': {}",
            sanitize_path_for_display(path),
            e
        ))
    })?;

    if !metadata.is_dir() {
        return Err(AppError::FileValidation(format!(
            "Path is not a directory: {}",
            sanitize_path_for_display(path)
        )));
    }

    path.canonicalize().map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot canonicalize directory '{}': {}",
            sanitize_path_for_display(path),
            e
        ))
    })
}

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

fn validate_path_characters(path: &Path) -> Result<()> {
    if path_contains_invalid_character(path) {
        return Err(AppError::FileValidation(
            "Path contains invalid characters (CR/LF/NUL)".to_string(),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn path_contains_invalid_character(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str()
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'\n' | b'\r' | b'\0'))
}

#[cfg(windows)]
fn path_contains_invalid_character(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .any(|unit| matches!(unit, 0x000A | 0x000D | 0x0000))
}

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

fn validate_audio_extension(path: &Path) -> Result<()> {
    super::extensions::audio_format_for_path(path)?;
    Ok(())
}

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
#[path = "path_validation_tests.rs"]
mod tests;
