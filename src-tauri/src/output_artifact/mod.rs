mod artifact;
mod collision;
mod commit;
pub(crate) mod naming;
mod parent_dirs;
mod plan;
mod review;
mod supplemental;
mod types;

#[cfg(test)]
mod contract_tests;

use crate::errors::AppError;

pub(crate) use artifact::derive_output_artifact_path;
pub(crate) use commit::{commit_output_artifact, finalized_output_success, OutputCommitRequest};
pub use naming::build_output_path_preview;
pub(crate) use parent_dirs::{ensure_output_parent_dirs, OutputParentDirCleanup};
pub(crate) use plan::OutputPlanLedger;
pub(crate) use review::{enforce_output_plan_review, OutputPlanReview};
pub(crate) use supplemental::{
    commit_supplemental_output_asset, commit_supplemental_output_assets_for_output,
    SupplementalOutputAssetCommitRequest, SupplementalOutputAssetsCommitRequest,
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
