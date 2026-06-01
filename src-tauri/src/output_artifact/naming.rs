use super::types::OutputNamingConfig;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::NamingMetadata;
use std::path::{Path, PathBuf};

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_output_path(
    base_dir: &Path,
    metadata: Option<&NamingMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let full_path = build_output_path_preview(base_dir, metadata, naming, source_path)?;
    if let Some(dir) = full_path.parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir).map_err(|e| {
                AppError::FileValidation(format!(
                    "Failed to create output directory '{}': {}",
                    sanitize_path_for_display(dir),
                    e
                ))
            })?;
        }
    }
    crate::audio::validate_output_path(&full_path)?;
    Ok(full_path)
}

pub fn build_output_path_preview(
    base_dir: &Path,
    metadata: Option<&NamingMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    abb_output_artifact_core::build_output_path_preview(base_dir, metadata, naming, source_path)
        .map_err(Into::into)
}
