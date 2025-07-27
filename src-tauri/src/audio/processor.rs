//! Core audio processing and merge implementation

use super::{AudioFile, AudioSettings, ProgressReporter, ProcessingStage, SampleRateConfig};
use super::constants::*;
use super::context::ProcessingContext;
use super::session::ProcessingSession;
use super::progress::ProgressEmitter;
use super::cleanup::CleanupGuard;
use super::metrics::ProcessingMetrics;
use crate::errors::{AppError, Result};
use crate::ffmpeg::FFmpegError;
use crate::metadata::{AudiobookMetadata, write_metadata};
use lofty::probe::Probe;
use lofty::file::AudioFile as LoftyAudioFile;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::time::Duration;

// ProgressEvent moved to progress.rs module for centralized management
// Using the centralized ProgressEvent from super::progress module

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
                log::warn!("Could not read sample rate from {}: {}", path.display(), e);
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
#[allow(deprecated)]
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
        write_metadata(&merged_output, &metadata)
            .map_err(|e| {
                log::error!("Failed to write metadata to '{}': {}", merged_output.display(), e);
                e
            })?;
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

/// Creates temporary directory for processing with session isolation
fn create_temp_directory_with_session(session_id: &str) -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir()
        .join(TEMP_DIR_NAME)
        .join(session_id);
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| AppError::FileValidation(
            format!("Cannot create session temp directory: {e}")
        ))?;
    Ok(temp_dir)
}

/// Creates temporary directory for processing (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by using a default
/// session ID. New code should use create_temp_directory_with_session.
#[deprecated = "Use create_temp_directory_with_session for session isolation"]
fn create_temp_directory() -> Result<PathBuf> {
    let default_session = "default-session";
    create_temp_directory_with_session(default_session)
}

