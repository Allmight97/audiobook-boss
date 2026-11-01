use super::collision::{detect_output_collision, next_rename_candidate, OutputCollisionCache};
use super::derive_output_artifact_path;
use super::types::{
    CollisionPolicy, OutputCollisionKind, OutputKind, PlannedOutputAction, ResolvedOutputPlan,
};
use crate::errors::{AppError, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub(crate) struct OutputPlanLedger {
    claimed: HashSet<PathBuf>,
    collision_cache: OutputCollisionCache,
}

impl OutputPlanLedger {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn resolve(
        &mut self,
        requested_final_path: &Path,
        kind: OutputKind,
        policy: CollisionPolicy,
        source_paths: &[PathBuf],
    ) -> Result<ResolvedOutputPlan> {
        let requested = derive_output_artifact_path(requested_final_path, kind)?;
        self.resolve_requested(&requested, kind, policy, source_paths)
    }

    /// Refreshes collisions at the already-derived artifact path; a preview
    /// suffix must not be applied for a second time during execution.
    pub(crate) fn refresh(
        &mut self,
        planned: &ResolvedOutputPlan,
        policy: CollisionPolicy,
        source_paths: &[PathBuf],
    ) -> Result<ResolvedOutputPlan> {
        self.resolve_requested(&planned.requested_path, planned.kind, policy, source_paths)
    }

    fn resolve_requested(
        &mut self,
        requested: &Path,
        kind: OutputKind,
        policy: CollisionPolicy,
        source_paths: &[PathBuf],
    ) -> Result<ResolvedOutputPlan> {
        self.collision_cache.cache_source_paths(source_paths);
        let plan = resolve_output_plan_with_cache(
            requested,
            kind,
            policy,
            &self.claimed,
            source_paths,
            &mut self.collision_cache,
        )?;
        if abb_output_artifact_core::action_requires_output_write(plan.action) {
            self.claimed.insert(plan.resolved_path.clone());
        }
        Ok(plan)
    }

    #[cfg(test)]
    fn claimed_paths(&self) -> &HashSet<PathBuf> {
        &self.claimed
    }
}

#[cfg(test)]
pub(crate) fn resolve_output_plan(
    requested_final_path: &Path,
    kind: OutputKind,
    policy: CollisionPolicy,
    claimed: &HashSet<PathBuf>,
    source_paths: &[PathBuf],
) -> Result<ResolvedOutputPlan> {
    let mut cache = OutputCollisionCache::default();
    cache.cache_source_paths(source_paths);
    resolve_output_plan_with_cache(
        &derive_output_artifact_path(requested_final_path, kind)?,
        kind,
        policy,
        claimed,
        source_paths,
        &mut cache,
    )
}

fn resolve_output_plan_with_cache(
    requested_final_path: &Path,
    kind: OutputKind,
    policy: CollisionPolicy,
    claimed: &HashSet<PathBuf>,
    source_paths: &[PathBuf],
    cache: &mut OutputCollisionCache,
) -> Result<ResolvedOutputPlan> {
    abb_output_artifact_core::check_output_name_lengths(requested_final_path)?;
    let requested_path = requested_final_path.to_path_buf();
    let collision = detect_output_collision(&requested_path, claimed, source_paths, cache)?;
    let hard_block = abb_output_artifact_core::collision_is_hard_block(collision.as_ref());
    let batch_like_collision = collision
        .as_ref()
        .map(|value| match value.kind {
            OutputCollisionKind::BatchDuplicate => true,
            OutputCollisionKind::CaseInsensitiveMatch => value
                .conflicting_path
                .as_ref()
                .is_some_and(|path| claimed.contains(path)),
            _ => false,
        })
        .unwrap_or(false);
    let rename_candidate = if collision.is_some() && !hard_block {
        next_rename_candidate(&requested_path, claimed, source_paths, cache)?
    } else {
        None
    };

    let action = match (
        collision.is_some(),
        hard_block,
        batch_like_collision,
        policy,
    ) {
        (false, _, _, _) => PlannedOutputAction::Write,
        (true, true, _, _) => PlannedOutputAction::ReviewRequired,
        (true, false, _, CollisionPolicy::Fail) => PlannedOutputAction::ReviewRequired,
        (true, false, true, CollisionPolicy::ReplaceExisting) => PlannedOutputAction::RenameNew,
        (true, false, _, CollisionPolicy::ReplaceExisting) => PlannedOutputAction::ReplaceExisting,
        (true, false, _, CollisionPolicy::RenameNew) => PlannedOutputAction::RenameNew,
        (true, false, _, CollisionPolicy::SkipExisting) => PlannedOutputAction::SkipExisting,
    };

    let resolved_path = match action {
        PlannedOutputAction::RenameNew => rename_candidate.clone().ok_or_else(|| {
            AppError::InvalidInput(
                "A renamed output name would be too long for a file name. Shorten the title or the naming template, or replace the existing file.".to_string(),
            )
        })?,
        _ => requested_path.clone(),
    };

    Ok(ResolvedOutputPlan {
        kind,
        requested_path,
        resolved_path,
        rename_candidate,
        collision,
        action,
    })
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
