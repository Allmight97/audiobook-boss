use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::settings_encoder::{
    validate_encoder_settings as validate_encoder_settings_impl,
    validate_requested_encoder_available, EncoderSettings,
};
use crate::audio::toolchain::{
    detect_encoder_availability, EncoderAvailability, ExternalToolchainPreference,
};
use crate::commands::CommandResult;
use crate::errors::AppError;
use crate::metadata::{MetadataIntentPatch, NamingMetadata};
use crate::output_artifact::{build_output_path_preview, derive_output_artifact_path, OutputKind};
use crate::processing::job_registry::JobId;
use crate::processing::run;
pub use crate::processing::{
    JobType, OutputNamingConfig, ProcessCommandResult, ProcessPayload, ProcessResultEntry,
    ProcessResultStatus, ProcessResultSummary, ProcessingPreflightPlan,
};
use std::collections::HashMap;
use std::path::PathBuf;

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

/// Validates encoder settings (no side effects)
#[tauri::command]
#[specta::specta]
pub fn validate_encoder_settings(
    settings: EncoderSettings,
    external_toolchain: Option<ExternalToolchainPreference>,
) -> CommandResult<String> {
    validate_encoder_settings_impl(&settings)?;

    let availability = detect_encoder_availability(external_toolchain.as_ref());
    validate_requested_encoder_available(settings.encoder_type, &availability)?;

    Ok("Encoder settings are valid".to_string())
}

/// Lists runtime encoder availability so the UI can surface guidance.
#[tauri::command]
#[specta::specta]
pub fn list_available_encoders(
    external_toolchain: Option<ExternalToolchainPreference>,
) -> EncoderAvailability {
    log::info!("🔍 list_available_encoders command invoked");
    let result = detect_encoder_availability(external_toolchain.as_ref());
    log::info!("🔍 Returning encoder availability: {:?}", result);
    result
}

/// Re-runs external toolchain detection so the UI can refresh FDK status without restart.
#[tauri::command]
#[specta::specta]
pub fn refresh_external_toolchain(
    external_toolchain: Option<ExternalToolchainPreference>,
) -> EncoderAvailability {
    detect_encoder_availability(external_toolchain.as_ref())
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

    Ok(run::process_payload(
        window,
        registry.inner().clone(),
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