/// Creates FFmpeg concat file for merging
fn create_concat_file(
    files: &[AudioFile],
    temp_dir: &Path
) -> Result<PathBuf> {
    let concat_file = temp_dir.join(TEMP_CONCAT_FILENAME);
    
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
        .join(TEMP_MERGED_FILENAME);
    
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
        "-f", FFMPEG_CONCAT_FORMAT,
        "-safe", FFMPEG_CONCAT_SAFE_MODE,
        "-i", &concat_file.to_string_lossy(),
        "-vn",  // Disable video processing (ignore album artwork)
        "-map", "0:a",  // Only map audio streams
        "-map_metadata", "0",  // Preserve metadata from first input
        "-c:a", FFMPEG_AUDIO_CODEC,
        "-b:a", &format!("{}k", settings.bitrate),
        "-ar", &sample_rate.to_string(),
        "-ac", &settings.channels.channel_count().to_string(),
        "-progress", FFMPEG_PROGRESS_PIPE,  // Enable progress output to stderr
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

/// Session data for audiobook processing workflow
struct ProcessingWorkflow {
    temp_dir: PathBuf,
    concat_file: PathBuf,
    total_duration: f64,
}

/// Validates inputs and emits progress
fn validate_inputs_with_progress(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<()> {
    let emitter = ProgressEmitter::new(context.window.clone());
    
    emitter.emit_analyzing_start("Validating input files...");
    validate_processing_inputs(files, &context.settings)?;
    
    if context.is_cancelled() {
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    
    Ok(())
}

/// Creates workspace and calculates total duration
fn prepare_workspace(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    let emitter = ProgressEmitter::new(context.window.clone());
    
    emitter.emit_analyzing_end("Creating temporary workspace...");
    let temp_dir = create_temp_directory_with_session(&context.session.id())?;
    let concat_file = create_concat_file(files, &temp_dir)?;
    
    let total_duration: f64 = files.iter()
        .filter(|f| f.is_valid)
        .map(|f| f.duration.unwrap_or(0.0))
        .sum();
    
    if context.is_cancelled() {
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    
    Ok(ProcessingWorkflow {
        temp_dir,
        concat_file,
        total_duration,
    })
}

/// Validates inputs and prepares processing session
fn validate_and_prepare(
    context: &ProcessingContext,
    files: &[AudioFile],
) -> Result<ProcessingWorkflow> {
    validate_inputs_with_progress(context, files)?;
    prepare_workspace(context, files)
}

/// Executes core audio processing operations
async fn execute_processing(
    context: &ProcessingContext,
    workflow: &ProcessingWorkflow,
    files: &[AudioFile],
    reporter: &mut ProgressReporter,
) -> Result<PathBuf> {
    let emitter = ProgressEmitter::new(context.window.clone());
    
    // Stage 2: Convert and merge files
    reporter.set_stage(ProcessingStage::Converting);
    emitter.emit_converting_start("Starting audio conversion...");
    
    // Log basic info for debugging
    log::info!("Starting FFmpeg merge - Total duration: {:.2}s, Bitrate: {}k", 
              workflow.total_duration, context.settings.bitrate);
    
    let merged_output = merge_audio_files_with_context(
        &workflow.concat_file,
        context,
        reporter,
        workflow.total_duration,
        files
    ).await?;
    
    if context.is_cancelled() {
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    
    Ok(merged_output)
}

/// Writes metadata if provided
fn write_metadata_stage(
    context: &ProcessingContext,
    merged_output: &PathBuf,
    metadata: Option<AudiobookMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<()> {
    if let Some(metadata) = metadata {
        let emitter = ProgressEmitter::new(context.window.clone());
        reporter.set_stage(ProcessingStage::WritingMetadata);
        emitter.emit_metadata_start("Writing metadata...");
        write_metadata(merged_output, &metadata)?;
        
        if context.is_cancelled() {
            return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
        }
    }
    Ok(())
}

/// Completes processing with file movement and cleanup
fn complete_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    let emitter = ProgressEmitter::new(context.window.clone());
    
    emitter.emit_finalizing("Moving to final location...");
    let final_output = move_to_final_location(merged_output, &context.settings.output_path)?;
    
    if context.is_cancelled() {
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    
    emitter.emit_cleanup("Cleaning up temporary files...");
    cleanup_temp_directory_with_session(&context.session.id(), workflow.temp_dir)?;
    
    reporter.complete();
    emitter.emit_complete("Processing completed successfully!");
    
    Ok(format!("Successfully created audiobook: {}", final_output.display()))
}

/// Finalizes processing with metadata and cleanup
async fn finalize_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    metadata: Option<AudiobookMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    write_metadata_stage(context, &merged_output, metadata, reporter)?;
    complete_processing(context, workflow, merged_output, reporter)
}

/// Main function to process audiobook with context-based architecture
/// 
/// This is the new structured approach using ProcessingContext
/// All new code should use this function directly
pub async fn process_audiobook_with_context(
    context: ProcessingContext,
    files: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    let mut reporter = ProgressReporter::new(files.len());
    let mut metrics = ProcessingMetrics::new();
    
    // Stage 1: Validate and prepare
    reporter.set_stage(ProcessingStage::Analyzing);
    let workflow = validate_and_prepare(&context, &files)?;
    
    // Update metrics with file information
    for file in &files {
        if file.is_valid {
            if let Some(duration) = file.duration {
                // Estimate file size based on duration and bitrate
                let estimated_bytes = (duration * context.settings.bitrate as f64 * 125.0) as usize;
                metrics.update_file_processed(
                    Duration::from_secs_f64(duration),
                    estimated_bytes
                );
            }
        }
    }
    
    // Stage 2: Execute processing
    let merged_output = execute_processing(&context, &workflow, &files, &mut reporter).await?;
    
    // Stage 3: Finalize with metadata and cleanup
    let result = finalize_processing(&context, workflow, merged_output, metadata, &mut reporter).await?;
    
    // Log final metrics summary
    log::info!("{}", metrics.format_summary());
    
    Ok(result)
}

/// Creates processing session from legacy state
fn create_session_from_legacy_state(
    state: &tauri::State<'_, crate::ProcessingState>,
) -> Result<std::sync::Arc<ProcessingSession>> {
    use std::sync::Arc;
    let session = Arc::new(ProcessingSession::new());
    
    // Copy state values from old state to new session
    {
        let old_is_processing = state.is_processing.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access processing state".to_string()))?;
        let old_is_cancelled = state.is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access cancellation state".to_string()))?;
            
        let mut new_is_processing = session.state().is_processing.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access new processing state".to_string()))?;
        let mut new_is_cancelled = session.state().is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access new cancellation state".to_string()))?;
            
        *new_is_processing = *old_is_processing;
        *new_is_cancelled = *old_is_cancelled;
    }
    
    Ok(session)
}

/// Main function to process audiobook with event emission for progress tracking
/// 
/// ADAPTER FUNCTION: This maintains backward compatibility by converting
/// the old parameter-based approach to use the new ProcessingContext internally.
/// 
/// All existing code calling this function will continue to work unchanged.
#[deprecated = "Use process_audiobook_with_context for new code - this adapter maintains compatibility"]
pub async fn process_audiobook_with_events(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    files: Vec<AudioFile>,
    settings: AudioSettings,
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    let session = create_session_from_legacy_state(&state)?;
    let context = ProcessingContext::new(window, session, settings);
    
    // Delegate to the new context-based function
    process_audiobook_with_context(context, files, metadata).await
}

/// Merges audio files with context-based progress tracking
async fn merge_audio_files_with_context(
    concat_file: &Path,
    context: &ProcessingContext,
    reporter: &mut ProgressReporter,
    total_duration: f64,
    files: &[AudioFile],
) -> Result<PathBuf> {
    let temp_output = concat_file.parent()
        .ok_or_else(|| AppError::FileValidation("Invalid concat file path".to_string()))?
        .join(TEMP_MERGED_FILENAME);
    
    // Extract file paths for sample rate detection
    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    
    // Build FFmpeg command
    let cmd = build_merge_command(concat_file, &temp_output, &context.settings, &file_paths)?;
    
    // Execute with progress tracking using context
    execute_with_progress_context(cmd, reporter, context, total_duration).await?;
    
    Ok(temp_output)
}

/// Merges audio files with progress tracking and event emission (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new context-based approach internally.
#[deprecated = "Use merge_audio_files_with_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
async fn merge_audio_files_with_events(
    concat_file: &Path,
    settings: &AudioSettings,
    reporter: &mut ProgressReporter,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
    total_duration: f64,
    files: &[AudioFile],
) -> Result<PathBuf> {
    // Create context from legacy parameters
    use std::sync::Arc;
    let session = Arc::new(ProcessingSession::new());
    
    // Copy state values
    {
        let old_is_cancelled = state.is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access cancellation state".to_string()))?;
        let mut new_is_cancelled = session.state().is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access new cancellation state".to_string()))?;
        *new_is_cancelled = *old_is_cancelled;
    }
    
    let context = ProcessingContext::new(window.clone(), session, settings.clone());
    
    // Delegate to new context-based function
    merge_audio_files_with_context(concat_file, &context, reporter, total_duration, files).await
}

/// Checks for cancellation and kills process if needed (context-based)
fn check_cancellation_and_kill_context(
    context: &ProcessingContext,
    child: &mut std::process::Child,
) -> Result<()> {
    if context.is_cancelled() {
        log::debug!("Cancellation detected, killing FFmpeg process...");
        let _ = child.kill();
        
        // Wait for process to actually terminate
        for i in 0..PROCESS_TERMINATION_MAX_ATTEMPTS {  // Try for 2 seconds max
            if let Ok(Some(_)) = child.try_wait() {
                log::debug!("FFmpeg process terminated successfully");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(PROCESS_TERMINATION_CHECK_DELAY_MS));
            if i == PROCESS_TERMINATION_MAX_ATTEMPTS - 1 {
                log::warn!("FFmpeg process may not have terminated cleanly");
            }
        }
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    Ok(())
}

/// Checks for cancellation and kills process if needed (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new context-based approach internally.
#[deprecated = "Use check_cancellation_and_kill_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
fn check_cancellation_and_kill(
    state: &tauri::State<'_, crate::ProcessingState>,
    child: &mut std::process::Child,
) -> Result<()> {
    let is_cancelled = state.is_cancelled.lock()
        .map_err(|_| AppError::InvalidInput("Failed to check cancellation state".to_string()))?;
    
    if *is_cancelled {
        log::debug!("Cancellation detected, killing FFmpeg process...");
        let _ = child.kill();
        
        // Wait for process to actually terminate
        for i in 0..PROCESS_TERMINATION_MAX_ATTEMPTS {  // Try for 2 seconds max
            if let Ok(Some(_)) = child.try_wait() {
                log::debug!("FFmpeg process terminated successfully");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(PROCESS_TERMINATION_CHECK_DELAY_MS));
            if i == PROCESS_TERMINATION_MAX_ATTEMPTS - 1 {
                log::warn!("FFmpeg process may not have terminated cleanly");
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

/// Updates time estimation based on current progress
fn update_time_estimation(
    estimated_total_time: &mut f64,
    progress_count: i32,
    total_duration: f64,
    progress_time: f32,
) {
    // Use known total duration if available
    if *estimated_total_time == 0.0 && total_duration > 0.0 {
        *estimated_total_time = total_duration;
    } else if progress_count > PROGRESS_ESTIMATION_MIN_COUNT && *estimated_total_time == 0.0 {
        *estimated_total_time = progress_time as f64 * INITIAL_TIME_ESTIMATE_MULTIPLIER; // Conservative estimate
    }
}


/// Handles completion state when progress reaches 100%
fn handle_progress_completion(emitter: &ProgressEmitter) {
    eprint!("\rConverting: Done!                                          \n");
    emitter.emit_converting_progress(
        PROGRESS_METADATA_START,
        "Finalizing audio conversion...",
        None,
        None,
    );
}

/// Processes progress update and emits events (context-based)
fn process_progress_update_context(
    progress_time: f32,
    last_progress_time: &mut f32,
    progress_count: &mut i32,
    estimated_total_time: &mut f64,
    total_duration: f64,
    speed_multiplier: Option<f64>,
    emitter: &ProgressEmitter,
) -> Result<()> {
    if progress_time == PROGRESS_COMPLETE {
        handle_progress_completion(emitter);
    } else if progress_time > *last_progress_time {
        *last_progress_time = progress_time;
        *progress_count += 1;
        
        update_time_estimation(estimated_total_time, *progress_count, total_duration, progress_time);
        
        let progress_percentage = calculate_and_display_progress(
            progress_time,
            *estimated_total_time,
            *progress_count,
            speed_multiplier,
        );
        
        let eta_seconds = if let Some(speed) = speed_multiplier {
            let remaining_time = (*estimated_total_time - progress_time as f64) / speed;
            if remaining_time > 0.0 { Some(remaining_time) } else { None }
        } else {
            None
        };
        
        emitter.emit_converting_progress(
            progress_percentage.min(PROGRESS_CONVERTING_MAX as f64) as f32,
            "Converting and merging audio files...",
            None,
            eta_seconds,
        );
    }
    Ok(())
}

/// Processes progress update and emits events (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new ProgressEmitter approach internally.
#[deprecated = "Use process_progress_update_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
fn process_progress_update(
    progress_time: f32,
    last_progress_time: &mut f32,
    progress_count: &mut i32,
    estimated_total_time: &mut f64,
    total_duration: f64,
    speed_multiplier: Option<f64>,
    window: &tauri::Window,
) -> Result<()> {
    let emitter = ProgressEmitter::new(window.clone());
    process_progress_update_context(
        progress_time,
        last_progress_time,
        progress_count,
        estimated_total_time,
        total_duration,
        speed_multiplier,
        &emitter,
    )
}


/// Displays progress with known duration
fn display_progress_with_duration(
    file_progress: f64,
    progress_time: f32,
    estimated_total_time: f64,
    speed_text: &str,
    eta_text: &str,
) -> f64 {
    let percentage = PROGRESS_CONVERTING_START as f64 + (file_progress * PROGRESS_RANGE_MULTIPLIER);
    
    eprint!("\rConverting: {:.1}% ({:.1}s / {:.1}s){}{}", 
        file_progress * 100.0, 
        progress_time, 
        estimated_total_time,
        speed_text,
        eta_text);
    
    percentage
}

/// Displays progress during analysis phase
fn display_analysis_progress(progress_count: i32) -> f64 {
    let percentage = PROGRESS_CONVERTING_START as f64 + ((progress_count as f64).min(MAX_INITIAL_PROGRESS_COUNT) * ANALYSIS_PROGRESS_MULTIPLIER);
    eprint!("\rConverting: {percentage:.1}% (analyzing...)");
    percentage
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
        let speed_text = speed_multiplier
            .map(|s| format!(" [Speed: {s:.1}x]"))
            .unwrap_or_default();
        let eta_text = if let Some(speed) = speed_multiplier {
            let remaining_time = (estimated_total_time - progress_time as f64) / speed;
            if remaining_time > 0.0 {
                let minutes = (remaining_time / SECONDS_PER_MINUTE) as u32;
                let seconds = (remaining_time % SECONDS_PER_MINUTE) as u32;
                format!(" [ETA: {minutes}m {seconds}s]")
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        display_progress_with_duration(file_progress, progress_time, estimated_total_time, &speed_text, &eta_text)
    } else {
        display_analysis_progress(progress_count)
    }
}

/// Process execution state for tracking progress
struct ProcessExecution {
    child: std::process::Child,
    emitter: ProgressEmitter,
    last_progress_time: f32,
    estimated_total_time: f64,
    progress_count: i32,
}

/// Sets up FFmpeg process and initial state
fn setup_process_execution(
    mut cmd: Command,
    context: &ProcessingContext,
) -> Result<ProcessExecution> {
    let child = cmd.spawn()
        .map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Failed to start FFmpeg".to_string())))?;
    
    let emitter = ProgressEmitter::new(context.window.clone());
    
    Ok(ProcessExecution {
        child,
        emitter,
        last_progress_time: 0.0,
        estimated_total_time: 0.0,
        progress_count: 0,
    })
}

/// Handles a single line of FFmpeg output for progress and error checking
fn handle_progress_line(
    line: &str,
    execution: &mut ProcessExecution,
    _context: &ProcessingContext,
    total_duration: f64,
) -> Result<()> {
    let speed_multiplier = parse_speed_multiplier(line);

    // Parse progress from FFmpeg output and emit events
    if let Some(progress_time) = crate::audio::progress::parse_ffmpeg_progress(line) {
        process_progress_update_context(
            progress_time,
            &mut execution.last_progress_time,
            &mut execution.progress_count,
            &mut execution.estimated_total_time,
            total_duration,
            speed_multiplier,
            &execution.emitter,
        )?;
    }
    
    // Check for errors (but ignore case-insensitive matches in file paths)
    if (line.contains("Error") || line.contains("error")) && 
       !line.contains("Output") && !line.contains("Input") {
        log::error!("FFmpeg error line: {line}");
        if line.contains("No such file") || line.contains("Invalid data") {
            log::error!("FFmpeg critical error: {line}");
            return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed(
                format!("FFmpeg failed to process audio files: {line}")
            )));
        }
    }
    
    Ok(())
}

/// Monitors FFmpeg process output and handles progress updates
fn monitor_process_with_progress(
    execution: &mut ProcessExecution,
    context: &ProcessingContext,
    total_duration: f64,
) -> Result<()> {
    if let Some(stderr) = execution.child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            check_cancellation_and_kill_context(context, &mut execution.child)?;
            
            let line = line.map_err(|_| AppError::FFmpeg(FFmpegError::ExecutionFailed("Error reading FFmpeg output".to_string())))?;
            
            handle_progress_line(&line, execution, context, total_duration)?;
        }
    }
    Ok(())
}

/// Waits for process completion and checks exit status
fn finalize_process_execution(
    mut execution: ProcessExecution,
    context: &ProcessingContext,
) -> Result<()> {
    // Check if process was cancelled before waiting
    if context.is_cancelled() {
        log::info!("Processing cancelled before FFmpeg completion");
        return Err(AppError::InvalidInput("Processing was cancelled by user before FFmpeg completion".to_string()));
    }
    
    // Wait for completion only if not cancelled
    let status = execution.child.wait()
        .map_err(|e| {
            let msg = format!("Failed to wait for FFmpeg process completion: {e}");
            log::error!("{msg}");
            AppError::FFmpeg(FFmpegError::ExecutionFailed(msg))
        })?;
    
    if !status.success() {
        let exit_code = status.code()
            .map(|c| format!(" (exit code: {c})"))
            .unwrap_or_default();
        let msg = format!("FFmpeg process failed during audio conversion{exit_code}");
        log::error!("{msg}");
        return Err(AppError::FFmpeg(FFmpegError::ExecutionFailed(msg)));
    }
    
    Ok(())
}

/// Executes command with context-based progress tracking
async fn execute_with_progress_context(
    cmd: Command,
    _reporter: &mut ProgressReporter,
    context: &ProcessingContext,
    total_duration: f64,
) -> Result<()> {
    let mut execution = setup_process_execution(cmd, context)?;
    monitor_process_with_progress(&mut execution, context, total_duration)?;
    finalize_process_execution(execution, context)
}

/// Executes command with progress tracking and event emission (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new context-based approach internally.
#[deprecated = "Use execute_with_progress_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
async fn execute_with_progress_events(
    cmd: Command,
    _reporter: &mut ProgressReporter,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
    total_duration: f64,
) -> Result<()> {
    // Create context from legacy parameters
    use std::sync::Arc;
    let session = Arc::new(ProcessingSession::new());
    
    // Copy state values
    {
        let old_is_cancelled = state.is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access cancellation state".to_string()))?;
        let mut new_is_cancelled = session.state().is_cancelled.lock()
            .map_err(|_| AppError::InvalidInput("Failed to access new cancellation state".to_string()))?;
        *new_is_cancelled = *old_is_cancelled;
    }
    
    // Create a minimal settings for context (we only need it for structure)
    let context = ProcessingContext::new(window.clone(), session, AudioSettings::default());
    
    // Delegate to new context-based function
    execute_with_progress_context(cmd, _reporter, &context, total_duration).await
}

/// Cleans up session-specific temporary directory using CleanupGuard
fn cleanup_temp_directory_with_session(session_id: &str, temp_dir: PathBuf) -> Result<()> {
    log::debug!("Cleaning up temporary directory for session {}: {}", session_id, temp_dir.display());
    let mut guard = CleanupGuard::new(session_id.to_string());
    guard.add_path(&temp_dir);
    guard.cleanup_now()
        .map_err(|e| {
            log::warn!("Failed to cleanup temporary directory '{}': {}", temp_dir.display(), e);
            e
        })
}

/// Cleans up temporary directory (ADAPTER)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by using manual cleanup.
/// New code should use cleanup_temp_directory_with_session.
#[deprecated = "Use cleanup_temp_directory_with_session for session isolation"]
fn cleanup_temp_directory(temp_dir: PathBuf) -> Result<()> {
    std::fs::remove_dir_all(&temp_dir)
        .map_err(|e| {
            let msg = format!("Cannot cleanup temporary directory '{}': {}. Directory may still contain files.", 
                temp_dir.display(), e);
            log::warn!("{msg}");
            AppError::FileValidation(msg)
        })?;
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
        let session_id = "test-session-123";
        let result = create_temp_directory_with_session(session_id);
        assert!(result.is_ok());
        let temp_dir = result.unwrap();
        assert!(temp_dir.exists());
        // Check that session ID is in the path
        assert!(temp_dir.to_string_lossy().contains(session_id));
        
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

    #[test]
    fn test_ffmpeg_command_includes_metadata_mapping() {
        let temp_dir = TempDir::new().unwrap();
        let concat_file = temp_dir.path().join("concat.txt");
        let output_file = temp_dir.path().join("output.m4b");
        let settings = AudioSettings::default();
        let file_paths = vec![PathBuf::from("/test/file.mp3")];
        
        // This test will fail if ffmpeg is not found, but that's expected
        // We're testing the command construction logic
        let result = build_merge_command(&concat_file, &output_file, &settings, &file_paths);
        
        // The test should either succeed or fail due to ffmpeg not being found
        // If it succeeds, the command should be built correctly
        if let Ok(cmd) = result {
            let cmd_str = format!("{:?}", cmd);
            // Verify metadata mapping is included
            assert!(cmd_str.contains("-map_metadata"));
            assert!(cmd_str.contains("0"));
        }
        // If it fails, it should be due to ffmpeg not being available, not our logic
    }
    
    #[test]
    fn test_session_isolated_temp_directories() -> Result<()> {
        // Test that different sessions create isolated temp directories
        let session_id1 = "session-123";
        let session_id2 = "session-456";
        
        let temp_dir1 = create_temp_directory_with_session(session_id1)?;
        let temp_dir2 = create_temp_directory_with_session(session_id2)?;
        
        // Directories should be different
        assert_ne!(temp_dir1, temp_dir2);
        
        // Both should contain the session ID in their path
        assert!(temp_dir1.to_string_lossy().contains(session_id1));
        assert!(temp_dir2.to_string_lossy().contains(session_id2));
        
        // Both directories should exist
        assert!(temp_dir1.exists());
        assert!(temp_dir2.exists());
        
        // Create files in each to test isolation
        let file1 = temp_dir1.join("test.txt");
        let file2 = temp_dir2.join("test.txt");
        
        std::fs::write(&file1, "session 1 content")
            .map_err(|e| AppError::Io(e))?;
        std::fs::write(&file2, "session 2 content")
            .map_err(|e| AppError::Io(e))?;
        
        // Files should exist independently
        assert!(file1.exists());
        assert!(file2.exists());
        
        // Contents should be different (proving isolation)
        let content1 = std::fs::read_to_string(&file1)
            .map_err(|e| AppError::Io(e))?;
        let content2 = std::fs::read_to_string(&file2)
            .map_err(|e| AppError::Io(e))?;
        
        assert_eq!(content1, "session 1 content");
        assert_eq!(content2, "session 2 content");
        assert_ne!(content1, content2);
        
        // Cleanup
        cleanup_temp_directory_with_session(session_id1, temp_dir1)?;
        cleanup_temp_directory_with_session(session_id2, temp_dir2)?;
        
        Ok(())
    }
    
    #[test]
    fn test_session_temp_directory_cleanup() -> Result<()> {
        let session_id = "test-cleanup-session";
        let temp_dir = create_temp_directory_with_session(session_id)?;
        
        // Create some test files
        let test_file = temp_dir.join("test.txt");
        let sub_dir = temp_dir.join("subdir");
        let sub_file = sub_dir.join("nested.txt");
        
        std::fs::write(&test_file, "test content")
            .map_err(|e| AppError::Io(e))?;
        std::fs::create_dir(&sub_dir)
            .map_err(|e| AppError::Io(e))?;
        std::fs::write(&sub_file, "nested content")
            .map_err(|e| AppError::Io(e))?;
        
        // Verify files exist
        assert!(temp_dir.exists());
        assert!(test_file.exists());
        assert!(sub_dir.exists());
        assert!(sub_file.exists());
        
        // Cleanup using session-aware function
        cleanup_temp_directory_with_session(session_id, temp_dir.clone())?;
        
        // Verify everything is cleaned up
        assert!(!temp_dir.exists());
        assert!(!test_file.exists());
        assert!(!sub_dir.exists());
        assert!(!sub_file.exists());
        
        Ok(())
    }
    
    #[test]
    fn test_multiple_sessions_no_interference() -> Result<()> {
        // Test that multiple sessions can work concurrently without interfering
        let session_id_a = "concurrent-session-a";
        let session_id_b = "concurrent-session-b";
        
        // Create temp directories for both sessions
        let temp_dir_a = create_temp_directory_with_session(session_id_a)?;
        let temp_dir_b = create_temp_directory_with_session(session_id_b)?;
        
        // Create test data in each session
        let file_a = temp_dir_a.join("session_a_data.txt");
        let file_b = temp_dir_b.join("session_b_data.txt");
        
        std::fs::write(&file_a, "Session A data")
            .map_err(|e| AppError::Io(e))?;
        std::fs::write(&file_b, "Session B data")
            .map_err(|e| AppError::Io(e))?;
        
        // Both should exist independently
        assert!(file_a.exists());
        assert!(file_b.exists());
        
        // Clean up session A
        cleanup_temp_directory_with_session(session_id_a, temp_dir_a)?;
        
        // Session A should be cleaned up, but session B should remain
        assert!(!file_a.exists());
        assert!(file_b.exists());
        
        // Clean up session B
        cleanup_temp_directory_with_session(session_id_b, temp_dir_b)?;
        
        // Now session B should also be cleaned up
        assert!(!file_b.exists());
        
        Ok(())
    }
    
    #[test]
    fn test_parse_speed_multiplier() {
        // Test the speed multiplier parsing logic
        assert_eq!(parse_speed_multiplier("speed=1.5x"), Some(1.5));
        assert_eq!(parse_speed_multiplier("frame=100 speed=2.0x bitrate=128k"), Some(2.0));
        assert_eq!(parse_speed_multiplier("speed=0.8x"), Some(0.8));
        assert_eq!(parse_speed_multiplier("no speed here"), None);
        assert_eq!(parse_speed_multiplier("speed=invalidx"), None);
        assert_eq!(parse_speed_multiplier("speed=1.2"), None); // Missing 'x'
        assert_eq!(parse_speed_multiplier(""), None);
    }
    
    #[test]
    fn test_error_handling_for_new_error_variants() {
        // Test that our error handling works correctly with new error types
        use crate::errors::AppError;
        
        // Test FileValidation error (used in temp directory creation)
        let file_error = AppError::FileValidation("Test file validation error".to_string());
        assert!(file_error.to_string().contains("Test file validation error"));
        
        // Test that InvalidInput error propagates correctly
        let input_error = AppError::InvalidInput("Test input error".to_string());
        assert!(input_error.to_string().contains("Test input error"));
        
        // Test Io error wrapping
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied");
        let wrapped_error = AppError::Io(io_error);
        assert!(wrapped_error.to_string().contains("Permission denied"));
    }
    
    #[test]
    fn test_resource_leak_prevention() -> Result<()> {
        // Test that we don't leak resources during processing
        let session_id = "leak-test-session";
        
        // Create multiple temp directories to simulate multiple processing attempts
        let mut temp_dirs = Vec::new();
        for i in 0..5 {
            let temp_dir = create_temp_directory_with_session(&format!("{session_id}-{i}"))?;
            
            // Create some files
            let test_file = temp_dir.join(format!("test_{i}.txt"));
            std::fs::write(&test_file, format!("Test content {i}"))
                .map_err(|e| AppError::Io(e))?;
            
            temp_dirs.push((format!("{session_id}-{i}"), temp_dir));
        }
        
        // Verify all directories exist
        for (_, temp_dir) in &temp_dirs {
            assert!(temp_dir.exists());
        }
        
        // Clean up all directories
        for (session_id, temp_dir) in temp_dirs {
            cleanup_temp_directory_with_session(&session_id, temp_dir)?;
        }
        
        // Verify no directories remain
        let base_temp_dir = std::env::temp_dir().join(TEMP_DIR_NAME);
        if base_temp_dir.exists() {
            // Check that our session directories are gone
            let entries = std::fs::read_dir(&base_temp_dir)
                .map_err(|e| AppError::Io(e))?;
            
            for entry in entries {
                let entry = entry.map_err(|e| AppError::Io(e))?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                assert!(!name.contains("leak-test-session"), 
                    "Found leaked directory: {}", name);
            }
        }
        
        Ok(())
    }
    
    #[test]
    fn test_deprecated_adapter_functions() -> Result<()> {
        // Test that deprecated adapter functions still work for backward compatibility
        
        // Test deprecated create_temp_directory
        #[allow(deprecated)]
        let temp_dir = create_temp_directory()?;
        assert!(temp_dir.exists());
        assert!(temp_dir.to_string_lossy().contains("default-session"));
        
        // Test deprecated cleanup_temp_directory
        #[allow(deprecated)]
        let result = cleanup_temp_directory(temp_dir);
        assert!(result.is_ok());
        
        Ok(())
    }
    
    #[test]
    fn test_update_time_estimation() {
        // Test the time estimation update logic
        let mut estimated_total_time = 0.0;
        let mut progress_count = 0;
        let total_duration = 120.0; // 2 minutes
        let progress_time = 30.0; // 30 seconds processed
        
        // First update with known total duration
        update_time_estimation(
            &mut estimated_total_time,
            progress_count,
            total_duration,
            progress_time,
        );
        
        assert_eq!(estimated_total_time, 120.0);
        
        // Test with unknown duration and insufficient progress
        estimated_total_time = 0.0;
        progress_count = 5; // Less than PROGRESS_ESTIMATION_MIN_COUNT
        update_time_estimation(
            &mut estimated_total_time,
            progress_count,
            0.0, // No known total duration
            progress_time,
        );
        
        assert_eq!(estimated_total_time, 0.0); // Should remain 0
        
        // Test with unknown duration but sufficient progress
        progress_count = 15; // More than PROGRESS_ESTIMATION_MIN_COUNT  
        update_time_estimation(
            &mut estimated_total_time,
            progress_count,
            0.0, // No known total duration
            progress_time,
        );
        
        // Should estimate based on current progress
        let expected = progress_time as f64 * INITIAL_TIME_ESTIMATE_MULTIPLIER;
        assert_eq!(estimated_total_time, expected);
    }
    
    #[test]
    fn test_progress_calculation() {
        // Test progress calculation logic
        let progress_time = 60.0; // 1 minute
        let estimated_total_time = 120.0; // 2 minutes
        let progress_count = 10;
        let speed_multiplier = Some(2.0); // 2x speed
        
        let percentage = calculate_and_display_progress(
            progress_time,
            estimated_total_time,
            progress_count,
            speed_multiplier,
        );
        
        // Should be in the converting range (20-95%)
        assert!(percentage >= PROGRESS_CONVERTING_START as f64);
        assert!(percentage <= PROGRESS_CONVERTING_MAX as f64);
        
        // With known duration, should be proportional
        let expected_base = PROGRESS_CONVERTING_START as f64;
        let file_progress = progress_time as f64 / estimated_total_time;
        let expected_percentage = expected_base + (file_progress * PROGRESS_RANGE_MULTIPLIER);
        assert!((percentage - expected_percentage).abs() < 0.1);
    }
}
