use super::artifact::derive_output_artifact_path;
use super::collision::{detect_output_collision, next_rename_candidate, OutputCollisionCache};
use super::types::{
    CollisionPolicy, OutputCollision, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    PlannedOutput, PlannedOutputAction, ResolvedOutputPlan,
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
        self.collision_cache.cache_source_paths(source_paths);
        let plan = resolve_output_plan_with_cache(
            requested_final_path,
            kind,
            policy,
            &self.claimed,
            source_paths,
            &mut self.collision_cache,
        )?;
        if action_requires_output_write(plan.action) {
            self.claimed.insert(plan.resolved_path.clone());
        }
        Ok(plan)
    }

    #[cfg(test)]
    fn claimed_paths(&self) -> &HashSet<PathBuf> {
        &self.claimed
    }
}

pub(crate) fn action_requires_output_write(action: PlannedOutputAction) -> bool {
    matches!(
        action,
        PlannedOutputAction::Write
            | PlannedOutputAction::ReplaceExisting
            | PlannedOutputAction::RenameNew
    )
}

pub(crate) fn collision_is_hard_block(collision: Option<&OutputCollision>) -> bool {
    collision.is_some_and(|value| {
        matches!(
            value.kind,
            OutputCollisionKind::SourceDestinationOverlap
                | OutputCollisionKind::CanonicalPathOverlap
        )
    })
}

