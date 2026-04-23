mod artifact;
mod collision;
mod naming;
mod plan;
mod types;

pub(crate) use artifact::derive_output_artifact_path;
pub(crate) use collision::path_entry_exists;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use naming::build_output_path;
pub use naming::build_output_path_preview;
#[allow(unused_imports)]
pub(crate) use plan::{action_requires_output_write, plan_is_hard_block, OutputPlanLedger};
pub use types::{
    CollisionPolicy, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    OutputNamingConfig, PlannedOutput, PlannedOutputAction,
};
#[allow(unused_imports)]
pub(crate) use types::{OutputCollision, ResolvedOutputPlan};
