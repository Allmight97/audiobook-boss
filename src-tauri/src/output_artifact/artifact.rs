use super::types::OutputKind;
use crate::errors::Result;
use std::path::{Path, PathBuf};

pub(crate) fn derive_output_artifact_path(
    requested_final_path: &Path,
    kind: OutputKind,
) -> Result<PathBuf> {
    abb_output_artifact_core::derive_output_artifact_path(requested_final_path, kind)
        .map_err(Into::into)
}
