//! Preparation stage — validation and workspace setup.
//!
//! Functions are private to the audio processor cluster.

use std::path::{Path, PathBuf};

use crate::audio::AudioFile;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::{PreviewConfig, ProcessingContext, ProcessingStage, ProgressReporter};
use uuid::Uuid;

use super::ProcessingWorkflow;

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
pub(crate) fn create_temp_directory_with_session(
    session_id: Uuid,
    final_artifact_path: &Path,
) -> Result<PathBuf> {
    crate::audio::processor::staging::create_destination_staging_dir(
        session_id,
        final_artifact_path,
    )
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

    let temp_dir =
        create_temp_directory_with_session(context.session.uuid(), context.output.artifact_path())?;

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
