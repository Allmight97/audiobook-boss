//! Finalization stage — metadata writing, output move, and cleanup.
//!
//! All filesystem side-effects (rename/move + cleanup) are centralized here.
//! Metadata is written during mux, so `write_metadata_stage` is a no-op.
//! Cancellation checks run after each sub-step to avoid unnecessary work.

// Imports
use std::path::{Path, PathBuf};

use crate::audio::cleanup::CleanupGuard;
use crate::audio::context::ProcessingContext;
use crate::audio::{ProcessingStage, ProgressReporter};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::AudiobookMetadata;

use super::ProcessingWorkflow;

/// Derives a preview output path `<final_basename>.preview.m4b` alongside the final output
fn derive_preview_output_path(final_output: &Path) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    let stem = final_output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    parent.join(format!("{}.preview.m4b", stem))
}

// Note: cover art detection helper removed; metadata writes handled during mux

// NOTE: File movement logic uses filesystem operations rather than FFmpeg output paths.
// This approach ensures compatibility across different output destinations and provides
// atomic completion semantics for the processing pipeline.
/// Moves temporary output to final location (filesystem boundary)
pub(crate) fn move_to_final_location(temp_output: PathBuf, final_path: &Path) -> Result<PathBuf> {
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::FileValidation(format!(
                "Cannot create output directory '{}': {e}",
                sanitize_path_for_display(parent)
            ))
        })?;
    }
    match std::fs::rename(&temp_output, final_path) {
        Ok(()) => {
            log::info!(
                "finalize_move method=rename status=ok dest={}",
                final_path.display()
            );
            Ok(final_path.to_path_buf())
        }
        Err(rename_err) => {
            log::warn!(
                "finalize_move method=rename status=err dest={} err={}",
                final_path.display(),
                rename_err
            );
            // FALLBACK[FB-012]: trigger=atomic rename fails (cross-volume/permissions edge)
            // observe=warn rename failure + info copy-replace success logs
            // sunset=2026-06-30 issue=#195
            // Fallback: copy then remove temp (handles cross-volume or other rename failures)
            if final_path.exists() {
                if let Err(e) = std::fs::remove_file(final_path) {
                    log::warn!("finalize_move overwrite remove failed: {}", e);
                }
            }
            std::fs::copy(&temp_output, final_path).map_err(|e| {
                AppError::FileValidation(format!(
                    "Cannot copy file to final location '{}': {e}",
                    sanitize_path_for_display(final_path)
                ))
            })?;
            if let Err(e) = std::fs::remove_file(&temp_output) {
                log::warn!("finalize_move temp removal failed: {}", e);
            }
            log::info!(
                "finalize_move method=copy-replace status=ok dest={}",
                final_path.display()
            );
            Ok(final_path.to_path_buf())
        }
    }
}

/// Cleans up session-specific temporary directory using CleanupGuard
pub(crate) fn cleanup_temp_directory_with_session(
    session_id: &str,
    temp_dir: PathBuf,
) -> Result<()> {
    log::debug!(
        "Cleaning up temporary directory for session {}: {}",
        session_id,
        temp_dir.display()
    );
    let mut guard = CleanupGuard::new(session_id.to_string());
    guard.add_path(&temp_dir);
    guard.cleanup_now().map_err(|e| {
        log::warn!(
            "Failed to cleanup temporary directory '{}': {}",
            temp_dir.display(),
            e
        );
        e
    })
}

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

    if !crate::metadata::mp4ameta_bridge::is_mp4_container(merged_output) {
        log::debug!("Finalize metadata stage skipped; ffmpeg-next handled metadata during mux");
        return Ok(());
    }

    let ui = _context.new_emitter();
    reporter.set_stage(ProcessingStage::WritingMetadata);
    ui.emit_metadata_start("Writing metadata...");
    crate::metadata::mp4ameta_bridge::write_metadata(merged_output, &metadata)?;
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
        context.output.final_path().display()
    );

    let ui = context.new_emitter();
    ui.emit_cleanup("Cleaning up...");

    log::info!("Moving temporary file to final location...");
    let final_output = move_to_final_location(merged_output, context.output.final_path())?;
    log::info!("✓ File moved successfully to: {}", final_output.display());

    if context.is_cancelled() {
        log::warn!("Processing was cancelled during completion");
        return Err(AppError::InvalidInput(
            "Processing was cancelled".to_string(),
        ));
    }

    log::info!("Cleaning up temporary directory...");
    cleanup_temp_directory_with_session(&context.session.id(), workflow.temp_dir)?;
    log::info!("✓ Temporary directory cleaned up successfully");

    reporter.complete();
    ui.emit_complete("Processing complete");

    let success_message = format!("Successfully created audiobook: {}", final_output.display());
    log::info!("🎉 {}", success_message);
    Ok(success_message)
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

    // If preview mode is enabled, move to preview-named path with overwrite policy
    if let Some(preview_cfg) = context.preview.as_ref() {
        let preview_path = derive_preview_output_path(context.output.final_path());
        log::info!(
            "Preview finalize: total_seconds={:.3} dest={}",
            preview_cfg.total_seconds,
            preview_path.display()
        );

        // Overwrite any existing preview file
        if preview_path.exists() {
            if let Err(e) = std::fs::remove_file(&preview_path) {
                log::warn!(
                    "Failed to remove existing preview file ({}): {}",
                    preview_path.display(),
                    e
                );
            }
        }
        let moved = move_to_final_location(merged_output.clone(), &preview_path)?;
        cleanup_temp_directory_with_session(&context.session.id(), workflow.temp_dir)?;
        reporter.complete();
        let ui = context.new_emitter();
        ui.emit_complete("Preview created successfully");
        let msg = format!("Successfully created preview: {}", moved.display());
        log::info!("🎉 {}", msg);
        return Ok(msg);
    }

    complete_processing(context, workflow, merged_output, reporter)
}
