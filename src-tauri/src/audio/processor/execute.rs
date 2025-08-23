//! execute.rs
//!
//! Phase 2 implementation module for the processor split (see
//! docs/planning/processor_split_plan.md).
//!
//! Responsibilities:
//!   - Execute merge / conversion stage using selected MediaProcessor implementation
//!   - Manage stage transition to Converting
//!   - Perform cancellation checks post-execution
//!   - Provide context-based (window/session) execution path
//!
//! Engine Selection (Post-Nuclear Phase 11):
//!   All feature flags and shell fallbacks were removed. The only implementation is
//!   `FfmpegNextProcessor` accessed through `selection::create_default_processor()`.
//!   Remaining abstraction is intentionally minimal for future enhancements.
//!
//! Function Size Compliance:
//!   All functions <60 LOC. A logging helper can be introduced later if
//!   additional logging expands bodies near the threshold.
//!
//! Public Surface:
//!   Functions are `pub(crate)` and invoked by orchestrator in `finalize.rs`
//!   (temporary placement) and later re-exported through `processor::mod.rs`.
//!
//! Phase: 2 (Execute Stage Extraction)



use std::path::{Path, PathBuf};

use crate::audio::constants::TEMP_MERGED_FILENAME;
use crate::audio::context::ProcessingContext;
use crate::audio::media_pipeline::{MediaProcessingPlan, MediaProcessor};
use crate::audio::{AudioFile, ProcessingStage, ProgressReporter};
use crate::errors::{AppError, Result};

use super::selection::{create_default_processor, get_engine_description};
use super::ProcessingWorkflow;

/// Executes core audio processing operations (merge / convert).
pub(crate) async fn execute_processing(
    context: &ProcessingContext,
    workflow: &ProcessingWorkflow,
    files: &[AudioFile],
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<PathBuf> {
    let mut emitter = ProgressReporter::new(1); // Single logical processing unit
    reporter.set_stage(ProcessingStage::Converting);
    emitter.set_stage(ProcessingStage::Converting);

    log::info!(
        "Starting FFmpeg merge - Total duration: {:.2}s, Bitrate: {}k",
        workflow.total_duration(),
        context.settings.bitrate
    );

    let merged_output = merge_audio_files_with_context(
        &workflow.temp_dir,
        context,
        reporter,
        workflow.total_duration(),
        files,
        metadata,
    )
    .await?;

    if context.is_cancelled() {
        return Err(AppError::InvalidInput(
            "Processing was cancelled".to_string(),
        ));
    }

    Ok(merged_output)
}

/// Merges audio files with context-based progress tracking.
pub(crate) async fn merge_audio_files_with_context(
    temp_dir: &Path,
    context: &ProcessingContext,
    _reporter: &mut ProgressReporter,
    total_duration: f64,
    files: &[AudioFile],
    metadata: Option<&crate::metadata::AudiobookMetadata>,
) -> Result<PathBuf> {
    let temp_output = temp_dir.join(TEMP_MERGED_FILENAME);

    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let settings = &context.settings;

    let mut plan = MediaProcessingPlan::new(
        temp_output.clone(),
        settings.clone(),
        file_paths,
        total_duration,
    );
    // Carry v2 encoder settings from context if present
    plan.encoder_settings_v2 = context.encoder_settings_v2.clone();

    // Use centralized engine selection via create_default_processor function
    // This abstracts away the feature flag logic and prepares for engine flip
    log::debug!("Using media processor: {}", get_engine_description());
    let processor = create_default_processor();
    processor.execute(&plan, context, metadata).await?;

    Ok(temp_output)
}
