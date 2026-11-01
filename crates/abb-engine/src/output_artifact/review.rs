use super::types::{CollisionPolicy, PlannedOutputAction, ResolvedOutputPlan};
use crate::errors::{sanitize_path_for_display, AppError, Result};

pub(crate) struct OutputPlanReview<'a> {
    pub(crate) expected_signature: Option<&'a str>,
    pub(crate) current_signature: &'a str,
    pub(crate) collision_policy: CollisionPolicy,
}

fn output_plan_review_message(output: &ResolvedOutputPlan) -> String {
    abb_output_artifact_core::output_plan_review_message(
        output,
        &sanitize_path_for_display(&output.requested_path),
    )
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
        .find(|output| abb_output_artifact_core::plan_is_hard_block(output))
    {
        return Err(AppError::FileValidation(output_plan_review_message(output)));
    }

    if review.expected_signature.is_none() {
        if let Some(output) = outputs
            .iter()
            .copied()
            .find(|output| output.action == PlannedOutputAction::ReviewRequired)
        {
            return Err(AppError::FileValidation(output_plan_review_message(output)));
        }

        if outputs.iter().any(|output| output.collision.is_some()) {
            return Err(AppError::FileValidation(
                "Output collisions require review before processing. Open the collision dialog and choose how to continue."
                    .to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
