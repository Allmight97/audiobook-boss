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
//! Phase: 0 (Baseline & Scaffolding) – intentionally empty file except for
//! documentation and temporary lint allowances.
//!
//! NOTE: Leaving this stub present while the original monolithic
//! `audio/processor.rs` still exists. Only after migration + removal of the
//! monolith will the compiled module set become active.
//!
//! After migration this file will own side-effect boundaries; tests targeting
//! end-to-end behavior should continue to exercise through the high-level
//! `process_audiobook_with_context` orchestrator in `mod.rs`.
//!
//! Size expectations (post-migration):
//!   - finalize.rs < 400 LOC
//!   - Each function < 60 LOC
//!
//! Keeping `#![allow(dead_code)]` temporarily. This will be removed or narrowed
//! once Phase 3 completes and functions are referenced.
//
#![allow(dead_code)]
// Phase 1 NOTE:
// Temporarily housing finalize + orchestrator logic here earlier than original
// phase plan for continuity. Execution functions still reside in the legacy
// monolithic file until Phase 2 extraction; references to `execute::execute_processing`
// will resolve only after that migration (or after removing the monolith).
//
// Imports
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::audio::cleanup::CleanupGuard;
use crate::audio::context::ProcessingContext;
use crate::audio::metrics::ProcessingMetrics;
use crate::audio::{AudioFile, ProcessingStage, ProgressReporter};
use crate::errors::{AppError, Result};
use crate::metadata::{write_metadata, AudiobookMetadata};

use super::ProcessingWorkflow;
// Prepare + Execute stage modules (execute still pending migration)
use crate::audio::processor::execute;
use crate::audio::processor::prepare;

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

/// Orchestrator (temporary placement here during early split)
/// Public API (will be re-exported) - context-based processing entrypoint.
pub async fn process_audiobook_with_context(
    context: ProcessingContext,
    files: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    let mut reporter = ProgressReporter::new(files.len());
    let mut metrics = ProcessingMetrics::new();

    // Stage 1: Validate + Prepare (from prepare module)
    reporter.set_stage(ProcessingStage::Analyzing);
    let workflow = prepare::validate_and_prepare(&context, &files)?;

    // Metrics accumulation (estimates)
    for file in &files {
        if file.is_valid {
            if let Some(duration) = file.duration {
                let estimated_bytes = (duration * context.settings.bitrate as f64 * 125.0) as usize;
                metrics.update_file_processed(Duration::from_secs_f64(duration), estimated_bytes);
            }
        }
    }

    // Stage 2: Execute (execute module; still pending migration if monolith present)
    let merged_output =
        execute::execute_processing(&context, &workflow, &files, &mut reporter).await?;

    // Stage 3: Finalize
    let result =
        finalize_processing(&context, workflow, merged_output, metadata, &mut reporter).await?;

    log::info!("{}", metrics.format_summary());
    Ok(result)
}

// TODO (Phase 5): Move orchestrator export + re-export logic into mod.rs to align
// with final architecture layout once execute + legacy migrations complete.
