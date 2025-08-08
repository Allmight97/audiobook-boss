//! Core audio processing and merge implementation

use super::{AudioFile, AudioSettings, ProgressReporter, ProcessingStage, CleanupGuard};
use super::constants::*;
use super::context::ProcessingContext;
use super::media_pipeline::{MediaProcessingPlan, MediaProcessor, ShellFFmpegProcessor};
use super::metrics::ProcessingMetrics;
use super::session::ProcessingSession;
use crate::errors::{AppError, Result};
use crate::metadata::{AudiobookMetadata, write_metadata};
use lofty::probe::Probe;
use lofty::file::AudioFile as LoftyAudioFile;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use crate::ffmpeg::format_concat_file_line;

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
    
    // Create a temporary context for the legacy function
    // Note: This is a simplified approach for the deprecated function
    let temp_output = concat_file.parent()
        .ok_or_else(|| AppError::FileValidation("Invalid concat file path".to_string()))?
        .join(TEMP_MERGED_FILENAME);
    
    // Extract file paths and create media processing plan
    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let total_duration = MediaProcessingPlan::calculate_total_duration(&files);
    
    let plan = MediaProcessingPlan::new(
        concat_file.clone(),
        temp_output.clone(),
        settings.clone(),
        file_paths,
        total_duration,
    );
    
    // Execute the plan using simplified approach for legacy function
    // This legacy function doesn't have access to window/session, so we execute directly
    let mut cmd = plan.build_ffmpeg_command()?;
    
    // Simple execution without progress tracking for legacy compatibility
    let output = cmd.output()
        .map_err(AppError::Io)?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::InvalidInput(format!("FFmpeg failed: {stderr}")));
    }
    
    let merged_output = temp_output;
    
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
        // Use centralized escaping and canonicalization
        content.push_str(&format_concat_file_line(&file.path));
    }
    
    std::fs::write(&concat_file, content)
        .map_err(|e| AppError::FileValidation(
            format!("Cannot write concat file: {e}")
        ))?;
    
    Ok(concat_file)
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
    let mut emitter = ProgressReporter::new(1); // Single file processing
    
    emitter.set_stage(ProcessingStage::Analyzing);
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
    let mut emitter = ProgressReporter::new(1); // Single file processing
    
    emitter.set_stage(ProcessingStage::Analyzing);
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
    let mut emitter = ProgressReporter::new(1); // Single file processing
    
    // Stage 2: Convert and merge files
    reporter.set_stage(ProcessingStage::Converting);
    emitter.set_stage(ProcessingStage::Converting);
    
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
        // Emit UI event for metadata stage
        {
            let ui = super::progress::ProgressEmitter::new(context.window.clone());
            ui.emit_metadata_start("Writing metadata...");
        }
        reporter.set_stage(ProcessingStage::WritingMetadata);
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
    // Emit UI events for cleanup and completion
    let ui = super::progress::ProgressEmitter::new(context.window.clone());
    ui.emit_cleanup("Cleaning up...");
    let final_output = move_to_final_location(merged_output, &context.settings.output_path)?;
    
    if context.is_cancelled() {
        return Err(AppError::InvalidInput("Processing was cancelled".to_string()));
    }
    
    // Cleanup stage - no specific stage for this
    cleanup_temp_directory_with_session(&context.session.id(), workflow.temp_dir)?;
    
    reporter.complete();
    ui.emit_complete("Processing complete");
    
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
pub fn create_session_from_legacy_state(
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
    _reporter: &mut ProgressReporter,
    total_duration: f64,
    files: &[AudioFile],
) -> Result<PathBuf> {
    let temp_output = concat_file.parent()
        .ok_or_else(|| AppError::FileValidation("Invalid concat file path".to_string()))?
        .join(TEMP_MERGED_FILENAME);
    
    // Extract file paths and settings from context
    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let settings = &context.settings;
    
    // Create media processing plan and execute using new pipeline
    let plan = MediaProcessingPlan::new(
        concat_file.to_path_buf(),
        temp_output.clone(),
        settings.clone(),
        file_paths,
        total_duration,
    );
    
    // Select processor implementation based on compile-time feature
    // Default behavior unchanged: always uses ShellFFmpegProcessor unless safe-ffmpeg is enabled
    #[cfg(feature = "safe-ffmpeg")]
    let processor = {
        log::info!("Using FfmpegNextProcessor (safe-ffmpeg feature enabled)");
        crate::audio::media_pipeline::FfmpegNextProcessor
    };
    
    #[cfg(not(feature = "safe-ffmpeg"))]
    let processor = {
        log::debug!("Using ShellFFmpegProcessor (default)");
        crate::audio::media_pipeline::ShellFFmpegProcessor
    };
    
    // Route execution through the trait boundary
    processor.execute(&plan, context).await?;
    
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
    
    // Use media pipeline for FFmpeg execution
    use super::media_pipeline::execute_ffmpeg_with_progress_context;
    execute_ffmpeg_with_progress_context(cmd, &context, total_duration).await
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


