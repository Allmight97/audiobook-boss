// Basic Tauri commands module
// This module contains simple commands for testing Tauri integration

use std::path::PathBuf;
use crate::ffmpeg;
use crate::errors::{AppError, Result};
use crate::metadata::{AudiobookMetadata, read_metadata, write_metadata};
use crate::audio::{AudioSettings, file_list::FileListInfo};
use crate::audio::constants::*;

/// Simple ping command that returns "pong"
/// Used for testing basic Tauri command functionality
#[tauri::command]
pub fn ping() -> Result<String> {
    Ok("pong".to_string())
}

/// Echo command that returns the input string
/// Demonstrates parameter passing in Tauri commands
#[tauri::command]
pub fn echo(input: String) -> Result<String> {
    Ok(input)
}

/// Validates that all provided file paths exist and are files
/// Accepts an array of file paths and checks file existence
#[tauri::command]
pub fn validate_files(file_paths: Vec<String>) -> Result<String> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput("No files provided for validation".to_string()));
    }

    let mut validated_count = 0;
    let mut missing_files = Vec::new();

    for path_str in file_paths {
        let path = PathBuf::from(&path_str);
        
        if path.exists() {
            if path.is_file() {
                validated_count += 1;
            } else {
                missing_files.push(format!("Path is not a file: {path_str}"));
            }
        } else {
            missing_files.push(format!("File not found: {path_str}"));
        }
    }

    if !missing_files.is_empty() {
        return Err(AppError::FileValidation(missing_files.join("; ")));
    }

    Ok(format!("Successfully validated {validated_count} files"))
}

/// Get FFmpeg version information
/// Returns version string if FFmpeg is available
#[tauri::command]
pub fn get_ffmpeg_version() -> Result<String> {
    Ok(ffmpeg::command::FFmpegCommand::version()?)
}

