use super::types::OutputNamingConfig;
use crate::errors::Result;
use crate::metadata::NamingMetadata;
use std::path::{Path, PathBuf};

pub fn build_output_path_preview(
    base_dir: &Path,
    metadata: Option<&NamingMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    abb_output_artifact_core::build_output_path_preview(base_dir, metadata, naming, source_path)
        .map_err(Into::into)
}

/// Keep the source container extension after applying the shared library naming rules.
pub(crate) fn preserve_source_extension(output: PathBuf, source: &Path) -> Result<PathBuf> {
    let extension = source.extension().ok_or_else(|| {
        crate::errors::AppError::InvalidInput(
            "Keep original audio requires a source file extension.".into(),
        )
    })?;
    Ok(output.with_extension(extension))
}
