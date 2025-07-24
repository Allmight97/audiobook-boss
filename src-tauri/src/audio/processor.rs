//! Core audio processing and merge implementation

use super::{AudioFile, AudioSettings, ProgressReporter, ProcessingStage};
use crate::errors::{AppError, Result};
use crate::ffmpeg::FFmpegError;
use crate::metadata::{AudiobookMetadata, write_metadata};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use serde::Serialize;
use tauri::Emitter;

/// Progress event for frontend communication
#[derive(Clone, Serialize)]
struct ProgressEvent {
    stage: String,
    percentage: f32,
    message: String,
    current_file: Option<String>,
    eta_seconds: Option<f64>,
}

/// Main function to process audiobook from multiple files
#[allow(dead_code)]
pub async fn process_audiobook(
    files: Vec<AudioFile>,
    settings: AudioSettings,
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    let mut reporter = ProgressReporter::new(files.len());
    
    // Validate inputs
    validate_processing_inputs(&files, &settings)?;
    
    // Stage 1: Analyze files
    reporter.set_stage(ProcessingStage::Analyzing);
    let temp_dir = create_temp_directory()?;
    let concat_file = create_concat_file(&files, &temp_dir)?;
    
    // Stage 2: Convert and merge files
    reporter.set_stage(ProcessingStage::Converting);
    let merged_output = merge_audio_files_with_progress(
        &concat_file,
        &settings,
        &mut reporter
    ).await?;
    
    // Stage 3: Write metadata if provided
    if let Some(metadata) = metadata {
        reporter.set_stage(ProcessingStage::WritingMetadata);
        write_metadata(&merged_output, &metadata)?;
    }
    
    // Stage 4: Move to final location
    let final_output = move_to_final_location(merged_output, &settings.output_path)?;
    
    // Cleanup
    cleanup_temp_directory(temp_dir)?;
    
    reporter.complete();
    Ok(format!("Successfully created audiobook: {}", final_output.display()))
}

/// Validates processing inputs
fn validate_processing_inputs(
    files: &[AudioFile],
    settings: &AudioSettings
) -> Result<()> {
    if files.is_empty() {
        return Err(AppError::InvalidInput("No files to process".to_string()));
    }
    
    // Check all files are valid
    for file in files {
        if !file.is_valid {
            return Err(AppError::FileValidation(
                format!("Invalid file: {} - {}", 
                       file.path.display(),
                       file.error.as_deref().unwrap_or("Unknown error"))
            ));
        }
    }
    
    // Validate settings
    crate::audio::settings::validate_audio_settings(settings)?;
    
    Ok(())
}

/// Creates temporary directory for processing
fn create_temp_directory() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir().join("audiobook-boss");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| AppError::FileValidation(
            format!("Cannot create temp directory: {e}")
        ))?;
    Ok(temp_dir)
}

/// Creates FFmpeg concat file for merging
fn create_concat_file(
    files: &[AudioFile],
    temp_dir: &Path
) -> Result<PathBuf> {
    let concat_file = temp_dir.join("concat.txt");
    
    let mut content = String::new();
    for file in files {
        // Escape file paths for FFmpeg
        let escaped_path = file.path.to_string_lossy().replace('\'', "'\"'\"'");
        content.push_str(&format!("file '{escaped_path}'\n"));
    }
    
    std::fs::write(&concat_file, content)
        .map_err(|e| AppError::FileValidation(
            format!("Cannot write concat file: {e}")
        ))?;
    
    Ok(concat_file)
}

/// Merges audio files with progress tracking
#[allow(dead_code)]
async fn merge_audio_files_with_progress(
    concat_file: &Path,
    settings: &AudioSettings,
    reporter: &mut ProgressReporter,
) -> Result<PathBuf> {
    let temp_output = concat_file.parent()
        .ok_or_else(|| AppError::FileValidation("Invalid concat file path".to_string()))?
        .join("merged.m4b");
    
    // Build FFmpeg command
    let cmd = build_merge_command(concat_file, &temp_output, settings)?;
    
    // Execute with progress tracking
    execute_with_progress(cmd, reporter).await?;
    
    Ok(temp_output)
}

