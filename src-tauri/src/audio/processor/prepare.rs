//! Preparation stage — validation and workspace setup.
//!
//! Functions are private to the audio processor cluster.

use crate::audio::AudioFile;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::{PreviewConfig, ProcessingContext};

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

/// Prepares workspace (temp dir + concat file + total duration aggregation).
pub(crate) fn prepare_workspace(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    let temp_dir = crate::audio::processor::staging::create_processing_workspace_dir(
        context.session.uuid(),
        context.processing_workspace_root(),
    )?;

    let total_duration = progress_total_duration(files, context.preview.as_ref());

    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }

    Ok(ProcessingWorkflow::new(temp_dir, total_duration))
}

pub(crate) fn progress_total_duration(files: &[AudioFile], preview: Option<&PreviewConfig>) -> f64 {
    if let Some(preview) = preview {
        // Attainable preview output, not the requested budget: each file can
        // contribute at most its own duration toward its per-file excerpt.
        // Unknown durations fall back to the full per-file budget.
        let valid_count = files.iter().filter(|file| file.is_valid).count();
        let per_file_budget = preview.per_file_seconds(valid_count);
        return files
            .iter()
            .filter(|file| file.is_valid)
            .map(|file| match file.duration {
                Some(duration) => duration.min(per_file_budget),
                None => per_file_budget,
            })
            .sum();
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
    validate_processing_inputs(context, files)?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    prepare_workspace(context, files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn audio_file(name: &str, duration: Option<f64>, is_valid: bool) -> AudioFile {
        AudioFile {
            input_id: format!("{name}-input"),
            path: PathBuf::from(name),
            size: Some(1.0),
            duration,
            format: Some("MP3".to_string()),
            bitrate: Some(64),
            sample_rate: Some(44_100),
            channels: Some(2),
            codec_label: Some("AAC".to_string()),
            selected_decoder: Some("Native AAC (FFmpeg)".to_string()),
            tag_title: None,
            tag_artist: None,
            chapters: Vec::new(),
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
    fn progress_total_duration_caps_preview_at_attainable_output() {
        let files = vec![audio_file("short.mp3", Some(20.0), true)];
        let preview = PreviewConfig::new(60.0);

        assert_eq!(progress_total_duration(&files, Some(&preview)), 20.0);
    }

    #[test]
    fn progress_total_duration_mixed_preview_caps_per_file() {
        let files = vec![
            audio_file("short.mp3", Some(8.0), true),
            audio_file("long.mp3", Some(240.0), true),
        ];
        let preview = PreviewConfig::new(30.0);

        assert_eq!(progress_total_duration(&files, Some(&preview)), 23.0);
    }

    #[test]
    fn progress_total_duration_unknown_duration_uses_the_budget() {
        let files = vec![audio_file("unknown.mp3", None, true)];
        let preview = PreviewConfig::new(30.0);

        assert_eq!(progress_total_duration(&files, Some(&preview)), 30.0);
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
