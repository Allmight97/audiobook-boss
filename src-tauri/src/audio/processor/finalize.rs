//! Finalization stage — metadata writing and processor lifecycle completion.
//!
//! Output artifact commit policy lives in `audio::output_path`; this module
//! sequences processor progress, cleanup registration, and success emission.
//! Metadata is written during mux, so `write_metadata_stage` is a no-op.
//! Cancellation checks run after each sub-step to avoid unnecessary work.

use std::path::{Path, PathBuf};

use crate::audio::cleanup::CleanupGuard;
use crate::audio::context::ProcessingContext;
use crate::audio::output_path::{commit_output_artifact, finalized_output_success};
use crate::audio::{ProcessingStage, ProgressReporter};
use crate::errors::Result;
use crate::metadata::AudiobookMetadata;

use super::ProcessingWorkflow;

// Note: cover art detection helper removed; metadata writes handled during mux

/// Writes metadata if provided (UI emission included)
pub(crate) fn write_metadata_stage(
    _context: &ProcessingContext,
    merged_output: &Path,
    metadata: Option<AudiobookMetadata>,
    _passthrough: Option<&crate::metadata::passthrough::PassthroughMetadata>,
    reporter: &mut ProgressReporter,
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
    reporter.set_stage(ProcessingStage::WritingMetadata);
    ui.emit_metadata_start("Writing metadata...");
    crate::metadata::write_finalized_metadata(merged_output, &metadata)?;
    ui.emit_finalizing("Finalizing...");
    Ok(())
}

/// Completes processing: move to final path + cleanup + final UI emit
pub(crate) fn complete_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    log::info!("🚀 Starting complete_processing stage");
    log::info!("Temporary file: {}", merged_output.display());
    log::info!(
        "Final output path: {}",
        context.output.artifact_path().display()
    );

    let ui = context.new_emitter();
    ui.emit_cleanup("Cleaning up...");

    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(workflow.temp_dir);
    cleanup_guard.add_path(&merged_output);
    let outcome = commit_output_artifact(
        context,
        merged_output,
        context.output.artifact_path(),
        &mut cleanup_guard,
    )?;
    log::info!(
        "✓ File moved successfully to: {}",
        outcome.final_output.display()
    );
    reporter.complete();
    let success = finalized_output_success(
        context.output.output_kind(),
        &outcome.final_output,
        outcome.cancelled,
    );
    ui.emit_complete(success.ui_message);
    log::info!("🎉 {}", success.result_message);
    Ok(success.result_message)
}

/// Finalize pipeline: metadata + completion
pub(crate) async fn finalize_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<crate::metadata::passthrough::PassthroughMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    let passthrough_ref = passthrough.as_ref();
    write_metadata_stage(context, &merged_output, metadata, passthrough_ref, reporter)?;

    complete_processing(context, workflow, merged_output, reporter)
}
