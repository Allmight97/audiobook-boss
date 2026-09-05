//! Finalization stage — metadata writing and processor lifecycle completion.
//!
//! Output artifact commit policy lives in `output_artifact`; this module
//! sequences processor progress, cleanup registration, and success emission.
//! Native muxing may write initial metadata; finalized MP4-family metadata can
//! still be written here before artifact commit.
//! Cancellation checks run after each sub-step to avoid unnecessary work.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::audio::CleanupGuard;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::AudiobookMetadata;
use crate::output_artifact::{
    commit_output_artifact, finalized_output_success, OutputCommitRequest,
};
use crate::processing::ProcessingContext;

use super::ProcessingWorkflow;

// Note: cover art detection helper removed; metadata writes handled during mux

/// Writes metadata if provided (UI emission included)
pub(crate) fn write_metadata_stage(
    _context: &ProcessingContext,
    merged_output: &Path,
    metadata: Option<AudiobookMetadata>,
) -> Result<()> {
    let Some(metadata) = metadata else {
        log::debug!("Finalize metadata stage skipped; no metadata provided");
        return Ok(());
    };

    if !crate::metadata::should_write_finalized_metadata(merged_output)? {
        log::debug!("Finalize metadata stage skipped; ffmpeg-next handled metadata during mux");
        return Ok(());
    }

    let ui = _context.new_emitter();
    ui.emit_metadata_start("Writing metadata...");
    let started = Instant::now();
    crate::metadata::write_finalized_metadata(merged_output, &metadata)?;
    log::info!(
        "finalized_metadata_write status=ok elapsed_ms={} path={}",
        started.elapsed().as_millis(),
        sanitize_path_for_display(merged_output)
    );
    ui.emit_finalizing("Finalizing...");
    Ok(())
}

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
    log::info!("🚀 Starting staged output completion");
    log::info!(
        "Temporary file: {}",
        sanitize_path_for_display(&staged_output)
    );
    log::info!(
        "Final output path: {}",
        sanitize_path_for_display(context.output.final_path())
    );

    ensure_not_cancelled_before_commit(context)?;

    let ui = context.new_emitter();
    ui.emit_cleanup("Cleaning up...");

    let commit_request =
        OutputCommitRequest::new(context.output.final_path(), context.output.commit_action());
    let outcome = commit_output_artifact(commit_request, staged_output, cleanup_guard, || {
        context.is_cancelled()
    })?;
    log::info!(
        "✓ File moved successfully to: {}",
        sanitize_path_for_display(&outcome.final_output)
    );
    let success = finalized_output_success(
        context.output.output_kind(),
        &outcome.final_output,
        outcome.cancelled,
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
    write_metadata_stage(context, &merged_output, metadata)?;

    if context.preview.is_none() {
        if let Some(passthrough) = passthrough {
            crate::metadata::verify_chapters(&merged_output, &passthrough.chapters)?;
        }
    }
    complete_processing(context, workflow, merged_output)
}

#[cfg(test)]
#[path = "finalize_tests.rs"]
mod tests;
