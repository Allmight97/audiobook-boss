use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::job_registry::JobId;
use crate::audio::settings_encoder::{
    detect_available_encoders, validate_encoder_settings, EncoderAvailability, EncoderSettings,
};
use crate::commands::audio_processing;
pub use crate::commands::audio_types::{
    JobType, OutputNamingConfig, ProcessCommandResult, ProcessV2Payload,
};
use crate::errors::{AppError, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Validates that all provided file paths exist and are files
/// Accepts an array of file paths and checks file existence
#[tauri::command]
pub fn validate_files(file_paths: Vec<String>) -> Result<String> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "No files provided for validation".to_string(),
        ));
    }

    let mut validated_count = 0;
    let mut validation_errors = Vec::new();

    for path_str in file_paths {
        let path = PathBuf::from(&path_str);

        match audio::path_validation::validate_input_audio_path(&path) {
            Ok(_canonical_path) => {
                validated_count += 1;
            }
            Err(e) => {
                validation_errors.push(e.to_string());
            }
        }
    }

    if !validation_errors.is_empty() {
        return Err(AppError::FileValidation(validation_errors.join("; ")));
    }

    Ok(format!("Successfully validated {validated_count} files"))
}

/// Validates and analyzes a list of audio files
/// Returns comprehensive file information including duration and size
#[tauri::command]
pub fn analyze_audio_files(file_paths: Vec<String>) -> Result<FileListInfo> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    audio::get_file_list_info(&paths)
}

/// Validates encoder settings (no side effects)
#[tauri::command]
pub fn validate_encoder_settings_cmd(settings: EncoderSettings) -> Result<String> {
    validate_encoder_settings(&settings)?;
    Ok("Encoder settings are valid".to_string())
}

/// Lists runtime encoder availability so the UI can surface guidance.
#[tauri::command]
pub fn list_available_encoders() -> EncoderAvailability {
    log::info!("🔍 list_available_encoders command invoked");
    let result = detect_available_encoders();
    log::info!("🔍 Returning encoder availability: {:?}", result);
    result
}

/// Returns the current maximum concurrent jobs setting
#[tauri::command]
pub fn get_max_concurrent_jobs(registry: tauri::State<'_, crate::ManagedJobRegistry>) -> usize {
    registry.max_concurrent()
}

/// Updates the maximum concurrent jobs setting (requires idle state)
#[tauri::command]
pub async fn set_max_concurrent_jobs(
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    max_concurrent: Option<usize>,
) -> Result<usize> {
    let desired = max_concurrent.unwrap_or(crate::audio::JobRegistry::default_max());
    registry.update_max_concurrent(desired).await
}

/// Processes files using the encoder settings payload (`process_audiobook_files_v2` command name retained for compatibility).
///
/// This command now supports parallel batch processing via the JobRegistry.
/// Multiple invocations can run concurrently up to the configured limit.
#[tauri::command]
pub async fn process_audiobook_files_v2(
    window: tauri::Window,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    payload: ProcessV2Payload,
    metadata: Option<HashMap<String, crate::metadata::AudiobookMetadata>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    audio_processing::process_payload(
        window,
        registry.inner().clone(),
        payload,
        metadata,
        preview_seconds,
    )
    .await
}

/// Cancels all active audio processing operations
/// Sets the global cancellation flag in the job registry
#[tauri::command]
pub async fn cancel_processing(
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    job_id: Option<String>,
) -> Result<String> {
    if let Some(id) = job_id {
        let parsed = JobId::parse(&id)?;
        registry.cancel_job(parsed).await?;
        Ok(format!("Cancellation requested for job {}", id))
    } else {
        // Cancel all jobs in the registry
        registry.cancel_all();

        Ok("All processing jobs cancellation requested".to_string())
    }
}

// Removed legacy merge_audio_files command and shell-based implementation during nuclear cleanup.
