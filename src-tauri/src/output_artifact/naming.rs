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
