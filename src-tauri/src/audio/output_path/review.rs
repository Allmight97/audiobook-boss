use super::plan::{action_requires_output_write, plan_is_hard_block};
use super::types::{
    CollisionPolicy, OutputCollisionKind, OutputReviewRequirement, PlannedOutputAction,
    ResolvedOutputPlan,
};
use crate::errors::{sanitize_path_for_display, AppError, Result};

pub(crate) struct OutputPlanReview<'a> {
    pub(crate) expected_signature: Option<&'a str>,
    pub(crate) current_signature: &'a str,
    pub(crate) collision_policy: CollisionPolicy,
}

pub(crate) fn ensure_output_parent_dirs<'a>(
    outputs: impl IntoIterator<Item = &'a ResolvedOutputPlan>,
) -> Result<()> {
    for output in outputs {
        if !action_requires_output_write(output.action) {
            continue;
        }
        if let Some(parent) = output.resolved_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                AppError::FileValidation(format!(
                    "Cannot create output directory '{}': {}",
                    sanitize_path_for_display(parent),
                    error
                ))
            })?;
        }
    }

    Ok(())
}

fn output_plan_review_message(output: &ResolvedOutputPlan) -> String {
    let destination = sanitize_path_for_display(&output.requested_path);
    match output.collision.as_ref().map(|value| value.kind) {
        Some(OutputCollisionKind::SourceDestinationOverlap)
        | Some(OutputCollisionKind::CanonicalPathOverlap) => format!(
            "Output path '{}' targets an input source file. Choose a different destination.",
            destination
        ),
        _ => format!(
            "Output collision review is required for '{}'. Re-run preflight and choose how to handle the collision.",
            destination
        ),
    }
}

pub(super) fn output_review_requirement(
    output: &ResolvedOutputPlan,
) -> Option<OutputReviewRequirement> {
    (output.action == PlannedOutputAction::ReviewRequired).then(|| OutputReviewRequirement {
        can_proceed: !plan_is_hard_block(output),
        message: output_plan_review_message(output),
    })
}

pub(crate) fn enforce_output_plan_review<'a>(
    review: OutputPlanReview<'_>,
    outputs: impl IntoIterator<Item = &'a ResolvedOutputPlan>,
) -> Result<()> {
    let outputs = outputs.into_iter().collect::<Vec<_>>();

    if let Some(signature) = review.expected_signature {
        if signature != review.current_signature {
            return Err(AppError::FileValidation(
                "Output collision state changed after review. Review the collision dialog and try again."
                    .to_string(),
            ));
        }
    }

    if review.collision_policy != CollisionPolicy::Fail && review.expected_signature.is_none() {
        return Err(AppError::InvalidInput(
            "Collision policy selections require a reviewed preflight plan.".to_string(),
        ));
    }

    if let Some(output) = outputs
        .iter()
        .copied()
        .find(|output| plan_is_hard_block(output))
    {
        return Err(AppError::FileValidation(output_plan_review_message(output)));
    }

    if let Some(output) = outputs
        .iter()
        .copied()
        .find(|output| output.action == PlannedOutputAction::ReviewRequired)
    {
        return Err(AppError::FileValidation(output_plan_review_message(output)));
    }

    if review.expected_signature.is_none()
        && outputs.iter().any(|output| output.collision.is_some())
    {
        return Err(AppError::FileValidation(
            "Output collisions require review before processing. Open the collision dialog and choose how to continue."
                .to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::output_path::{OutputCollision, OutputKind};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn output_plan(action: PlannedOutputAction, path: impl Into<PathBuf>) -> ResolvedOutputPlan {
        let path = path.into();
        ResolvedOutputPlan {
            kind: OutputKind::Final,
            requested_path: path.clone(),
            resolved_path: path,
            rename_candidate: None,
            collision: None,
            action,
        }
    }

    fn review<'a>(
        expected_signature: Option<&'a str>,
        current_signature: &'a str,
        collision_policy: CollisionPolicy,
    ) -> OutputPlanReview<'a> {
        OutputPlanReview {
            expected_signature,
            current_signature,
            collision_policy,
        }
    }

    #[test]
    fn rejects_stale_review_signature() {
        let temp_dir = TempDir::new().expect("temp dir");
        let output = output_plan(PlannedOutputAction::Write, temp_dir.path().join("book.m4b"));

        let err = enforce_output_plan_review(
            review(Some("old"), "new", CollisionPolicy::ReplaceExisting),
            [&output],
        )
        .expect_err("stale signature should fail");

        assert!(err
            .to_string()
            .contains("Output collision state changed after review"));
    }

    #[test]
    fn rejects_policy_selection_without_signature() {
        let temp_dir = TempDir::new().expect("temp dir");
        let output = output_plan(PlannedOutputAction::Write, temp_dir.path().join("book.m4b"));

        let err = enforce_output_plan_review(
            review(None, "sig", CollisionPolicy::ReplaceExisting),
            [&output],
        )
        .expect_err("policy without signature should fail");

        assert!(err
            .to_string()
            .contains("Collision policy selections require"));
    }

    #[test]
    fn rejects_hard_block_even_with_review_signature() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut output = output_plan(
            PlannedOutputAction::ReviewRequired,
            temp_dir.path().join("book.m4b"),
        );
        output.collision = Some(OutputCollision {
            kind: OutputCollisionKind::SourceDestinationOverlap,
            conflicting_path: Some(output.requested_path.clone()),
            detail: None,
        });

        let err = enforce_output_plan_review(
            review(Some("sig"), "sig", CollisionPolicy::ReplaceExisting),
            [&output],
        )
        .expect_err("hard block should fail");

        assert!(err.to_string().contains("targets an input source file"));
    }

    #[test]
    fn creates_parent_dirs_for_writable_outputs_only() {
        let temp_dir = TempDir::new().expect("temp dir");
        let writable = output_plan(
            PlannedOutputAction::Write,
            temp_dir.path().join("created").join("book.m4b"),
        );
        let skipped = output_plan(
            PlannedOutputAction::SkipExisting,
            temp_dir.path().join("skipped").join("book.m4b"),
        );

        ensure_output_parent_dirs([&writable, &skipped]).expect("parent dirs");

        assert!(temp_dir.path().join("created").exists());
        assert!(!temp_dir.path().join("skipped").exists());
    }
}
