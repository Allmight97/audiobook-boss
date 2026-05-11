mod artifact;
mod collision;
mod commit;
mod naming;
mod plan;
mod review;
mod types;

#[cfg(test)]
mod contract_tests;

pub(crate) use artifact::derive_output_artifact_path;
pub(crate) use commit::{commit_output_artifact, finalized_output_success};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use naming::build_output_path;
pub use naming::build_output_path_preview;
#[allow(unused_imports)]
pub(crate) use plan::{action_requires_output_write, plan_is_hard_block, OutputPlanLedger};
pub(crate) use review::{enforce_output_plan_review, ensure_output_parent_dirs, OutputPlanReview};
pub use types::{
    CollisionPolicy, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    OutputNamingConfig, OutputReviewRequirement, PlannedOutput, PlannedOutputAction,
};
#[allow(unused_imports)]
pub(crate) use types::{OutputCollision, ResolvedOutputPlan};