/// Builds FFmpeg command for merging
fn build_merge_command(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
) -> Result<Command> {
    let ffmpeg_path = crate::ffmpeg::locate_ffmpeg()?;
    
    let mut cmd = Command::new(ffmpeg_path);
    cmd.args([
        "-f", "concat",
        "-safe", "0",
        "-i", &concat_file.to_string_lossy(),
        "-c:a", "aac",
        "-b:a", &format!("{}k", settings.bitrate),
        "-ar", &settings.sample_rate.to_string(),
        "-ac", &settings.channels.channel_count().to_string(),
        "-y",  // Overwrite output file
        &output.to_string_lossy(),
    ]);
    
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::piped());
    
    Ok(cmd)
}

/// Executes command with progress tracking
#[allow(dead_code)]
async fn execute_with_progress(
    mut cmd: Command,
    _reporter: &mut ProgressReporter,
) -> Result<()> {
    let mut child = cmd.spawn()
        .map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Failed to start FFmpeg".to_string())))?;
    
    // Read stderr for progress
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = line.map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Error reading FFmpeg output".to_string())))?;
            
            // Parse progress from FFmpeg output
            if let Some(_progress) = crate::audio::progress::parse_ffmpeg_progress(&line) {
                // Progress parsing could be enhanced here
                // For now, we'll just update that we're processing
            }
            
            // Check for errors
            if line.contains("Error") || line.contains("error") {
                return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed("FFmpeg reported an error".to_string())));
            }
        }
    }
    
    // Wait for completion
    let status = child.wait()
        .map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("FFmpeg execution failed".to_string())))?;
    
    if !status.success() {
        return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed("FFmpeg exited with error".to_string())));
    }
    
    Ok(())
}

/// Moves temporary output to final location
fn move_to_final_location(
    temp_output: PathBuf,
    final_path: &Path
) -> Result<PathBuf> {
    // Ensure parent directory exists
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::FileValidation(
                format!("Cannot create output directory: {e}")
            ))?;
    }
    
    std::fs::rename(&temp_output, final_path)
        .map_err(|e| AppError::FileValidation(
            format!("Cannot move file to final location: {e}")
        ))?;
    
    Ok(final_path.to_path_buf())
}

/// Main function to process audiobook with event emission for progress tracking
pub async fn process_audiobook_with_events(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    files: Vec<AudioFile>,
    settings: AudioSettings,
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    let mut reporter = ProgressReporter::new(files.len());
    
    // Helper function to emit progress events
    let emit_progress = |stage: &ProcessingStage, progress: f32, message: &str| {
        let stage_str = match stage {
            ProcessingStage::Analyzing => "analyzing",
            ProcessingStage::Converting => "converting", 
            ProcessingStage::Merging => "merging",
            ProcessingStage::WritingMetadata => "writing",
            ProcessingStage::Completed => "completed",
            ProcessingStage::Failed(_) => "failed",
        };
        
        let event = ProgressEvent {
            stage: stage_str.to_string(),
            percentage: progress,
            message: message.to_string(),
            current_file: None,
            eta_seconds: None,
        };
        
        let _ = window.emit("processing-progress", &event);
    };
    
    // Check for cancellation
    let check_cancelled = || -> Result<()> {
        let is_cancelled = state.is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to check cancellation state".to_string()))?;
        if *is_cancelled {
            return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
        }
        Ok(())
    };
    
    // Validate inputs
    emit_progress(&ProcessingStage::Analyzing, 0.0, "Validating input files...");
    validate_processing_inputs(&files, &settings)?;
    check_cancelled()?;
    
    // Stage 1: Analyze files
    reporter.set_stage(ProcessingStage::Analyzing);
    emit_progress(&ProcessingStage::Analyzing, 10.0, "Creating temporary workspace...");
    let temp_dir = create_temp_directory()?;
    let concat_file = create_concat_file(&files, &temp_dir)?;
    check_cancelled()?;
    
    // Stage 2: Convert and merge files
    reporter.set_stage(ProcessingStage::Converting);
    emit_progress(&ProcessingStage::Converting, 20.0, "Starting audio conversion...");
    let merged_output = merge_audio_files_with_events(
        &concat_file,
        &settings,
        &mut reporter,
        &window,
        &state
    ).await?;
    check_cancelled()?;
    
    // Stage 3: Write metadata if provided
    if let Some(metadata) = metadata {
        reporter.set_stage(ProcessingStage::WritingMetadata);
        emit_progress(&ProcessingStage::WritingMetadata, 90.0, "Writing metadata...");
        write_metadata(&merged_output, &metadata)?;
        check_cancelled()?;
    }
    
    // Stage 4: Move to final location
    emit_progress(&ProcessingStage::WritingMetadata, 95.0, "Moving to final location...");
    let final_output = move_to_final_location(merged_output, &settings.output_path)?;
    check_cancelled()?;
    
    // Cleanup
    emit_progress(&ProcessingStage::Completed, 98.0, "Cleaning up temporary files...");
    cleanup_temp_directory(temp_dir)?;
    
    reporter.complete();
    emit_progress(&ProcessingStage::Completed, 100.0, "Processing completed successfully!");
    
    Ok(format!("Successfully created audiobook: {}", final_output.display()))
}

