use crate::errors::{AppError, Result};
use crate::audio::{self, AudioSettings};
use crate::audio::file_list::FileListInfo;
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

/// Validates audio processing settings
/// Checks bitrate, sample rate, and output path validity
#[tauri::command]
pub fn validate_audio_settings(settings: AudioSettings) -> Result<String> {
    audio::validate_audio_settings(&settings)?;
    Ok("Settings are valid".to_string())
}

/// Validates encoder v2 settings 
/// Checks bitrate whitelist, channel constraints, thread settings, and HE-AAC v2 stereo enforcement
#[tauri::command]
pub fn validate_encoder_settings(settings: crate::audio::EncoderSettings) -> Result<String> {
    audio::validate_encoder_settings(&settings)?;
    Ok("Encoder settings are valid".to_string())
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

#[tauri::command]
pub async fn process_audiobook_files(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    file_paths: Vec<String>,
    settings: AudioSettings,
    metadata: Option<crate::metadata::AudiobookMetadata>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    // Set processing state
    {
        let mut is_processing = state.is_processing.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire processing lock".to_string())
        })?;
        *is_processing = true;

        let mut is_cancelled = state.is_cancelled.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire cancellation lock".to_string())
        })?;
        *is_cancelled = false;
    }

    // Validate and get file information
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&paths)?;

    // Process the audiobook with progress events
    let (message, preview_path_opt, preview_seconds_used) = {
        // Single engine path: ffmpeg-next context based processing
        let session = audio::session::ProcessingSession::new();
        // Clone settings for deriving preview path later (original moved into context)
        let settings_for_path = settings.clone();
        let mut context = audio::ProcessingContext::new(window, std::sync::Arc::new(session), settings);
        // Resolve preview seconds from payload or environment fallback
        let mut preview_seconds_resolved: Option<f64> = None;
        if let Some(sec) = preview_seconds.or_else(|| {
            std::env::var("ABB_PREVIEW_SECONDS").ok().and_then(|s| s.parse::<f64>().ok())
        }) {
            if sec.is_finite() && sec > 0.0 {
                context.preview = Some(crate::audio::context::PreviewConfig { seconds: sec });
                log::info!("Preview requested: seconds={:.3}", sec);
                preview_seconds_resolved = Some(sec);
            }
        }
        let is_preview = context.preview.is_some();
        let msg = audio::processor::process_audiobook_with_context(context, file_info.files, metadata).await?;
        let preview_path = if is_preview {
            // Derive preview path deterministically based on output settings
            let final_output = &settings_for_path.output_path;
            let parent = final_output.parent().unwrap_or_else(|| std::path::Path::new("."));
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
        let mut is_processing = state.is_processing.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire processing lock".to_string())
        })?;
        *is_processing = false;
    }

    Ok(ProcessCommandResult { message, preview_file_path: preview_path_opt, preview_actual_seconds: preview_seconds_used })
}

/// Cancels the current audio processing operation
/// Sets the cancellation flag in the shared processing state
#[tauri::command]
pub fn cancel_processing(state: tauri::State<crate::ProcessingState>) -> Result<String> {
    let mut is_cancelled = state.is_cancelled.lock().map_err(|_| {
        AppError::InvalidInput("Failed to acquire cancellation lock".to_string())
    })?;
    *is_cancelled = true;
    Ok("Processing cancellation requested".to_string())
}

// Removed legacy merge_audio_files command and shell-based implementation during nuclear cleanup.


