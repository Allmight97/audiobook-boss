use crate::audio::file_list::FileListInfo;
use crate::audio::settings_encoder::{validate_encoder_settings, EncoderSettings};
use crate::audio::{self, ChannelConfig};
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

/// Validates advanced encoder v2 settings (no side effects)
#[tauri::command]
pub fn validate_encoder_settings_cmd(settings: EncoderSettings) -> Result<String> {
    validate_encoder_settings(&settings)?;
    Ok("Encoder settings are valid".to_string())
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

/// Processes files using v2 payload (EncoderSettings + v2 shape). For now, routes
/// through the same pipeline by mapping to v1 AudioSettings. No behavior change.
#[tauri::command]
pub async fn process_audiobook_files_v2(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    payload: ProcessV2Payload,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    // Validate encoder settings (v2)
    validate_encoder_settings(&payload.settings)?;
    log::info!(
        "encoder_v2 summary: encoder={:?} bitrate={}k channels={} sample_rate={:?} aac_coder={:?} afterburner={:?} threads={:?}",
        payload.settings.encoder_type,
        payload.settings.bitrate_kbps,
        payload.settings.channels,
        payload.sample_rate,
        payload.settings.aac_coder,
        payload.settings.afterburner,
        payload.settings.threads
    );

    // Validate output path and create parent directories if needed
    // Note: Frontend sends full file path in output_dir field (legacy naming)
    let output_path = prepare_output_path(&payload.output_dir)?;

    // Minimal AudioSettings kept for legacy validation paths (bitrate/channel/sample rate)
    let mut settings_v1 = audio::AudioSettings::audiobook_preset();
    settings_v1.bitrate = payload.settings.bitrate_kbps as u32;
    settings_v1.channels = match payload.settings.channels {
        1 => ChannelConfig::Mono,
        2 => ChannelConfig::Stereo,
        other => {
            return Err(AppError::InvalidInput(format!(
                "Unsupported channel count: {} (allowed: 1 or 2)",
                other
            )))
        }
    };
    // Map sample rate from frontend payload (defaults to Auto if not provided)
    if let Some(sample_rate) = payload.sample_rate {
        settings_v1.sample_rate = sample_rate;
    }
    settings_v1.output_path = output_path.clone();

    // Reuse existing validation to ensure path/bitrate configuration is acceptable
    audio::validate_audio_settings(&settings_v1)?;

    // Set processing state
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
    let (message, preview_path_opt, preview_seconds_used) = {
        let session = audio::session::ProcessingSession::new();
        let final_output_path = output_path.clone();
        let output_config = audio::OutputConfig::new(final_output_path.clone());
        let mut context = audio::ProcessingContext::new(
            window,
            std::sync::Arc::new(session),
            settings_v1,
            output_config,
        );
        // Attach v2 encoder settings to context for downstream mapping
        context.encoder_settings_v2 = Some(payload.settings.clone());

        // Resolve preview seconds
        let mut preview_seconds_resolved: Option<f64> = None;
        if let Some(sec) = preview_seconds.or_else(|| {
            std::env::var("ABB_PREVIEW_SECONDS")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
        }) {
            if sec.is_finite() && sec > 0.0 {
                context.preview = Some(crate::audio::context::PreviewConfig { seconds: sec });
                log::info!("Preview requested (v2): seconds={:.3}", sec);
                preview_seconds_resolved = Some(sec);
            }
        }
        let is_preview = context.preview.is_some();
        let msg =
            audio::processor::process_audiobook_with_context(context, file_info.files, metadata)
                .await?;
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
        (msg, preview_path, preview_seconds_resolved)
    };

    // Reset processing state
    {
        let mut is_processing = state
            .is_processing
            .lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire processing lock".to_string()))?;
        *is_processing = false;
    }

    Ok(ProcessCommandResult {
        message,
        preview_file_path: preview_path_opt,
        preview_actual_seconds: preview_seconds_used,
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
}

/// Prepares the output path for the audiobook file.
///
/// Accepts a full file path (e.g., `/path/to/Author/2024-Title/Book.m4b`).
/// Creates parent directories if they don't exist.
///
/// # Contract
/// - Frontend sends the complete output file path (including filename)
/// - Creates parent directories as needed
/// - Extension validation is deferred to `validate_audio_settings`
fn prepare_output_path(output_path: &str) -> Result<PathBuf> {
    let path = Path::new(output_path);

    // Validate the path has a filename
    if path.file_name().is_none() {
        return Err(AppError::InvalidInput(
            "Output path must include a filename".to_string(),
        ));
    }

    // Create parent directory if needed (extension validated later by validate_audio_settings)
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

/// Cancels the current audio processing operation
/// Sets the cancellation flag in the shared processing state
#[tauri::command]
pub fn cancel_processing(state: tauri::State<crate::ProcessingState>) -> Result<String> {
    let mut is_cancelled = state
        .is_cancelled
        .lock()
        .map_err(|_| AppError::InvalidInput("Failed to acquire cancellation lock".to_string()))?;
    *is_cancelled = true;
    Ok("Processing cancellation requested".to_string())
}

// Removed legacy merge_audio_files command and shell-based implementation during nuclear cleanup.
