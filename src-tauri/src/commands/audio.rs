use crate::audio;
use crate::audio::file_list::FileListInfo;
use crate::audio::job_registry::JobId;
use crate::audio::settings_encoder::{
    detect_available_encoders, validate_encoder_settings, EncoderAvailability, EncoderSettings,
};
use crate::errors::{AppError, Result};
use std::path::{Path, PathBuf};
// removed duplicate PathBuf import

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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    /// Sample rate from frontend (optional, defaults to Auto)
    pub sample_rate: Option<audio::SampleRateConfig>,
}

/// Processes files using the encoder settings payload (`process_audiobook_files_v2` command name retained for compatibility).
///
/// This command now supports parallel batch processing via the JobRegistry.
/// Multiple invocations can run concurrently up to the configured limit.
// EXCEPTION: Function exceeds 100 lines due to job registry integration for parallel processing.
// Refactoring into smaller functions would obscure the linear flow and complicate error handling.
#[allow(clippy::too_many_lines)]
#[tauri::command]
pub async fn process_audiobook_files_v2(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    payload: ProcessV2Payload,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    // Validate encoder settings
    validate_encoder_settings(&payload.settings)?;
    log::info!(
        "encoder summary: encoder={:?} bitrate={}k bitrate_mode={:?} channels={:?} sample_rate={:?} afterburner={} threads={:?}",
        payload.settings.encoder_type,
        payload.settings.bitrate_kbps,
        payload.settings.bitrate_mode,
        payload.settings.channels,
        payload.sample_rate,
        payload.settings.afterburner,
        payload.settings.threads
    );

    // Validate output path and create parent directories if needed
    // Note: Frontend sends full file path in output_dir field (legacy naming)
    let output_path = prepare_output_path(&payload.output_dir)?;

    // Map sample rate from frontend payload (defaults to Auto if not provided)
    let sample_rate = payload.sample_rate.unwrap_or(audio::SampleRateConfig::Auto);

    // Validate derived settings (sample_rate/output path) without legacy encoder assumptions
    audio::settings::validate_sample_rate_config(&sample_rate)?;
    audio::settings::validate_output_path(&output_path)?;

    // Register job with the registry (blocks if at capacity)
    let (job_id, _permit) = registry.register_job().await?;
    log::info!(
        "Job {} started for output: {}",
        job_id,
        output_path.display()
    );

    // Get cancellation checker for this job
    let cancellation_checker = registry.cancellation_checker(job_id).await;

    // Also update legacy state for backward compatibility with any code that checks it
    {
        let mut is_processing = state
            .is_processing
            .lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire processing lock".to_string()))?;
        *is_processing = true;

        let mut is_cancelled = state.is_cancelled.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire cancellation lock".to_string())
        })?;
        *is_cancelled = false;
    }

    // Validate and get file information
    let paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&paths)?;

    // Process the audiobook with progress events
    let result = {
        // Create session using job registry for cancellation
        let session =
            audio::session::ProcessingSession::from_job_registry(job_id.0, cancellation_checker);
        let final_output_path = output_path.clone();
        let output_config = audio::OutputConfig::new(final_output_path.clone());
        let mut context = audio::ProcessingContext::new(
            window,
            std::sync::Arc::new(session),
            payload.settings.clone(),
            sample_rate,
            output_config,
        );

        // Set job_id on context for progress emission
        context.job_id = Some(job_id.to_string());

        // Resolve preview seconds
        let mut preview_seconds_resolved: Option<f64> = None;
        if let Some(sec) = preview_seconds.or_else(|| {
            std::env::var("ABB_PREVIEW_SECONDS")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
        }) {
            if sec.is_finite() && sec > 0.0 {
                context.preview = Some(crate::audio::context::PreviewConfig::new(sec));
                log::info!("Preview requested: total_seconds={:.3}", sec);
                preview_seconds_resolved = Some(sec);
            }
        }
        let is_preview = context.preview.is_some();

        // Execute processing
        let process_result =
            audio::processor::process_audiobook_with_context(context, file_info.files, metadata)
                .await;

        // Handle result
        match process_result {
            Ok(msg) => {
                let preview_path = if is_preview {
                    let final_output = &final_output_path;
                    let parent = final_output
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."));
                    let stem = final_output
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("output");
                    let p = parent.join(format!("{}.preview.m4b", stem));
                    Some(p.display().to_string())
                } else {
                    None
                };
                Ok((msg, preview_path, preview_seconds_resolved))
            }
            Err(e) => Err(e),
        }
    };

    // Complete or fail the job in registry
    match &result {
        Ok(_) => {
            registry.complete_job(job_id).await;
            log::info!("Job {} completed successfully", job_id);
        }
        Err(e) => {
            registry.fail_job(job_id, e.to_string()).await;
            log::error!("Job {} failed: {}", job_id, e);
        }
    }

    // Reset legacy processing state
    {
        let mut is_processing = state
            .is_processing
            .lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire processing lock".to_string()))?;
        *is_processing = false;
    }

    let (message, preview_path_opt, preview_seconds_used) = result?;

    Ok(ProcessCommandResult {
        message,
        preview_file_path: preview_path_opt,
        preview_actual_seconds: preview_seconds_used,
        job_id: job_id.to_string(),
    })
}

/// Processes multiple audio files into a single M4B audiobook
/// Merges files with specified settings and optional metadata
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCommandResult {
    pub message: String,
    pub preview_file_path: Option<String>,
    pub preview_actual_seconds: Option<f64>,
    pub job_id: String,
}

/// Prepares the output path for the audiobook file.
///
/// Accepts a full file path (e.g., `/path/to/Author/2024-Title/Book.m4b`).
/// Creates parent directories if they don't exist.
///
/// # Contract
/// - Frontend sends the complete output file path (including filename)
/// - Creates parent directories as needed
/// - Extension validation is performed later by `validate_output_path`
fn prepare_output_path(output_path: &str) -> Result<PathBuf> {
    let path = Path::new(output_path);

    // Validate the path has a filename
    if path.file_name().is_none() {
        return Err(AppError::InvalidInput(
            "Output path must include a filename".to_string(),
        ));
    }

    // Create parent directory if needed (extension validated later by validate_output_path)
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            log::info!("Creating output directory: {}", parent.display());
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::FileValidation(format!(
                    "Failed to create output directory '{}': {}",
                    parent.display(),
                    e
                ))
            })?;
        }
    }

    Ok(path.to_path_buf())
}

/// Cancels all active audio processing operations
/// Sets the global cancellation flag in the job registry
#[tauri::command]
pub async fn cancel_processing(
    state: tauri::State<'_, crate::ProcessingState>,
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

        // Also set legacy state for backward compatibility
        let mut is_cancelled = state.is_cancelled.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire cancellation lock".to_string())
        })?;
        *is_cancelled = true;

        Ok("All processing jobs cancellation requested".to_string())
    }
}

// Removed legacy merge_audio_files command and shell-based implementation during nuclear cleanup.
