use crate::audio;
use crate::audio::{
    encoder_settings_capabilities, validate_input_audio_path, EncoderSettingsCapabilities,
    FileListInfo, SupportedAudioImportMetadata,
};
use crate::commands::CommandResult;
use crate::errors::AppError;
use crate::metadata::{MetadataIntentPatch, NamingMetadata};
use crate::opened_audio::OpenedAudioFileQueue;
use crate::output_artifact::{
    build_output_path_preview, derive_output_artifact_path, OutputKind, OutputNamingConfig,
};
use crate::processing::run;
use crate::processing::{JobRegistry, MaxConcurrentJobsCapabilities};
pub use crate::processing::{
    ProcessCommandResult, ProcessPayload, ProcessResultEntry, ProcessResultStatus,
    ProcessResultSummary, ProcessingPreflightPlan,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettingsCapabilities {
    pub encoder: EncoderSettingsCapabilities,
    pub max_concurrent_jobs: MaxConcurrentJobsCapabilities,
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

/// Returns backend-owned runtime settings capabilities for UI controls.
#[tauri::command]
#[specta::specta]
pub async fn get_runtime_settings_capabilities() -> CommandResult<RuntimeSettingsCapabilities> {
    Ok(tokio::task::spawn_blocking(|| RuntimeSettingsCapabilities {
        encoder: encoder_settings_capabilities(),
        max_concurrent_jobs: JobRegistry::max_concurrent_jobs_capabilities(),
    })
    .await
    .map_err(|error| AppError::General(error.to_string()))?)
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
    format: crate::audio::AudiobookFormat,
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
    let artifact = artifact.with_extension(format.extension());
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

fn require_preview_seconds(preview_seconds: Option<f64>) -> Result<f64, AppError> {
    preview_seconds.ok_or_else(|| {
        AppError::InvalidInput(
            "Direct processing requires a preview duration; submit final processing through WorkRuntime"
                .to_string(),
        )
    })
}

/// Processes a direct preview with configurable encoder settings.
///
/// Final processing must enter through WorkRuntime so it has durable
/// operation identity, snapshots, and operation and title cancellation.
#[tauri::command]
#[specta::specta]
pub async fn process_audiobook_files(
    window: tauri::Window,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> CommandResult<ProcessCommandResult> {
    let preview_seconds = require_preview_seconds(preview_seconds)?;

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
        Some(preview_seconds),
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use super::require_preview_seconds;
    use crate::errors::AppError;

    #[test]
    fn direct_processing_requires_preview_duration() {
        match require_preview_seconds(None) {
            Err(AppError::InvalidInput(message)) => assert_eq!(
                message,
                "Direct processing requires a preview duration; submit final processing through WorkRuntime"
            ),
            result => panic!("expected invalid-input error, got {result:?}"),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn preview_title_audio(
    file_paths: Vec<String>,
    request: audio::TitleAudioRequest,
    chapter_plans: Option<std::collections::HashMap<String, crate::metadata::ChapterPlan>>,
) -> CommandResult<audio::TitleAudioPlan> {
    let paths = file_paths
        .iter()
        .map(|path| validate_input_audio_path(std::path::Path::new(path)))
        .collect::<crate::errors::Result<Vec<_>>>()?;
    Ok(tokio::task::spawn_blocking(move || {
        let mut info = audio::get_file_list_info(&paths)?;
        audio::apply_chapter_plans(&mut info, chapter_plans.as_ref())?;
        audio::resolve_title_audio(&request, &info, false)
    })
    .await
    .map_err(|error| AppError::General(format!("Audio plan failed: {error}")))??)
}
