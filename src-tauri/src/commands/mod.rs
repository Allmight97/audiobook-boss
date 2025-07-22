// Basic Tauri commands module
// This module contains simple commands for testing Tauri integration

use std::path::PathBuf;
use crate::ffmpeg;
use crate::errors::{AppError, Result};

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