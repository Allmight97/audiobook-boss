//! Core audio processing and merge implementation

use super::{AudioFile, AudioSettings, ProgressReporter, ProcessingStage, SampleRateConfig};
use crate::errors::{AppError, Result};
use crate::ffmpeg::FFmpegError;
use crate::metadata::{AudiobookMetadata, write_metadata};
use lofty::probe::Probe;
use lofty::file::AudioFile as LoftyAudioFile;
use std::collections::HashMap;
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

/// Detects the most common sample rate from input files
pub fn detect_input_sample_rate(file_paths: &[PathBuf]) -> Result<u32> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "Cannot detect sample rate: no input files provided".to_string()
        ));
    }
    
    let mut sample_rates = HashMap::new();
    let mut first_rate = None;
    
    for path in file_paths {
        match get_file_sample_rate(path) {
            Ok(rate) => {
                if first_rate.is_none() {
                    first_rate = Some(rate);
                }
                *sample_rates.entry(rate).or_insert(0) += 1;
            }
            Err(e) => {
                // Log the error but continue with other files
                eprintln!("Warning: Could not read sample rate from {}: {}", path.display(), e);
            }
        }
    }
    
    if sample_rates.is_empty() {
        return Err(AppError::InvalidInput(
            "Cannot detect sample rate: no valid audio files found".to_string()
        ));
    }
    
    // Return the most common sample rate
    let most_common = sample_rates.iter()
        .max_by_key(|(_, &count)| count)
        .map(|(&rate, _)| rate);
    
    match most_common {
        Some(rate) => Ok(rate),
        None => first_rate.ok_or_else(|| AppError::InvalidInput(
            "Cannot determine sample rate from input files".to_string()
        )),
    }
}

/// Gets sample rate from a single audio file
fn get_file_sample_rate(path: &Path) -> Result<u32> {
    let tagged_file = Probe::open(path)
        .map_err(AppError::Metadata)?
        .read()
        .map_err(AppError::Metadata)?;
    
    let properties = tagged_file.properties();
    properties.sample_rate()
        .ok_or_else(|| AppError::InvalidInput(
            format!("File {} has no sample rate information", path.display())
        ))
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
        &mut reporter,
        &files
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
    files: &[AudioFile],
) -> Result<PathBuf> {
    let temp_output = concat_file.parent()
        .ok_or_else(|| AppError::FileValidation("Invalid concat file path".to_string()))?
        .join("merged.m4b");
    
    // Extract file paths for sample rate detection
    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    
    // Build FFmpeg command
    let cmd = build_merge_command(concat_file, &temp_output, settings, &file_paths)?;
    
    // Execute with progress tracking
    execute_with_progress(cmd, reporter).await?;
    
    Ok(temp_output)
}

