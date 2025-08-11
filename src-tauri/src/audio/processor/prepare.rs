//! prepare.rs
//!
//! Phase 1 implementation module for the processor split (see
//! docs/planning/processor_split_plan.md).
//!
//! Responsibilities (extracted from former monolithic `audio/processor.rs`):
//!   - Input validation
//!   - Workspace (temp directory + concat file) creation
//!   - Sample rate detection
//!   - Early progress stage emission
//!
//! Roadmap Notes:
//!   - `detect_input_sample_rate` may move to a future analysis module
//!     (see plan: Deferred Item P9-D2).
//!   - Legacy adapters will delegate into the staged API after full split.
//!
//! Public Surface (indirect):
//!   These functions are internal (`pub(crate)`) and re-exported selectively
//!   by `processor::mod.rs` to preserve stable external API.
//!
//! Function Size Compliance:
//!   All functions kept <60 LOC and cohesive. Helpers remain private unless
//!   required across stage boundaries.
//!
//! Phase: 1 (Prepare Stage Extraction)

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::audio::context::ProcessingContext;
use crate::audio::{AudioFile, AudioSettings, ProcessingStage, ProgressReporter};
use crate::errors::{AppError, Result};
use crate::ffmpeg::format_concat_file_line;
use lofty::file::AudioFile;

use super::ProcessingWorkflow;

/// Detects the most common sample rate across provided files.
/// Potential future extraction to dedicated analysis module (see roadmap P9-D2).
pub fn detect_input_sample_rate(file_paths: &[PathBuf]) -> Result<u32> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "Cannot detect sample rate: no input files provided".to_string(),
        ));
    }

    let mut sample_rates: HashMap<u32, u32> = HashMap::new();
    let mut first_rate = None;

    for path in file_paths {
        match get_file_sample_rate(path) {
            Ok(rate) => {
                if first_rate.is_none() {
                    first_rate = Some(rate);
                }
                *sample_rates.entry(rate).or_insert(0) += 1;
            }
            Err(e) => {
                log::warn!("Could not read sample rate from {}: {}", path.display(), e);
            }
        }
    }

    if sample_rates.is_empty() {
        return Err(AppError::InvalidInput(
            "Cannot detect sample rate: no valid audio files found".to_string(),
        ));
    }

    let most_common = sample_rates
        .iter()
        .max_by_key(|(_, &count)| count)
        .map(|(&rate, _)| rate);

    match most_common {
        Some(rate) => Ok(rate),
        None => first_rate.ok_or_else(|| {
            AppError::InvalidInput("Cannot determine sample rate from input files".to_string())
        }),
    }
}

/// Private helper to retrieve sample rate from a single file.
fn get_file_sample_rate(path: &Path) -> Result<u32> {
    use lofty::probe::Probe;
    let tagged_file = Probe::open(path)
        .map_err(AppError::Metadata)?
        .read()
        .map_err(AppError::Metadata)?;

    tagged_file.properties().sample_rate().ok_or_else(|| {
        AppError::InvalidInput(format!(
            "File {} has no sample rate information",
            path.display()
        ))
    })
}

/// Validates processing inputs (files + settings).
pub(crate) fn validate_processing_inputs(
    files: &[AudioFile],
    settings: &AudioSettings,
) -> Result<()> {
    if files.is_empty() {
        return Err(AppError::InvalidInput("No files to process".to_string()));
    }

    for file in files {
        if !file.is_valid {
            return Err(AppError::FileValidation(format!(
                "Invalid file: {} - {}",
                file.path.display(),
                file.error.as_deref().unwrap_or("Unknown error")
            )));
        }
        // Safety: ensure path passes our centralized validation again.
        let _ = crate::audio::path_validation::validate_input_audio_path(&file.path)?;
    }

    crate::audio::settings::validate_audio_settings(settings)?;
    Ok(())
}

/// Creates a session-specific temporary directory.
pub(crate) fn create_temp_directory_with_session(session_id: &str) -> Result<PathBuf> {
    const TEMP_DIR_NAME: &str = crate::audio::constants::TEMP_DIR_NAME;
    let temp_dir = std::env::temp_dir().join(TEMP_DIR_NAME).join(session_id);
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot create session temp directory '{}': {e}",
            temp_dir.display()
        ))
    })?;
    Ok(temp_dir)
}

/// Creates FFmpeg concat list file with properly escaped lines.
pub(crate) fn create_concat_file(files: &[AudioFile], temp_dir: &Path) -> Result<PathBuf> {
    const TEMP_CONCAT_FILENAME: &str = crate::audio::constants::TEMP_CONCAT_FILENAME;
    let concat_file = temp_dir.join(TEMP_CONCAT_FILENAME);

    let mut content = String::new();
    for file in files {
        content.push_str(&format_concat_file_line(&file.path));
    }

    std::fs::write(&concat_file, content).map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot write concat file '{}': {e}",
            concat_file.display()
        ))
    })?;

    Ok(concat_file)
}

/// Emits initial progress + validates inputs with cancellation awareness.
pub(crate) fn validate_inputs_with_progress(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<()> {
    let mut emitter = ProgressReporter::new(1);
    emitter.set_stage(ProcessingStage::Analyzing);

    validate_processing_inputs(files, &context.settings)?;

    if context.is_cancelled() {
        return Err(AppError::InvalidInput(
            "Processing was cancelled".to_string(),
        ));
    }
    Ok(())
}

/// Prepares workspace (temp dir + concat file + total duration aggregation).
pub(crate) fn prepare_workspace(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    let mut emitter = ProgressReporter::new(1);
    emitter.set_stage(ProcessingStage::Analyzing);

    let temp_dir = create_temp_directory_with_session(&context.session.id())?;
    let concat_file = create_concat_file(files, &temp_dir)?;

    let total_duration: f64 = files
        .iter()
        .filter(|f| f.is_valid)
        .map(|f| f.duration.unwrap_or(0.0))
        .sum();

    if context.is_cancelled() {
        return Err(AppError::InvalidInput(
            "Processing was cancelled".to_string(),
        ));
    }

    Ok(ProcessingWorkflow::new(
        temp_dir,
        concat_file,
        total_duration,
    ))
}

/// Orchestrates validation + workspace preparation.
pub(crate) fn validate_and_prepare(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    validate_inputs_with_progress(context, files)?;
    prepare_workspace(context, files)
}
