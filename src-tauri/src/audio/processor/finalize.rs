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
//! Legacy adapters referencing finalize helpers were removed during nuclear
//! cleanup; no further gating required.
//!
//! Phase: 3 (Finalize Stage Extraction) - metadata writing, cleanup, and file movement
//!
//! NOTE: Orchestrator moved to mod.rs in Phase 5.



// Imports
use std::path::{Path, PathBuf};

use crate::audio::cleanup::CleanupGuard;
use crate::audio::context::ProcessingContext;
use crate::audio::{ProcessingStage, ProgressReporter};
use crate::errors::{AppError, Result};
use crate::metadata::{write_metadata, AudiobookMetadata};

use super::ProcessingWorkflow;

/// Checks if native cover art embedding was successful by examining the output file
/// 
/// This function uses Lofty to probe the file and check for existing cover art,
/// which helps determine if the native FFmpeg embedding worked correctly.
fn check_native_cover_art_success(file_path: &Path) -> Result<bool> {
    use lofty::prelude::*;
    use lofty::probe::Probe;
    
    match Probe::open(file_path) {
        Ok(probe) => {
            match probe.read() {
                Ok(tagged_file) => {
                    // Check if any tag contains pictures/cover art
                    let has_cover = tagged_file.tags().iter().any(|tag| !tag.pictures().is_empty());
                    log::debug!("Native cover art check: {} (file: {})", 
                              if has_cover { "found" } else { "not found" }, 
                              file_path.display());
                    Ok(has_cover)
                }
                Err(e) => {
                    log::debug!("Could not read file for cover art check: {}", e);
                    Err(AppError::Metadata(e))
                }
            }
        }
        Err(e) => {
            log::debug!("Could not probe file for cover art check: {}", e);
            Err(AppError::Metadata(e))
        }
    }
}

// NOTE: File movement logic uses filesystem operations rather than FFmpeg output paths.
// This approach ensures compatibility across different output destinations and provides
// atomic completion semantics for the processing pipeline.
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
        
        log::info!("Starting finalize stage metadata writing for: {}", merged_output.display());
        
        // Write basic metadata tags
        write_metadata(merged_output, &metadata)?;
        log::info!("✓ Basic metadata tags written successfully");
        
        // Handle cover art with fallback detection
        if let Some(cover) = metadata.cover_art.as_ref() {
            log::info!("Attempting Lofty cover art embedding as fallback - {} bytes", cover.len());
            
            // Check if native embedding succeeded by examining the file
            match check_native_cover_art_success(merged_output) {
                Ok(true) => {
                    log::info!("✓ Native cover art embedding detected - skipping Lofty fallback");
                }
                Ok(false) => {
                    log::info!("Native cover art not detected - proceeding with Lofty fallback");
                    match crate::metadata::writer::write_cover_art(merged_output, cover) {
                        Ok(()) => log::info!("✓ Lofty cover art fallback completed successfully"),
                        Err(e) => {
                            log::error!("✗ Lofty cover art fallback failed: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Could not verify native cover art status ({}), proceeding with Lofty fallback", e);
                    match crate::metadata::writer::write_cover_art(merged_output, cover) {
                        Ok(()) => log::info!("✓ Lofty cover art fallback completed successfully"),
                        Err(e) => {
                            log::error!("✗ Lofty cover art fallback failed: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
        } else {
            log::debug!("No cover art data to write in finalize stage");
        }
        
        if context.is_cancelled() {
            return Err(AppError::InvalidInput(
                "Processing was cancelled".to_string(),
            ));
        }
        
        log::info!("✓ Finalize stage metadata writing completed successfully");
    } else {
        log::debug!("No metadata provided for finalize stage");
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
    log::info!("🚀 Starting complete_processing stage");
    log::info!("Temporary file: {}", merged_output.display());
    log::info!("Final output path: {}", context.settings.output_path.display());
    
    let ui = crate::audio::progress::ProgressEmitter::new(context.window.clone());
    ui.emit_cleanup("Cleaning up...");
    
    log::info!("Moving temporary file to final location...");
    let final_output = move_to_final_location(merged_output, &context.settings.output_path)?;
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
    reporter: &mut ProgressReporter,
) -> Result<String> {
    write_metadata_stage(context, &merged_output, metadata, reporter)?;
    complete_processing(context, workflow, merged_output, reporter)
}
