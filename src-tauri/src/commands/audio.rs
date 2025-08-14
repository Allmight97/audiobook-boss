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

/// Processes multiple audio files into a single M4B audiobook
/// Merges files with specified settings and optional metadata
#[tauri::command]
pub async fn process_audiobook_files(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    file_paths: Vec<String>,
    settings: AudioSettings,
    metadata: Option<crate::metadata::AudiobookMetadata>,
) -> Result<String> {
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
    #[allow(deprecated)]
    #[cfg(feature = "legacy-adapters")]
    let result = audio::process_audiobook_with_events(
        window,
        state.clone(),
        file_info.files,
        settings,
        metadata,
    )
    .await;

    #[cfg(not(feature = "legacy-adapters"))]
    let result = {
        let session = audio::session::ProcessingSession::new();
        let context = audio::ProcessingContext::new(window, std::sync::Arc::new(session), settings);
        audio::processor::process_audiobook_with_context(context, file_info.files, metadata).await
    };

    // Reset processing state
    {
        let mut is_processing = state.is_processing.lock().map_err(|_| {
            AppError::InvalidInput("Failed to acquire processing lock".to_string())
        })?;
        *is_processing = false;
    }

    result
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

/// Basic merge command for two audio files
/// Merges files to a fixed output location for testing
#[tauri::command]
pub fn merge_audio_files(file1: String, file2: String) -> Result<String> {
    let input1 = PathBuf::from(&file1);
    let input2 = PathBuf::from(&file2);

    // Validate inputs exist
    if !input1.exists() {
        return Err(AppError::FileValidation(format!(
            "First input file not found: {file1}"
        )));
    }
    if !input2.exists() {
        return Err(AppError::FileValidation(format!(
            "Second input file not found: {file2}"
        )));
    }

    // Fixed output for testing
    let output = PathBuf::from("merged_output.m4b");

    // Create and execute FFmpeg command
    crate::ffmpeg::command::FFmpegCommand::new()?
        .add_input(input1)
        .add_input(input2)
        .set_output(output.clone())
        .execute()?;

    Ok(format!(
        "Successfully merged files to: {}",
        output.to_string_lossy()
    ))
}


