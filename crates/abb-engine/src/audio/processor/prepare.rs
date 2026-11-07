use crate::audio::AudioFile;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::preview_config;
use crate::processing::ProcessingContext;

use super::workspace::Workspace;
use super::ProcessingWorkflow;

pub(crate) fn validate_processing_inputs(files: &[AudioFile]) -> Result<()> {
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

    Ok(())
}

pub(crate) fn prepare_workspace(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    let workspace = Workspace::open(context)?;
    let total_duration = progress_total_duration(files, context.is_preview());

    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }

    Ok(ProcessingWorkflow::new(workspace, total_duration))
}

pub(crate) fn progress_total_duration(files: &[AudioFile], preview: bool) -> f64 {
    let durations: Vec<f64> = files
        .iter()
        .map(|file| file.duration.unwrap_or_default())
        .collect();
    if preview {
        preview_config::title_seconds(&durations)
    } else {
        durations.iter().sum()
    }
}

pub(crate) fn validate_and_prepare(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    validate_processing_inputs(files)?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    prepare_workspace(context, files)
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
