//! Preparation stage — validation, workspace setup, and sample rate detection.
//!
//! Functions are `pub(crate)` and re-exported selectively by `processor::mod.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::audio::context::{PreviewConfig, ProcessingContext};
use crate::audio::{AudioFile, ProcessingStage, ProgressReporter};
use crate::errors::{sanitize_path_for_display, AppError, Result};

use super::ProcessingWorkflow;

/// Detects the most common sample rate across provided files.
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
    let inspection = crate::audio::processor::streams::inspect_audio_decoder(path)?;
    log::info!(
        "sample_rate_probe path={} selected_decoder={} sample_rate={}",
        sanitize_path_for_display(path),
        inspection.selected_decoder,
        inspection.sample_rate
    );
    Ok(inspection.sample_rate)
}

/// Validates processing inputs (files + settings).
pub(crate) fn validate_processing_inputs(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<()> {
    if files.is_empty() {
        return Err(AppError::InvalidInput("No files to process".to_string()));
    }

    for file in files {
        if !file.is_valid {
            return Err(AppError::FileValidation(format!(
                "Invalid file: {} - {}",
                sanitize_path_for_display(&file.path),
                file.error.as_deref().unwrap_or("Unknown error")
            )));
        }
        // Safety: ensure path passes our centralized validation again.
        let _ = crate::audio::path_validation::validate_input_audio_path(&file.path)?;
    }

    crate::audio::settings::validate_sample_rate_config(&context.sample_rate)?;
    crate::audio::settings::validate_output_path(context.output.final_path())?;
    Ok(())
}

/// Creates a session-specific temporary directory.
pub(crate) fn create_temp_directory_with_session(session_id: &str) -> Result<PathBuf> {
    const TEMP_DIR_NAME: &str = crate::audio::constants::TEMP_DIR_NAME;
    let temp_dir = std::env::temp_dir().join(TEMP_DIR_NAME).join(session_id);
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        AppError::FileValidation(format!(
            "Cannot create session temp directory '{}': {e}",
            sanitize_path_for_display(&temp_dir)
        ))
    })?;
    Ok(temp_dir)
}

/// Emits initial progress + validates inputs with cancellation awareness.
pub(crate) fn validate_inputs_with_progress(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<()> {
    let mut emitter = ProgressReporter::new(1);
    emitter.set_stage(ProcessingStage::Analyzing);

    validate_processing_inputs(context, files)?;

    if context.is_cancelled() {
        return Err(AppError::cancelled());
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

    // Single engine: no concat file needed

    let total_duration = progress_total_duration(files, context.preview.as_ref());

    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }

    Ok(ProcessingWorkflow::new(temp_dir, total_duration))
}

pub(crate) fn progress_total_duration(files: &[AudioFile], preview: Option<&PreviewConfig>) -> f64 {
    if let Some(preview) = preview {
        let valid_count = files.iter().filter(|file| file.is_valid).count();
        return preview.per_file_seconds(valid_count) * valid_count as f64;
    }

    files
        .iter()
        .filter(|file| file.is_valid)
        .map(|file| file.duration.unwrap_or(0.0))
        .sum()
}

/// Orchestrates validation + workspace preparation.
pub(crate) fn validate_and_prepare(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    validate_inputs_with_progress(context, files)?;
    prepare_workspace(context, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio_file(name: &str, duration: Option<f64>, is_valid: bool) -> AudioFile {
        AudioFile {
            path: PathBuf::from(name),
            size: Some(1.0),
            duration,
            format: Some("MP3".to_string()),
            bitrate: Some(64),
            sample_rate: Some(44_100),
            channels: Some(2),
            codec_label: Some("AAC".to_string()),
            selected_decoder: Some("Native AAC (FFmpeg)".to_string()),
            is_valid,
            error: None,
        }
    }

    #[test]
    fn progress_total_duration_uses_preview_scaled_work() {
        let files = vec![
            audio_file("first.mp3", Some(120.0), true),
            audio_file("second.mp3", Some(240.0), true),
            audio_file("invalid.mp3", Some(500.0), false),
        ];
        let preview = PreviewConfig::new(30.0);

        let total = progress_total_duration(&files, Some(&preview));

        assert_eq!(total, 30.0);
    }

    #[test]
    fn progress_total_duration_uses_full_duration_without_preview() {
        let files = vec![
            audio_file("first.mp3", Some(120.0), true),
            audio_file("second.mp3", Some(240.0), true),
            audio_file("invalid.mp3", Some(500.0), false),
        ];

        let total = progress_total_duration(&files, None);

        assert_eq!(total, 360.0);
    }
}