/// Builds FFmpeg command for merging
fn build_merge_command(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
    file_paths: &[PathBuf],
) -> Result<Command> {
    let ffmpeg_path = crate::ffmpeg::locate_ffmpeg()?;
    
    // Resolve sample rate (auto-detect if needed)
    let sample_rate = match &settings.sample_rate {
        SampleRateConfig::Explicit(rate) => *rate,
        SampleRateConfig::Auto => detect_input_sample_rate(file_paths)?,
    };
    
    let mut cmd = Command::new(ffmpeg_path);
    cmd.args([
        "-f", "concat",
        "-safe", "0",
        "-i", &concat_file.to_string_lossy(),
        "-vn",  // Disable video processing (ignore album artwork)
        "-map", "0:a",  // Only map audio streams
        "-c:a", "libfdk_aac",
        "-b:a", &format!("{}k", settings.bitrate),
        "-ar", &sample_rate.to_string(),
        "-ac", &settings.channels.channel_count().to_string(),
        "-progress", "pipe:2",  // Enable progress output to stderr
        "-nostats",  // Disable normal stats output to avoid interference
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
    emit_progress(&ProcessingStage::Converting, 10.0, "Starting audio conversion...");
    
    // Calculate total duration for progress tracking
    let total_duration: f64 = files.iter()
        .filter(|f| f.is_valid)
        .map(|f| f.duration.unwrap_or(0.0))
        .sum();
    
    // Log basic info for debugging
    eprintln!("Starting FFmpeg merge - Total duration: {:.2}s, Bitrate: {}k", 
              total_duration, settings.bitrate);
    
    let merged_output = merge_audio_files_with_events(
        &concat_file,
        &settings,
        &mut reporter,
        &window,
        &state,
        total_duration,
        &files
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
    total_duration: f64,
    files: &[AudioFile],
) -> Result<PathBuf> {
    let temp_output = concat_file.parent()
        .ok_or_else(|| AppError::FileValidation("Invalid concat file path".to_string()))?
        .join("merged.m4b");
    
    // Extract file paths for sample rate detection
    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    
    // Build FFmpeg command
    let cmd = build_merge_command(concat_file, &temp_output, settings, &file_paths)?;
    
    // Execute with progress tracking and events
    execute_with_progress_events(cmd, reporter, window, state, total_duration).await?;
    
    Ok(temp_output)
}

/// Checks for cancellation and kills process if needed
fn check_cancellation_and_kill(
    state: &tauri::State<'_, crate::ProcessingState>,
    child: &mut std::process::Child,
) -> Result<()> {
    let is_cancelled = state.is_cancelled.lock()
        .map_err(|_| AppError::InvalidInput("Failed to check cancellation state".to_string()))?;
    
    if *is_cancelled {
        eprintln!("Cancellation detected, killing FFmpeg process...");
        let _ = child.kill();
        
        // Wait for process to actually terminate
        for i in 0..20 {  // Try for 2 seconds max
            if let Ok(Some(_)) = child.try_wait() {
                eprintln!("FFmpeg process terminated successfully");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            if i == 19 {
                eprintln!("Warning: FFmpeg process may not have terminated cleanly");
            }
        }
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    Ok(())
}

/// Parses speed multiplier from FFmpeg output line
fn parse_speed_multiplier(line: &str) -> Option<f64> {
    if line.contains("speed=") {
        if let Some(start) = line.find("speed=") {
            let speed_part = &line[start + 6..];
            if let Some(end) = speed_part.find('x') {
                if let Ok(speed) = speed_part[..end].parse::<f64>() {
                    return Some(speed);
                }
            }
        }
    }
    None
}

/// Processes progress update and emits events
fn process_progress_update(
    progress_time: f32,
    last_progress_time: &mut f32,
    progress_count: &mut i32,
    estimated_total_time: &mut f64,
    total_duration: f64,
    speed_multiplier: Option<f64>,
    window: &tauri::Window,
) -> Result<()> {
    if progress_time == 100.0 {
        eprint!("\rConverting: Done!                                          \n");
        let event = ProgressEvent {
            stage: "converting".to_string(),
            percentage: 90.0,
            message: "Finalizing audio conversion...".to_string(),
            current_file: None,
            eta_seconds: None,
        };
        let _ = window.emit("processing-progress", &event);
    } else if progress_time > *last_progress_time {
        *last_progress_time = progress_time;
        *progress_count += 1;
        
        // Use known total duration if available
        if *estimated_total_time == 0.0 && total_duration > 0.0 {
            *estimated_total_time = total_duration;
        } else if *progress_count > 5 && *estimated_total_time == 0.0 {
            *estimated_total_time = progress_time as f64 * 10.0; // Conservative estimate
        }
        
        let progress_percentage = calculate_and_display_progress(
            progress_time,
            *estimated_total_time,
            *progress_count,
            speed_multiplier,
        );
        
        let event = ProgressEvent {
            stage: "converting".to_string(),
            percentage: progress_percentage.min(79.0) as f32,
            message: "Converting and merging audio files...".to_string(),
            current_file: None,
            eta_seconds: None,
        };
        let _ = window.emit("processing-progress", &event);
    }
    Ok(())
}

/// Calculates and displays progress information
fn calculate_and_display_progress(
    progress_time: f32,
    estimated_total_time: f64,
    progress_count: i32,
    speed_multiplier: Option<f64>,
) -> f64 {
    if estimated_total_time > 0.0 {
        let file_progress = (progress_time as f64 / estimated_total_time).min(1.0);
        let percentage = 10.0 + (file_progress * 70.0); // Map to 10-80% range
        
        let speed_text = speed_multiplier
            .map(|s| format!(" [Speed: {s:.1}x]"))
            .unwrap_or_default();
        
        let eta_text = if let Some(speed) = speed_multiplier {
            let remaining_time = (estimated_total_time - progress_time as f64) / speed;
            if remaining_time > 0.0 {
                let minutes = (remaining_time / 60.0) as u32;
                let seconds = (remaining_time % 60.0) as u32;
                format!(" [ETA: {minutes}m {seconds}s]")
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        eprint!("\rConverting: {:.1}% ({:.1}s / {:.1}s){}{}", 
            file_progress * 100.0, 
            progress_time, 
            estimated_total_time,
            speed_text,
            eta_text);
        
        percentage
    } else {
        let percentage = 10.0 + ((progress_count as f64).min(50.0) * 1.4);
        eprint!("\rConverting: {percentage:.1}% (analyzing...)");
        percentage
    }
}

/// Executes command with progress tracking and event emission
async fn execute_with_progress_events(
    mut cmd: Command,
    _reporter: &mut ProgressReporter,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
    total_duration: f64,
) -> Result<()> {
    let mut child = cmd.spawn()
        .map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Failed to start FFmpeg".to_string())))?;
    
    // Track progress state
    let mut last_progress_time = 0.0;
    let mut estimated_total_time = 0.0;
    let mut progress_count = 0;
    
    // Read stderr for progress
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            check_cancellation_and_kill(state, &mut child)?;
            
            let line = line.map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Error reading FFmpeg output".to_string())))?;
            
            let speed_multiplier = parse_speed_multiplier(&line);

            // Parse progress from FFmpeg output and emit events
            if let Some(progress_time) = crate::audio::progress::parse_ffmpeg_progress(&line) {
                process_progress_update(
                    progress_time,
                    &mut last_progress_time,
                    &mut progress_count,
                    &mut estimated_total_time,
                    total_duration,
                    speed_multiplier,
                    window,
                )?;
            }
            
            // Check for errors (but ignore case-insensitive matches in file paths)
            if (line.contains("Error") || line.contains("error")) && 
               !line.contains("Output") && !line.contains("Input") {
                eprintln!("FFmpeg error line: {line}");
                if line.contains("No such file") || line.contains("Invalid data") {
                    return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed(format!("FFmpeg error: {line}"))));
                }
            }
        }
    }
    
    // Check if process was cancelled before waiting
    let is_cancelled = state.is_cancelled.lock()
        .map_err(|_| AppError::InvalidInput("Failed to check cancellation state".to_string()))?;
    
    if *is_cancelled {
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    
    // Wait for completion only if not cancelled
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
    fn test_speed_parsing_logic() {
        // Test the speed parsing logic from the progress formatting
        let test_line = "frame=1234 fps=30 q=28.0 size=1024kB time=00:01:30.45 bitrate=128.0kbits/s speed=1.2x";
        
        let mut speed_multiplier = None;
        if test_line.contains("speed=") {
            if let Some(start) = test_line.find("speed=") {
                let speed_part = &test_line[start + 6..];
                if let Some(end) = speed_part.find('x') {
                    if let Ok(speed) = speed_part[..end].parse::<f64>() {
                        speed_multiplier = Some(speed);
                    }
                }
            }
        }
        
        assert_eq!(speed_multiplier, Some(1.2));
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
    
    #[test]
    fn test_detect_input_sample_rate_empty() {
        let result = detect_input_sample_rate(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no input files provided"));
    }
    
    #[test]
    fn test_detect_input_sample_rate_no_valid_files() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_file = temp_dir.path().join("invalid.mp3");
        fs::write(&invalid_file, b"not audio data").unwrap();
        
        let result = detect_input_sample_rate(&[invalid_file]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no valid audio files found"));
    }
    
    #[test] 
    fn test_get_file_sample_rate_nonexistent() {
        let nonexistent = PathBuf::from("/nonexistent/file.mp3");
        let result = get_file_sample_rate(&nonexistent);
        assert!(result.is_err());
    }
}