/// Merges audio files with progress tracking and event emission
async fn merge_audio_files_with_events(
    concat_file: &Path,
    settings: &AudioSettings,
    reporter: &mut ProgressReporter,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
) -> Result<PathBuf> {
    let temp_output = concat_file.parent()
        .ok_or_else(|| AppError::FileValidation("Invalid concat file path".to_string()))?
        .join("merged.m4b");
    
    // Build FFmpeg command
    let cmd = build_merge_command(concat_file, &temp_output, settings)?;
    
    // Execute with progress tracking and events
    execute_with_progress_events(cmd, reporter, window, state).await?;
    
    Ok(temp_output)
}

/// Executes command with progress tracking and event emission
async fn execute_with_progress_events(
    mut cmd: Command,
    _reporter: &mut ProgressReporter,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
) -> Result<()> {
    let mut child = cmd.spawn()
        .map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Failed to start FFmpeg".to_string())))?;
    
    // Read stderr for progress
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            // Check for cancellation
            let is_cancelled = state.is_cancelled.lock()
                .map_err(|_| AppError::InvalidInput("Failed to check cancellation state".to_string()))?;
            
            if *is_cancelled {
                let _ = child.kill();
                return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
            }
            
            let line = line.map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Error reading FFmpeg output".to_string())))?;
            
            // Parse progress from FFmpeg output and emit events
            if let Some(progress) = crate::audio::progress::parse_ffmpeg_progress(&line) {
                let progress_percentage = 20.0 + (progress / 100.0 * 70.0); // Map to 20-90% range
                let event = ProgressEvent {
                    stage: "converting".to_string(),
                    percentage: progress_percentage,
                    message: "Converting and merging audio files...".to_string(),
                    current_file: None,
                    eta_seconds: None,
                };
                let _ = window.emit("processing-progress", &event);
            }
            
            // Check for errors
            if line.contains("Error") || line.contains("error") {
                return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed("FFmpeg reported an error".to_string())));
            }
        }
    }
    
    // Wait for completion
    let status = child.wait()
        .map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("FFmpeg execution failed".to_string())))?;
    
    if !status.success() {
        return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed("FFmpeg exited with error".to_string())));
    }
    
    Ok(())
}

/// Cleans up temporary directory
fn cleanup_temp_directory(temp_dir: PathBuf) -> Result<()> {
    std::fs::remove_dir_all(&temp_dir)
        .map_err(|e| AppError::FileValidation(
            format!("Cannot cleanup temp directory: {e}")
        ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_validate_processing_inputs_empty() {
        let files = vec![];
        let settings = AudioSettings::default();
        let result = validate_processing_inputs(&files, &settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No files to process"));
    }

    #[test]
    fn test_validate_processing_inputs_invalid_file() {
        let mut file = AudioFile::new("test.mp3".into());
        file.is_valid = false;
        file.error = Some("Test error".to_string());
        
        let files = vec![file];
        let settings = AudioSettings::default();
        let result = validate_processing_inputs(&files, &settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid file"));
    }

    #[test]
    fn test_create_temp_directory() {
        let result = create_temp_directory();
        assert!(result.is_ok());
        let temp_dir = result.unwrap();
        assert!(temp_dir.exists());
        
        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_create_concat_file() {
        let temp_dir = TempDir::new().unwrap();
        
        let mut file1 = AudioFile::new("/path/to/file1.mp3".into());
        file1.is_valid = true;
        let mut file2 = AudioFile::new("/path/to/file2.mp3".into());
        file2.is_valid = true;
        
        let files = vec![file1, file2];
        let result = create_concat_file(&files, temp_dir.path());
        assert!(result.is_ok());
        
        let concat_file = result.unwrap();
        assert!(concat_file.exists());
        
        let content = fs::read_to_string(&concat_file).unwrap();
        assert!(content.contains("file '/path/to/file1.mp3'"));
        assert!(content.contains("file '/path/to/file2.mp3'"));
    }
}
