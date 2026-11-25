use super::collision::{detect_output_collision, next_rename_candidate, OutputCollisionCache};
use super::derive_output_artifact_path;
use super::types::{CollisionPolicy, OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use crate::errors::{AppError, Result};
use abb_output_artifact_core::RenameOption;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct OutputPlanLedger {
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
    let hard_block = collision
        .as_ref()
        .is_some_and(|value| value.kind.is_hard_block());
    let claimed_conflict = collision.as_ref().is_some_and(|value| {
        value
            .conflicting_path
            .as_ref()
            .is_some_and(|path| claimed.contains(path))
    });
    let rename = if collision.is_some() && !hard_block {
        next_rename_candidate(&requested_path, claimed, source_paths, cache)?
    } else {
        RenameOption::Unneeded
    };

    let action = abb_output_artifact_core::resolve_action(
        collision.as_ref().map(|value| value.kind),
        claimed_conflict,
        policy,
    );

    let resolved_path = match action {
        PlannedOutputAction::RenameNew => renamed_path(&rename, &requested_path)?,
        _ => requested_path.clone(),
    };

    Ok(ResolvedOutputPlan {
        kind,
        requested_path,
        resolved_path,
        rename,
        collision,
        action,
    })
}

fn renamed_path(rename: &RenameOption, requested_path: &Path) -> Result<PathBuf> {
    match rename {
        RenameOption::Free(path) => Ok(path.clone()),
        RenameOption::TooLong => Err(AppError::InvalidInput(
            "A renamed output name would be too long for a file name. Shorten the title or the naming template, or choose another way to handle the collision.".to_string(),
        )),
        RenameOption::Unneeded => Err(AppError::General(
            "A rename was planned for an output with no collision.".to_string(),
        )),
        RenameOption::Exhausted => {
            let stem = requested_path
                .file_stem()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            let ext = requested_path
                .extension()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            Err(AppError::FileValidation(format!(
                "Every renamed name from \"{stem}-1.{ext}\" to \"{stem}-99.{ext}\" is already taken. Choose another way to handle the collision, or change the title or naming template."
            )))
        }
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
