use crate::audio;
use crate::audio::{
    detect_encoder_availability, encoder_settings_capabilities,
    validate_encoder_settings as validate_encoder_settings_impl, validate_input_audio_path,
    validate_requested_encoder_available, EncoderSettings, EncoderSettingsCapabilities,
    FileListInfo, SupportedAudioImportMetadata,
};
use crate::commands::CommandResult;
use crate::errors::AppError;
use crate::metadata::{MetadataIntentPatch, NamingMetadata};
use crate::opened_audio::OpenedAudioFileQueue;
use crate::output_artifact::{
    build_output_path_preview, derive_output_artifact_path, OutputKind, OutputNamingConfig,
};
use crate::processing::job_registry::JobId;
use crate::processing::run;
use crate::processing::{JobRegistry, MaxConcurrentJobsCapabilities};
pub use crate::processing::{
    JobType, ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessResultSummary, ProcessingPreflightPlan,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettingsCapabilities {
    pub encoder: EncoderSettingsCapabilities,
    pub max_concurrent_jobs: MaxConcurrentJobsCapabilities,
}

/// Validates that all provided file paths exist and are files
/// Accepts an array of file paths and checks file existence
#[tauri::command]
#[specta::specta]
pub fn validate_files(file_paths: Vec<String>) -> CommandResult<String> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput("No files provided for validation".to_string()).into());
    }

    let mut validated_count = 0;
    let mut validation_errors = Vec::new();

    for path_str in file_paths {
        let path = PathBuf::from(&path_str);

        match validate_input_audio_path(&path) {
            Ok(_canonical_path) => {
                validated_count += 1;
            }
            Err(e) => {
                validation_errors.push(e.to_string());
            }
        }
    }

    if !validation_errors.is_empty() {
        return Err(AppError::FileValidation(validation_errors.join("; ")).into());
    }

    Ok(format!("Successfully validated {validated_count} files"))
}

/// Validates and analyzes a list of audio files
/// Returns comprehensive file information including duration and size
#[tauri::command]
#[specta::specta]
pub fn analyze_audio_files(file_paths: Vec<String>) -> CommandResult<FileListInfo> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    Ok(audio::get_file_list_info(&paths)?)
}

/// Returns backend-owned supported local audio import metadata for picker UI.
#[tauri::command]
#[specta::specta]
pub fn get_supported_audio_import_metadata() -> CommandResult<SupportedAudioImportMetadata> {
    Ok(audio::supported_audio_import_metadata())
}

/// Recursively discovers supported local audio files from files and directories.
#[tauri::command]
#[specta::specta]
pub async fn discover_audio_import_paths(input_paths: Vec<String>) -> CommandResult<Vec<String>> {
    let paths = input_paths
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let discovered =
        tokio::task::spawn_blocking(move || audio::discover_audio_import_paths(&paths))
            .await
            .map_err(|error| {
                AppError::General(format!("Audio import discovery failed: {error}"))
            })??;

    Ok(discovered
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect())
}

/// Drains local audio paths opened by the OS before the frontend was ready.
#[tauri::command]
#[specta::specta]
pub fn take_opened_audio_files(
    queue: tauri::State<'_, OpenedAudioFileQueue>,
) -> CommandResult<Vec<String>> {
    Ok(queue.take_paths()?)
}

/// Validates encoder settings (no side effects)
#[tauri::command]
#[specta::specta]
pub fn validate_encoder_settings(settings: EncoderSettings) -> CommandResult<String> {
    validate_encoder_settings_impl(&settings)?;

    let availability = detect_encoder_availability();
    validate_requested_encoder_available(settings.encoder_type, &availability)?;

    Ok("Encoder settings are valid".to_string())
}

/// Returns backend-owned runtime settings capabilities for UI controls.
#[tauri::command]
#[specta::specta]
pub fn get_runtime_settings_capabilities() -> CommandResult<RuntimeSettingsCapabilities> {
    Ok(RuntimeSettingsCapabilities {
        encoder: encoder_settings_capabilities(),
        max_concurrent_jobs: JobRegistry::max_concurrent_jobs_capabilities(),
    })
}

/// Builds an output path preview using backend naming rules without collision suffixing.
#[tauri::command]
#[specta::specta]
pub fn preview_output_path(
    output_dir: String,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    output_naming: Option<OutputNamingConfig>,
    source_path: Option<String>,
    output_kind: Option<OutputKind>,
) -> CommandResult<String> {
    let base_output_dir = PathBuf::from(output_dir);
    let source_path_buf = source_path.as_deref().map(PathBuf::from);
    let naming = output_naming.unwrap_or_default();
    let draft_naming_metadata = metadata.as_ref().map(NamingMetadata::from_metadata);
    let requested = build_output_path_preview(
        &base_output_dir,
        draft_naming_metadata.as_ref(),
        naming,
        source_path_buf.as_deref(),
    )?;
    let artifact =
        derive_output_artifact_path(&requested, output_kind.unwrap_or(OutputKind::Final))?;
    Ok(artifact.to_string_lossy().to_string())
}

#[tauri::command]
#[specta::specta]
pub fn preflight_processing_plan(
    payload: ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> CommandResult<ProcessingPreflightPlan> {
    Ok(run::preflight_payload(payload, metadata, preview_seconds)?)
}

/// Returns the current maximum concurrent jobs setting
#[tauri::command]
#[specta::specta]
pub fn get_max_concurrent_jobs(registry: tauri::State<'_, crate::ManagedJobRegistry>) -> usize {
    registry.max_concurrent()
}

/// Updates the maximum concurrent jobs setting (requires idle state)
#[tauri::command]
#[specta::specta]
pub async fn set_max_concurrent_jobs(
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    max_concurrent: Option<usize>,
) -> CommandResult<usize> {
    let desired = max_concurrent.unwrap_or(crate::processing::JobRegistry::default_max());
    Ok(registry.update_max_concurrent(desired).await?)
}

/// Processes audiobook files with configurable encoder settings.
///
/// Supports parallel batch processing via the JobRegistry.
/// Multiple invocations can run concurrently up to the configured limit.
#[tauri::command]
#[specta::specta]
pub async fn process_audiobook_files(
    window: tauri::Window,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> CommandResult<ProcessCommandResult> {
    if registry.is_global_cancelled() && registry.get_aggregate_status().await.total_jobs == 0 {
        registry.reset_global_cancel();
    }
    let cache_dir = window
        .app_handle()
        .path()
        .app_cache_dir()
        .map_err(|error| {
            AppError::General(format!(
                "Failed to resolve processing workspace root: {error}"
            ))
        })?;
    let workspace_root = audio::processing_workspace_root(&cache_dir);

    Ok(run::process_payload(
        window,
        registry.inner().clone(),
        workspace_root,
        payload,
        metadata,
        preview_seconds,
    )
    .await?)
}

/// Cancels all active audio processing operations
/// Sets the global cancellation flag in the job registry
#[tauri::command]
#[specta::specta]
pub async fn cancel_processing(
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    job_id: Option<String>,
) -> CommandResult<String> {
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
