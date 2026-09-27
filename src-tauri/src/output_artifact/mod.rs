mod collision;
mod commit;
mod parent_dirs;
mod plan;
mod review;
mod supplemental;
mod types;

#[cfg(test)]
mod contract_tests;

use crate::errors::{AppError, Result};
use crate::metadata::NamingMetadata;
use std::path::{Path, PathBuf};

pub(crate) use commit::{commit_output_artifact, finalized_output_success, OutputCommitRequest};
pub(crate) use parent_dirs::{ensure_output_parent_dirs, OutputParentDirCleanup};
pub(crate) use plan::OutputPlanLedger;
pub(crate) use review::{enforce_output_plan_review, OutputPlanReview};
#[cfg(test)]
pub(crate) use supplemental::{
    commit_supplemental_output_asset, SupplementalOutputAssetCommitRequest,
};
pub(crate) use supplemental::{
    commit_supplemental_output_assets_for_output, SupplementalOutputAssetsCommitRequest,
};
pub(crate) use types::ResolvedOutputPlan;
pub use types::{
    CollisionPolicy, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    OutputNamingConfig, OutputReviewRequirement, PlannedOutput, PlannedOutputAction,
};

impl From<abb_output_artifact_core::OutputArtifactCoreError> for AppError {
    fn from(error: abb_output_artifact_core::OutputArtifactCoreError) -> Self {
        match error {
            abb_output_artifact_core::OutputArtifactCoreError::InvalidInput(message) => {
                AppError::InvalidInput(message)
            }
            abb_output_artifact_core::OutputArtifactCoreError::FileValidation(message) => {
                AppError::FileValidation(message)
            }
        }
    }
}

pub(crate) fn derive_output_artifact_path(
    requested_final_path: &Path,
    kind: OutputKind,
) -> Result<PathBuf> {
    abb_output_artifact_core::derive_output_artifact_path(requested_final_path, kind)
        .map_err(Into::into)
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
