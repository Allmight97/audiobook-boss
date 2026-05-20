//! Finalization stage — metadata writing and processor lifecycle completion.
//!
//! Output artifact commit policy lives in `output_artifact`; this module
//! sequences processor progress, cleanup registration, and success emission.
//! Metadata is written during mux, so `write_metadata_stage` is a no-op.
//! Cancellation checks run after each sub-step to avoid unnecessary work.

use std::path::{Path, PathBuf};

use crate::audio::CleanupGuard;
use crate::errors::{AppError, Result};
use crate::metadata::AudiobookMetadata;
use crate::output_artifact::{
    commit_output_artifact, finalized_output_success, OutputCommitRequest,
};
use crate::processing::{ProcessingContext, ProcessingStage, ProgressReporter};

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
    reporter: Option<&mut ProgressReporter>,
) -> Result<String> {
    log::info!("🚀 Starting staged output completion");
    log::info!("Temporary file: {}", staged_output.display());
    log::info!(
        "Final output path: {}",
        context.output.artifact_path().display()
    );

    ensure_not_cancelled_before_commit(context)?;

    let ui = context.new_emitter();
    ui.emit_cleanup("Cleaning up...");

    let commit_request = OutputCommitRequest::new(
        context.output.artifact_path(),
        context.output.commit_action(),
    );
    let outcome = commit_output_artifact(commit_request, staged_output, cleanup_guard, || {
        context.is_cancelled()
    })?;
    log::info!(
        "✓ File moved successfully to: {}",
        outcome.final_output.display()
    );
    if let Some(reporter) = reporter {
        reporter.complete();
    }
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
    reporter: &mut ProgressReporter,
) -> Result<String> {
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(workflow.temp_dir);
    cleanup_guard.add_path(&merged_output);

    complete_staged_output(context, merged_output, &mut cleanup_guard, Some(reporter))
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

#[cfg(test)]
mod tests {
    use super::complete_staged_output;
    use crate::audio::{
        BitrateMode, ChannelConfig, CleanupGuard, EncoderSettings, EncoderType, SampleRateConfig,
        ThreadSetting,
    };
    use crate::processing::{JobRegistry, OutputConfig, ProcessingContext, ProcessingSession};
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn encoder_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Auto,
            afterburner: false,
            threads: ThreadSetting::Auto,
            twoloop: true,
        }
    }

    #[tokio::test]
    async fn complete_staged_output_cleans_without_committing_when_cancelled_before_commit() {
        let registry = JobRegistry::new(1);
        let (job_id, _permit) = registry.register_job().await.expect("register job");
        let checker = registry.cancellation_checker(job_id).await;
        registry.cancel_job(job_id).await.expect("cancel job");

        let temp_dir = TempDir::new().expect("temp dir");
        let staged_output = temp_dir.path().join("staged.m4b");
        let final_output = temp_dir.path().join("final.m4b");
        fs::write(&staged_output, b"audio").expect("write staged output");

        let context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::from_job_registry(job_id.0, checker)),
            encoder_settings(),
            SampleRateConfig::Auto,
            OutputConfig::new(&final_output),
        );
        let mut cleanup_guard = CleanupGuard::new(context.session.id());
        cleanup_guard.add_path(&staged_output);

        let error =
            complete_staged_output(&context, staged_output.clone(), &mut cleanup_guard, None)
                .expect_err("cancelled job should not commit staged output");

        assert!(
            error.to_string().contains("Processing was cancelled"),
            "unexpected error: {error}"
        );
        assert!(
            !final_output.exists(),
            "cancelled staged output should not be committed"
        );
        drop(cleanup_guard);
        assert!(
            !staged_output.exists(),
            "cancelled staged output should be cleaned"
        );
    }
}
