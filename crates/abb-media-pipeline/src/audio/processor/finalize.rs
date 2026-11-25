use std::path::PathBuf;

use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::execution::{ProcessingContext, PublishedOutput};
use crate::metadata::AudiobookMetadata;
use crate::output_artifact::{
    cleanup_failure_warning, commit_output_artifact, finalized_output_success, OutputCommitRequest,
};

use super::workspace::Workspace;
use super::ProcessingWorkflow;

fn ensure_not_cancelled_before_commit(context: &ProcessingContext) -> Result<()> {
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }

    Ok(())
}

fn settle_published(
    path: PathBuf,
    workspace: &mut Workspace,
    is_cancelled: impl FnOnce() -> bool,
) -> PublishedOutput {
    let cancelled = is_cancelled();
    let cleanup_warning = workspace
        .published(&path)
        .err()
        .map(|error| cleanup_failure_warning(&error));
    PublishedOutput {
        path,
        cancelled,
        cleanup_warning,
    }
}

pub(super) fn complete_staged_output(
    context: &ProcessingContext,
    staged_output: PathBuf,
    workspace: &mut Workspace,
) -> Result<String> {
    ensure_not_cancelled_before_commit(context)?;
    super::run_diagnostics::log_output_observation(context, &staged_output);

    log::info!("media_handoff stage=publish session_id={} job_id={} source_artifact={} destination_artifact={} destination_state={:?}",
        context.session.id(), context.identity.job_id.as_deref().unwrap_or("unscoped"),
        crate::diagnostics::artifact_id(&staged_output), crate::diagnostics::artifact_id(context.output.final_path()),
        crate::diagnostics::file_state(context.output.final_path()));
    let mut commit = || {
        ensure_not_cancelled_before_commit(context)?;
        crate::diagnostics::stage("publish", &staged_output, || {
            let request = OutputCommitRequest::new(
                context.output.final_path(),
                context.output.commit_action(),
            );
            let path = commit_output_artifact(request, staged_output.clone())?;
            Ok(settle_published(path, workspace, || context.is_cancelled()))
        })
    };
    let outcome = match &context.publish_gate {
        Some(gate) => gate.publish(&staged_output, context.output.final_path(), &mut commit)?,
        None => commit()?,
    };
    log::info!(
        "media_handoff stage=published job_id={} output_path={:?} artifact={} {}",
        context.identity.job_id.as_deref().unwrap_or("unscoped"),
        outcome.path,
        crate::diagnostics::artifact_id(&outcome.path),
        crate::diagnostics::file_state(&outcome.path)
    );
    log::info!(
        "✓ File moved successfully to: {}",
        sanitize_path_for_display(&outcome.path)
    );
    let success = finalized_output_success(
        context.output.output_kind(),
        &outcome.path,
        outcome.cancelled,
        outcome.cleanup_warning.as_deref(),
    );
    context.new_emitter().emit_complete(success.ui_message);
    log::info!("🎉 {}", success.result_message);
    Ok(success.result_message)
}

pub(super) fn finalize(
    context: &ProcessingContext,
    workflow: &mut ProcessingWorkflow,
    merged_output: PathBuf,
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<&crate::metadata::PassthroughMetadata>,
) -> Result<String> {
    let ui = context.new_emitter();
    let chapters = passthrough.map(|value| value.chapters.as_slice());
    let rewrote =
        crate::metadata::finish_artifact_tags(&merged_output, metadata.as_ref(), chapters, || {
            ui.emit_metadata_start("Writing metadata...")
        })?;
    if rewrote {
        ui.emit_finalizing("Finalizing...");
    }
    complete_staged_output(context, merged_output, &mut workflow.workspace)
}

#[cfg(test)]
#[path = "finalize_tests.rs"]
mod tests;