/// Basic merge command for two audio files
/// Merges files to a fixed output location for testing
#[tauri::command]
pub fn merge_audio_files(
    file1: String, 
    file2: String
) -> Result<String> {
    let input1 = PathBuf::from(&file1);
    let input2 = PathBuf::from(&file2);
    
    // Validate inputs exist
    if !input1.exists() {
        return Err(AppError::FileValidation(format!("First input file not found: {file1}")));
    }
    if !input2.exists() {
        return Err(AppError::FileValidation(format!("Second input file not found: {file2}")));
    }
    
    // Fixed output for testing
    let output = PathBuf::from("merged_output.m4b");
    
    // Create and execute FFmpeg command
    ffmpeg::command::FFmpegCommand::new()?
        .add_input(input1)
        .add_input(input2)
        .set_output(output.clone())
        .execute()?;
        
    Ok(format!(
        "Successfully merged files to: {}", 
        output.to_string_lossy()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping() {
        let result = ping();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "pong");
    }

    #[test]
    fn test_echo() {
        let test_string = "Hello, Tauri!".to_string();
        let result = echo(test_string.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_string);
    }

    #[test]
    fn test_validate_files_empty() {
        let result = validate_files(vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No files provided for validation"));
    }

    #[test]
    fn test_validate_files_nonexistent() {
        let files = vec!["nonexistent_file.txt".to_string()];
        let result = validate_files(files);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[test]
    fn test_get_ffmpeg_version() {
        let result = get_ffmpeg_version();
        // This test might fail if FFmpeg isn't installed
        // We just verify the function runs without panic
        
        if result.is_ok() {
            assert!(result.unwrap().contains("ffmpeg version"));
        } else {
            // If FFmpeg isn't found, we should get a specific error
            assert!(result.unwrap_err().to_string().contains("not found"));
        }
    }

    #[test]
    fn test_merge_audio_files_nonexistent() {
        let result = merge_audio_files(
            "nonexistent1.mp3".to_string(),
            "nonexistent2.mp3".to_string()
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file not found"));
    }
}

/// Reads metadata from an audio file
/// Returns metadata as JSON-serializable struct
#[tauri::command]
pub fn read_audio_metadata(file_path: String) -> Result<AudiobookMetadata> {
    read_metadata(&file_path)
}

/// Writes metadata to an existing M4B file
/// Accepts file path and metadata object
#[tauri::command]
pub fn write_audio_metadata(
    file_path: String,
    metadata: AudiobookMetadata
) -> Result<()> {
    write_metadata(&file_path, &metadata)
}

/// Writes cover art to an M4B file
/// Accepts file path and base64-encoded image data
#[tauri::command]
pub fn write_cover_art(
    file_path: String,
    cover_data: Vec<u8>
) -> Result<()> {
    use crate::metadata::writer::write_cover_art as write_cover;
    write_cover(&file_path, &cover_data)
}

/// Loads image file from disk and returns as byte array
/// Supports common image formats: jpg, jpeg, png, webp
#[tauri::command]
pub async fn load_cover_art_file(file_path: String) -> Result<Vec<u8>> {
    use std::fs;
    
    let path = PathBuf::from(&file_path);
    
    // Validate file exists
    if !path.exists() {
        return Err(AppError::FileValidation(format!("Image file not found: {file_path}")));
    }
    
    if !path.is_file() {
        return Err(AppError::FileValidation(format!("Path is not a file: {file_path}")));
    }
    
    // Validate file extension
    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| AppError::InvalidInput("File has no extension".to_string()))?;
    
    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "webp" => {},
        _ => return Err(AppError::InvalidInput(format!(
            "Unsupported image format: {extension}. Supported formats: jpg, jpeg, png, webp"
        )))
    }
    
    // Read file contents
    let image_data = fs::read(&path)
        .map_err(AppError::Io)?;
    
    // Validate it's not empty
    if image_data.is_empty() {
        return Err(AppError::InvalidInput("Image file appears to be empty".to_string()));
    }
    
    // Basic format validation by checking file headers
    validate_image_format(&image_data, &extension)?;
    
    Ok(image_data)
}

/// Validates image format by checking file headers
fn validate_image_format(data: &[u8], extension: &str) -> Result<()> {
    if data.len() < MIN_IMAGE_SIZE {
        return Err(AppError::InvalidInput("Image file too small to validate".to_string()));
    }
    
    match extension {
        "jpg" | "jpeg" => {
            if data.len() >= JPEG_HEADER.len() && data[..JPEG_HEADER.len()] == JPEG_HEADER {
                Ok(())
            } else {
                Err(AppError::InvalidInput("Invalid JPEG file format".to_string()))
            }
        },
        "png" => {
            if data.len() >= MIN_PNG_SIZE && data[..PNG_HEADER.len()] == PNG_HEADER {
                Ok(())
            } else {
                Err(AppError::InvalidInput("Invalid PNG file format".to_string()))
            }
        },
        "webp" => {
            if data.len() >= MIN_WEBP_SIZE && 
               &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
                Ok(())
            } else {
                Err(AppError::InvalidInput("Invalid WebP file format".to_string()))
            }
        },
        _ => Ok(()) // Already validated in main function
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_read_metadata_nonexistent() {
        let result = read_audio_metadata("nonexistent.m4b".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[test]
    fn test_write_metadata_nonexistent() {
        let metadata = AudiobookMetadata::new();
        let result = write_audio_metadata("nonexistent.m4b".to_string(), metadata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[test]
    fn test_write_cover_art_nonexistent() {
        let cover_data = vec![0u8; 100];
        let result = write_cover_art("nonexistent.m4b".to_string(), cover_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[tokio::test]
    async fn test_load_cover_art_file_nonexistent() {
        let result = load_cover_art_file("nonexistent.jpg".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Image file not found"));
    }

    #[tokio::test] 
    async fn test_load_cover_art_file_invalid_extension() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"not an image").unwrap();
        
        let result = load_cover_art_file(file_path.to_string_lossy().to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported image format"));
    }

    #[test]
    fn test_read_metadata_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid.txt");
        fs::write(&file_path, b"not audio").unwrap();
        
        let result = read_audio_metadata(file_path.to_string_lossy().to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metadata error"));
    }
}

/// Validates and analyzes a list of audio files
/// Returns comprehensive file information including duration and size
#[tauri::command]
pub fn analyze_audio_files(file_paths: Vec<String>) -> Result<FileListInfo> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    crate::audio::get_file_list_info(&paths)
}


/// Validates audio processing settings
/// Checks bitrate, sample rate, and output path validity
#[tauri::command]
pub fn validate_audio_settings(settings: AudioSettings) -> Result<String> {
    crate::audio::validate_audio_settings(&settings)?;
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
    metadata: Option<AudiobookMetadata>
) -> Result<String> {
    // Set processing state
    {
        let mut is_processing = state.is_processing.lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire processing lock".to_string()))?;
        *is_processing = true;
        
        let mut is_cancelled = state.is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire cancellation lock".to_string()))?;
        *is_cancelled = false;
    }
    
    // Validate and get file information
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    let file_info = crate::audio::get_file_list_info(&paths)?;
    
    // Process the audiobook with progress events
    #[allow(deprecated)]
    let result = crate::audio::process_audiobook_with_events(
        window,
        state.clone(),
        file_info.files,
        settings,
        metadata
    ).await;
    
    // Reset processing state
    {
        let mut is_processing = state.is_processing.lock()
            .map_err(|_| AppError::InvalidInput("Failed to acquire processing lock".to_string()))?;
        *is_processing = false;
    }
    
    result
}

/// Cancels the current audio processing operation
/// Sets the cancellation flag in the shared processing state
#[tauri::command]
pub fn cancel_processing(state: tauri::State<crate::ProcessingState>) -> Result<String> {
    let mut is_cancelled = state.is_cancelled.lock()
        .map_err(|_| AppError::InvalidInput("Failed to acquire cancellation lock".to_string()))?;
    *is_cancelled = true;
    Ok("Processing cancellation requested".to_string())
}

#[cfg(test)]
mod audio_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_analyze_audio_files_empty() {
        let result = analyze_audio_files(vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No files provided"));
    }

    #[test]
    fn test_analyze_audio_files_nonexistent() {
        let files = vec!["nonexistent.mp3".to_string()];
        let result = analyze_audio_files(files).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(!result.files[0].is_valid);
        assert_eq!(result.valid_count, 0);
        assert_eq!(result.invalid_count, 1);
    }

    #[test]
    fn test_validate_audio_settings_valid() {
        let temp_dir = TempDir::new().unwrap();
        let mut settings = AudioSettings::audiobook_preset();
        settings.output_path = temp_dir.path().join("test.m4b");
        let result = validate_audio_settings(settings);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Settings are valid");
    }

    #[test]
    fn test_validate_audio_settings_invalid_bitrate() {
        let mut settings = AudioSettings::audiobook_preset();
        settings.bitrate = 256; // Invalid - too high
        let result = validate_audio_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Bitrate must be"));
    }

}