pub(crate) fn plan_is_hard_block(plan: &ResolvedOutputPlan) -> bool {
    matches!(plan.action, PlannedOutputAction::ReviewRequired)
        && collision_is_hard_block(plan.collision.as_ref())
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
        requested_final_path,
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
    let requested_path = derive_output_artifact_path(requested_final_path, kind)?;
    let collision = detect_output_collision(&requested_path, claimed, source_paths, cache)?;
    let hard_block = collision
        .as_ref()
        .map(|value| {
            matches!(
                value.kind,
                OutputCollisionKind::SourceDestinationOverlap
                    | OutputCollisionKind::CanonicalPathOverlap
            )
        })
        .unwrap_or(false);
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
        Some(next_rename_candidate(
            &requested_path,
            claimed,
            source_paths,
            cache,
        )?)
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
        PlannedOutputAction::RenameNew => rename_candidate
            .clone()
            .ok_or_else(|| AppError::General("Missing rename candidate".to_string()))?,
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

impl OutputCollision {
    pub(crate) fn to_public(&self) -> OutputCollisionInfo {
        OutputCollisionInfo {
            kind: self.kind,
            conflicting_path: self
                .conflicting_path
                .as_ref()
                .map(|value| value.display().to_string()),
            detail: self.detail.clone(),
        }
    }
}

impl ResolvedOutputPlan {
    pub(crate) fn to_public(
        &self,
        input_index: Option<usize>,
        input_path: Option<&Path>,
    ) -> PlannedOutput {
        PlannedOutput {
            input_index,
            input_path: input_path.map(|value| value.display().to_string()),
            kind: self.kind,
            requested_path: self.requested_path.display().to_string(),
            resolved_path: self.resolved_path.display().to_string(),
            rename_candidate: self
                .rename_candidate
                .as_ref()
                .map(|value| value.display().to_string()),
            collision: self.collision.as_ref().map(OutputCollision::to_public),
            review: super::review::output_review_requirement(self),
            action: self.action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_output_plan, OutputPlanLedger};
    use crate::output_artifact::{
        CollisionPolicy, OutputCollisionKind, OutputKind, PlannedOutputAction,
    };
    use std::collections::HashSet;
    use std::fs::write;
    use tempfile::TempDir;

    #[test]
    fn resolve_output_plan_marks_existing_file_for_review_by_default() {
        let temp_dir = TempDir::new().expect("temp dir");
        let existing_path = temp_dir.path().join("book.m4b");
        write(&existing_path, b"existing").expect("write existing file");

        let plan = resolve_output_plan(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::Fail,
            &HashSet::new(),
            &[],
        )
        .expect("plan");

        assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
        assert_eq!(
            plan.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::ExistingFile)
        );
        assert_eq!(
            plan.rename_candidate,
            Some(temp_dir.path().join("book-1.m4b"))
        );
    }

    #[test]
    fn resolve_output_plan_renames_batch_duplicates() {
        let temp_dir = TempDir::new().expect("temp dir");
        let requested_path = temp_dir.path().join("book.m4b");
        let mut claimed = HashSet::new();
        claimed.insert(requested_path.clone());

        let plan = resolve_output_plan(
            &requested_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
            &claimed,
            &[],
        )
        .expect("resolved path");

        assert_eq!(plan.action, PlannedOutputAction::RenameNew);
        assert_eq!(plan.resolved_path, temp_dir.path().join("book-1.m4b"));
        assert_eq!(
            plan.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::BatchDuplicate)
        );
    }

    #[test]
    fn resolve_output_plan_blocks_source_destination_overlap() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source_path = temp_dir.path().join("input.m4b");
        write(&source_path, b"input").expect("write input");

        let plan = resolve_output_plan(
            &source_path,
            OutputKind::Final,
            CollisionPolicy::ReplaceExisting,
            &HashSet::new(),
            std::slice::from_ref(&source_path),
        )
        .expect("plan");

        assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
        assert_eq!(
            plan.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::SourceDestinationOverlap)
        );
        assert_eq!(plan.rename_candidate, None);
    }

    #[test]
    fn resolve_output_plan_skips_existing_when_requested() {
        let temp_dir = TempDir::new().expect("temp dir");
        let existing_path = temp_dir.path().join("book.m4b");
        write(&existing_path, b"existing").expect("write existing file");

        let plan = resolve_output_plan(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::SkipExisting,
            &HashSet::new(),
            &[],
        )
        .expect("plan");

        assert_eq!(plan.action, PlannedOutputAction::SkipExisting);
        assert_eq!(plan.resolved_path, existing_path);
    }

    #[test]
    fn output_plan_ledger_preserves_duplicate_ordering() {
        let temp_dir = TempDir::new().expect("temp dir");
        let requested_path = temp_dir.path().join("book.m4b");
        let source_paths = Vec::new();
        let mut ledger = OutputPlanLedger::new();

        let first = ledger
            .resolve(
                &requested_path,
                OutputKind::Final,
                CollisionPolicy::RenameNew,
                &source_paths,
            )
            .expect("first plan");
        let second = ledger
            .resolve(
                &requested_path,
                OutputKind::Final,
                CollisionPolicy::RenameNew,
                &source_paths,
            )
            .expect("second plan");
        let third = ledger
            .resolve(
                &requested_path,
                OutputKind::Final,
                CollisionPolicy::RenameNew,
                &source_paths,
            )
            .expect("third plan");

        assert_eq!(first.action, PlannedOutputAction::Write);
        assert_eq!(first.resolved_path, requested_path);
        assert_eq!(second.action, PlannedOutputAction::RenameNew);
        assert_eq!(second.resolved_path, temp_dir.path().join("book-1.m4b"));
        assert_eq!(third.action, PlannedOutputAction::RenameNew);
        assert_eq!(third.resolved_path, temp_dir.path().join("book-2.m4b"));
        assert_eq!(ledger.claimed_paths().len(), 3);
    }

    #[test]
    fn output_plan_ledger_does_not_claim_skipped_outputs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let existing_path = temp_dir.path().join("book.m4b");
        write(&existing_path, b"existing").expect("write existing file");
        let source_paths = Vec::new();
        let mut ledger = OutputPlanLedger::new();

        let first = ledger
            .resolve(
                &existing_path,
                OutputKind::Final,
                CollisionPolicy::SkipExisting,
                &source_paths,
            )
            .expect("first plan");
        assert_eq!(first.action, PlannedOutputAction::SkipExisting);
        assert!(ledger.claimed_paths().is_empty());

        let second = ledger
            .resolve(
                &existing_path,
                OutputKind::Final,
                CollisionPolicy::RenameNew,
                &source_paths,
            )
            .expect("second plan");

        assert_eq!(second.action, PlannedOutputAction::RenameNew);
        assert_eq!(second.resolved_path, temp_dir.path().join("book-1.m4b"));
        assert_eq!(ledger.claimed_paths().len(), 1);
    }

    #[test]
    fn output_plan_ledger_detects_preview_suffix_collisions() {
        let temp_dir = TempDir::new().expect("temp dir");
        let requested_path = temp_dir.path().join("book.m4b");
        let preview_path = temp_dir.path().join("book.preview.m4b");
        write(&preview_path, b"existing preview").expect("write existing preview");
        let source_paths = Vec::new();
        let mut ledger = OutputPlanLedger::new();

        let plan = ledger
            .resolve(
                &requested_path,
                OutputKind::Preview,
                CollisionPolicy::Fail,
                &source_paths,
            )
            .expect("preview plan");

        assert_eq!(plan.requested_path, preview_path);
        assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
        assert_eq!(
            plan.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::ExistingFile)
        );
        assert_eq!(
            plan.rename_candidate,
            Some(temp_dir.path().join("book.preview-1.m4b"))
        );
    }
}
