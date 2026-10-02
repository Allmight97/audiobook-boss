//! Finalization stage — metadata writing and processor lifecycle completion.
//!
//! Output artifact commit policy lives in `output_artifact`; this module
//! sequences processor progress, cleanup registration, and success emission.
//! Native muxing writes initial metadata; Metadata's finish step completes
//! the artifact's tags and chapter checks before commit.
//! Cancellation checks run after each sub-step to avoid unnecessary work.

use std::path::PathBuf;

use crate::audio::CleanupGuard;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::AudiobookMetadata;
use crate::output_artifact::{
    commit_output_artifact, finalized_output_success, OutputCommitRequest,
};
use crate::processing::ProcessingContext;

use super::ProcessingWorkflow;

fn ensure_not_cancelled_before_commit(context: &ProcessingContext) -> Result<()> {
    if context.is_cancelled() {
        context
            .new_emitter()
            .emit_cancelled("Processing was cancelled");
        return Err(AppError::cancelled());
    }

    Ok(())
}

pub(super) fn complete_staged_output(
    context: &ProcessingContext,
    staged_output: PathBuf,
    cleanup_guard: &mut CleanupGuard,
) -> Result<String> {
    ensure_not_cancelled_before_commit(context)?;
    super::run_diagnostics::log_output_observation(context, &staged_output);

    let ui = context.new_emitter();
    ui.emit_cleanup("Cleaning up...");

    let commit_request =
        OutputCommitRequest::new(context.output.final_path(), context.output.commit_action());
    log::info!("media_handoff stage=publish session_id={} job_id={} source_artifact={} destination_artifact={} destination_state={:?}",
        context.session.id(), context.job_id.as_deref().unwrap_or("unscoped"),
        crate::diagnostics::artifact_id(&staged_output), crate::diagnostics::artifact_id(context.output.final_path()),
        crate::diagnostics::file_state(context.output.final_path()));
    let commit = || {
        crate::diagnostics::stage("publish", &staged_output, || {
            commit_output_artifact(commit_request, staged_output.clone(), cleanup_guard, || {
                context.is_cancelled()
            })
        })
        .map(|outcome| {
            let published = outcome.final_output.clone();
            (outcome, published)
        })
    };
    // An export title publishes through its output record, which writes an
    // edit accepted meanwhile to the staged file first.
    let outcome = match &context.title_output {
        Some(title) => title.publish(&staged_output, context.output.final_path(), commit)?,
        None => commit()?.0,
    };
    log::info!(
        "media_handoff stage=published job_id={} output_path={:?} artifact={} {}",
        context.job_id.as_deref().unwrap_or("unscoped"),
        outcome.final_output,
        crate::diagnostics::artifact_id(&outcome.final_output),
        crate::diagnostics::file_state(&outcome.final_output)
    );
    log::info!(
        "✓ File moved successfully to: {}",
        sanitize_path_for_display(&outcome.final_output)
    );
    let success = finalized_output_success(
        context.output.output_kind(),
        &outcome.final_output,
        outcome.cancelled,
        outcome.cleanup_warning.as_deref(),
    );
    ui.emit_complete(success.ui_message);
    log::info!("🎉 {}", success.result_message);
    Ok(success.result_message)
}

/// Completes processing: move to final path + cleanup + final UI emit
pub(crate) fn complete_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
) -> Result<String> {
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(workflow.temp_dir);
    cleanup_guard.add_path(&merged_output);

    complete_staged_output(context, merged_output, &mut cleanup_guard)
}

/// Finalize pipeline: metadata + completion
pub(crate) fn finalize_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<&crate::metadata::PassthroughMetadata>,
) -> Result<String> {
    let ui = context.new_emitter();
    let chapters = passthrough
        .filter(|_| context.preview.is_none())
        .map(|value| value.chapters.as_slice());
    let rewrote =
        crate::metadata::finish_artifact_tags(&merged_output, metadata.as_ref(), chapters, || {
            ui.emit_metadata_start("Writing metadata...")
        })?;
    if rewrote {
        ui.emit_finalizing("Finalizing...");
    }
    complete_processing(context, workflow, merged_output)
}

#[cfg(test)]
#[path = "finalize_tests.rs"]
mod tests;
