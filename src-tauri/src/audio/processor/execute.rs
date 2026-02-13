//! execute.rs
//!
//! Execution stage for audio processing.
//!
//! This module owns the conversion/merge boundary in the processing pipeline:
//! it transitions progress into `Converting`, builds a `MediaProcessingPlan`,
//! dispatches to the selected processor implementation, and performs
//! cancellation checks before returning the merged output path.

use std::path::{Path, PathBuf};

use crate::audio::constants::TEMP_MERGED_FILENAME;
use crate::audio::context::ProcessingContext;
use crate::audio::processor::{MediaProcessingPlan, MediaProcessor};
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
    passthrough: Option<&crate::metadata::passthrough::PassthroughMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<PathBuf> {
    let mut emitter = ProgressReporter::new(1); // Single logical processing unit
    reporter.set_stage(ProcessingStage::Converting);
    emitter.set_stage(ProcessingStage::Converting);

    log::info!(
        "Starting FFmpeg merge - Total duration: {:.2}s, Bitrate: {}k",
        workflow.total_duration(),
        context.effective_bitrate_kbps()
    );

    let merged_output = merge_audio_files_with_context(
        &workflow.temp_dir,
        context,
        reporter,
        workflow.total_duration(),
        files,
        metadata,
        passthrough,
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
    passthrough: Option<&crate::metadata::passthrough::PassthroughMetadata>,
) -> Result<PathBuf> {
    let temp_output = temp_dir.join(TEMP_MERGED_FILENAME);

    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let plan = MediaProcessingPlan::new(
        temp_output.clone(),
        context.encoder_settings.clone(),
        context.sample_rate.clone(),
        file_paths,
        total_duration,
    );

    // Centralized processor selection keeps execution flow decoupled from engine wiring.
    log::debug!("Using media processor: {}", get_engine_description());
    let processor = create_default_processor();
    processor
        .execute(&plan, context, metadata, passthrough)
        .await?;

    Ok(temp_output)
}
