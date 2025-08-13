//! finalize.rs
//!
//! Phase 0 scaffold module for the processor split (see
//! docs/planning/processor_split_plan.md).
//!
//! Purpose:
//!   Will host the metadata writing, final movement to output path,
//!   cleanup (temporary directory removal), and final reporting logic
//!   migrated from the former monolithic `audio/processor.rs` in Phase 3.
//!
//! Target contents (to be implemented in Phase 3):
//!   - write_metadata_stage
//!   - complete_processing
//!   - finalize_processing (orchestration combining the above)
//!   - move_to_final_location
//!   - cleanup_temp_directory_with_session
//!
//! Additionally, Phase 4 will relocate deprecated adapter helpers that
//! delegate to these functions into `legacy.rs` (e.g. old cleanup wrapper).
//!
//! Design Notes:
//!   - All filesystem side-effects (rename/move + cleanup) centralized here
//!     to confine IO + failure handling semantics to one layer.
//!   - Metadata writing uses `crate::metadata::write_metadata`; this stage
//!     emits UI progress events via `ProgressEmitter` (kept intact when moved).
//!   - Cancellation checks should remain after each major sub‑step to avoid
//!     doing unnecessary work if user aborts late.
//!   - This module should not expose public API directly; `mod.rs` will
//!     re‑export necessary orchestration functions to preserve stable surface.
//!   - Keep each function under 60 LOC; extract small helpers only when they
//!     demonstrably reduce cognitive complexity (avoid premature granularity).
//!
//! Migration Strategy (Phase 3):
//!   1. Move pure helpers (`move_to_final_location`, `cleanup_temp_directory_with_session`).
//!   2. Move metadata + completion stages.
//!   3. Move the orchestration (`finalize_processing`) last to minimize interim
//!      broken references.
//!   4. Adjust imports (e.g. `use crate::errors::AppError`) locally; prefer
//!      fully qualified paths sparingly to keep readability.
//!   5. Run incremental compilation (Phase 6) after each move if desired.
//!
//! Roadmap TODOs:
//!   - Deprecated adapters referencing these functions will receive a
//!     file‑level TODO in `legacy.rs` for gating/removal (Roadmap P2.1.1).
//!
//! Phase: 3 (Finalize Stage Extraction) - metadata writing, cleanup, and file movement
//!
//! NOTE: Orchestrator moved to mod.rs in Phase 5.

#![allow(dead_code)]

// Imports
use std::path::{Path, PathBuf};

use crate::audio::cleanup::CleanupGuard;
use crate::audio::context::ProcessingContext;
use crate::audio::{ProcessingStage, ProgressReporter};
use crate::errors::{AppError, Result};
use crate::metadata::{write_metadata, AudiobookMetadata};

use super::ProcessingWorkflow;

// TODO (Roadmap P2.1.1): This movement logic will be reviewed during legacy removal.
// The new engine may handle output paths directly, potentially deprecating this.
/// Moves temporary output to final location (filesystem boundary)
pub(crate) fn move_to_final_location(temp_output: PathBuf, final_path: &Path) -> Result<PathBuf> {
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::FileValidation(format!(
                "Cannot create output directory '{}': {e}",
                parent.display()
            ))
        })?;
    }
    std::fs::rename(&temp_output, final_path).map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot move file to final location '{}': {e}",
            final_path.display()
        ))
    })?;
    Ok(final_path.to_path_buf())
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
    context: &ProcessingContext,
    merged_output: &PathBuf,
    metadata: Option<AudiobookMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<()> {
    if let Some(metadata) = metadata {
        let ui = crate::audio::progress::ProgressEmitter::new(context.window.clone());
        ui.emit_metadata_start("Writing metadata...");
        reporter.set_stage(ProcessingStage::WritingMetadata);
        write_metadata(merged_output, &metadata)?;
        // If cover art bytes are present, write them after basic tags
        if let Some(cover) = metadata.cover_art.as_ref() {
            crate::metadata::writer::write_cover_art(merged_output, cover)?;
        }
        if context.is_cancelled() {
            return Err(AppError::InvalidInput(
                "Processing was cancelled".to_string(),
            ));
        }
    }
    Ok(())
}

/// Completes processing: move to final path + cleanup + final UI emit
pub(crate) fn complete_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    let ui = crate::audio::progress::ProgressEmitter::new(context.window.clone());
    ui.emit_cleanup("Cleaning up...");
    let final_output = move_to_final_location(merged_output, &context.settings.output_path)?;
    if context.is_cancelled() {
        return Err(AppError::InvalidInput(
            "Processing was cancelled".to_string(),
        ));
    }
    cleanup_temp_directory_with_session(&context.session.id(), workflow.temp_dir)?;
    reporter.complete();
    ui.emit_complete("Processing complete");
    Ok(format!(
        "Successfully created audiobook: {}",
        final_output.display()
    ))
}

/// Finalize pipeline: metadata + completion
pub(crate) async fn finalize_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    metadata: Option<AudiobookMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    write_metadata_stage(context, &merged_output, metadata, reporter)?;
    complete_processing(context, workflow, merged_output, reporter)
}
