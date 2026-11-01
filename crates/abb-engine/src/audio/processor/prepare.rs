use crate::audio::AudioFile;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::{PreviewConfig, ProcessingContext};

use super::ProcessingWorkflow;

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
        let source = crate::audio::path_validation::validate_input_audio_path(&file.path)?;
        let expected = file.chapter_plan.as_ref().ok_or_else(|| {
            AppError::InvalidInput(
                "Audio source identity is missing. Remove and import it again.".into(),
            )
        })?;
        crate::metadata::validate_source_fingerprint(
            &std::fs::metadata(source)?,
            &expected.source_fingerprint,
        )?;
    }

    crate::audio::settings::validate_sample_rate_config(&context.sample_rate)?;
    Ok(())
}

pub(crate) fn prepare_workspace(
    context: &ProcessingContext,
    files: &[AudioFile],
    cleanup: &mut crate::audio::CleanupGuard,
) -> Result<ProcessingWorkflow> {
    let temp_dir = crate::audio::processor::staging::create_processing_workspace_dir(
        context.session.uuid(),
        context.processing_workspace_root(),
    )?;

    cleanup.add_path(&temp_dir);
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

pub(crate) fn validate_and_prepare(
    context: &ProcessingContext,
    files: &[AudioFile],
    cleanup: &mut crate::audio::CleanupGuard,
) -> Result<ProcessingWorkflow> {
    validate_processing_inputs(context, files)?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    prepare_workspace(context, files, cleanup)
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
