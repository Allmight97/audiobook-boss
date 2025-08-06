//! Media processing pipeline for FFmpeg operations
//! 
//! This module provides a unified interface for all FFmpeg operations,
//! encapsulating command building and execution behind a stable Rust interface.
//! 
//! The `MediaProcessingPlan` struct holds inputs, outputs, and metadata for
//! processing operations, following mentor recommendations for abstraction.

use super::{AudioSettings, SampleRateConfig};
use super::constants::*;
use super::context::ProcessingContext;
use super::processor::{detect_input_sample_rate, create_session_from_legacy_state};
use super::progress_monitor::{setup_process_execution, monitor_process_with_progress, finalize_process_execution};
use crate::errors::Result;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Media processing plan that encapsulates inputs, outputs, and metadata
/// 
/// This struct follows the mentor's recommendation to use a `MediaProcessingPlan`
/// to hold all processing parameters in a structured way.
#[derive(Debug, Clone)]
pub struct MediaProcessingPlan {
    /// Input concat file path
    pub input_concat_file: PathBuf,
    /// Output file path
    pub output_path: PathBuf,
    /// Audio processing settings
    pub settings: AudioSettings,
    /// Input file paths for sample rate detection
    pub input_file_paths: Vec<PathBuf>,
    /// Total duration for progress tracking
    pub total_duration: f64,
}

impl MediaProcessingPlan {
    /// Creates a new media processing plan
    pub fn new(
        input_concat_file: PathBuf,
        output_path: PathBuf,
        settings: AudioSettings,
        input_file_paths: Vec<PathBuf>,
        total_duration: f64,
    ) -> Self {
        Self {
            input_concat_file,
            output_path,
            settings,
            input_file_paths,
            total_duration,
        }
    }

    /// Helper function to calculate total duration from AudioFile list
    /// Handles Option<f64> duration fields properly
    pub fn calculate_total_duration(files: &[super::AudioFile]) -> f64 {
        files.iter()
            .filter_map(|f| f.duration)
            .sum()
    }

    /// Builds FFmpeg command for this processing plan
    pub fn build_ffmpeg_command(&self) -> Result<Command> {
        build_merge_command(
            &self.input_concat_file,
            &self.output_path,
            &self.settings,
            &self.input_file_paths,
        )
    }

    /// Executes the processing plan with context-based progress tracking
    pub async fn execute_with_context(
        &self,
        context: &ProcessingContext,
    ) -> Result<()> {
        let cmd = self.build_ffmpeg_command()?;
        execute_ffmpeg_with_progress_context(cmd, context, self.total_duration).await
    }


}

/// Builds FFmpeg command for merging audio files
/// 
/// This function encapsulates all FFmpeg command construction logic,
/// providing a stable interface for audio processing operations.
pub fn build_merge_command(
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

/// Executes FFmpeg command with context-based progress tracking
/// 
/// This function provides a unified interface for executing FFmpeg commands
/// with proper progress monitoring and cancellation support.
pub async fn execute_ffmpeg_with_progress_context(
    cmd: Command,
    context: &ProcessingContext,
    total_duration: f64,
) -> Result<()> {
    log::debug!("Starting FFmpeg execution with progress tracking");
    
    // Set up process execution
    let mut execution = setup_process_execution(cmd, context)?;
    
    // Monitor process with progress updates
    monitor_process_with_progress(&mut execution, context, total_duration)?;
    
    // Finalize and check exit status
    finalize_process_execution(execution, context)?;
    
    log::debug!("FFmpeg execution completed successfully");
    Ok(())
}

/// ADAPTER: Executes command with progress tracking (legacy compatibility)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility by converting parameters
/// to use the new context-based approach internally.
#[deprecated = "Use execute_ffmpeg_with_progress_context for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
pub async fn execute_with_progress_events(
    cmd: Command,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
    total_duration: f64,
) -> Result<()> {
    // Convert legacy parameters to context-based approach
    let session = create_session_from_legacy_state(state)?;
    let context = ProcessingContext::new(window.clone(), session, AudioSettings::default());
    // Note: We use default settings here since they're not available in the legacy adapter
    
    execute_ffmpeg_with_progress_context(cmd, &context, total_duration).await
}

/// ADAPTER: Builds merge command (legacy compatibility)
/// 
/// ADAPTER FUNCTION: Maintains backward compatibility for existing code
/// that calls build_merge_command directly.
#[deprecated = "Use MediaProcessingPlan::build_ffmpeg_command for new code - this adapter maintains compatibility"]
#[allow(dead_code)]
pub fn build_merge_command_legacy(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
    file_paths: &[PathBuf],
) -> Result<Command> {
    build_merge_command(concat_file, output, settings, file_paths)
}
