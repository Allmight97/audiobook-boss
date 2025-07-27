//! Centralized progress event emission for audio processing
//! 
//! This module provides a unified interface for emitting progress events
//! throughout the audio processing pipeline, eliminating duplicate code
//! and ensuring consistent progress reporting to the frontend.

use super::{ProcessingProgress, ProcessingStage};
use super::constants::*;
use serde::Serialize;
use std::time::Instant;
use tauri::{Emitter, Window};

/// Progress event structure for frontend communication
/// Extracted from processor.rs to centralize progress event handling
#[derive(Clone, Serialize)]
pub struct ProgressEvent {
    /// Current processing stage name
    pub stage: String,
    /// Progress percentage (0-100)
    pub percentage: f32,
    /// Human-readable status message
    pub message: String,
    /// Currently processing file (if applicable)
    pub current_file: Option<String>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
}

/// Centralized progress event emitter
/// Eliminates duplicate progress emission code throughout the codebase
#[allow(dead_code)] // New infrastructure - will be used when processor.rs is refactored
pub struct ProgressEmitter {
    /// Reference to the Tauri window for event emission
    window: Window,
}

#[allow(dead_code)] // New infrastructure - methods will be used when processor.rs is refactored
impl ProgressEmitter {
    /// Creates a new progress emitter
    pub fn new(window: Window) -> Self {
        Self { window }
    }

    /// Emits a progress event for analyzing stage start
    pub fn emit_analyzing_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Analyzing,
            PROGRESS_ANALYZING_START,
            message,
            None,
            None,
        );
    }

    /// Emits a progress event for analyzing stage end
    pub fn emit_analyzing_end(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Analyzing,
            PROGRESS_ANALYZING_END,
            message,
            None,
            None,
        );
    }

    /// Emits a progress event for converting stage start
    pub fn emit_converting_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Converting,
            PROGRESS_CONVERTING_START,
            message,
            None,
            None,
        );
    }

    /// Emits a progress event during conversion with file info
    pub fn emit_converting_progress(
        &self,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        let clamped_percentage = percentage.min(PROGRESS_CONVERTING_MAX);
        self.emit_event(
            ProcessingStage::Converting,
            clamped_percentage,
            message,
            current_file,
            eta_seconds,
        );
    }

    /// Emits a progress event for metadata writing start
    pub fn emit_metadata_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_METADATA_START,
            message,
            None,
            None,
        );
    }

    /// Emits a progress event for finalizing stage
    pub fn emit_finalizing(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_FINALIZING,
            message,
            None,
            None,
        );
    }

    /// Emits a progress event for cleanup stage
    pub fn emit_cleanup(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Completed,
            PROGRESS_CLEANUP,
            message,
            None,
            None,
        );
    }

    /// Emits a progress event for completion
    pub fn emit_complete(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Completed,
            PROGRESS_COMPLETE,
            message,
            None,
            None,
        );
    }

    /// Emits a custom progress event with all parameters
    pub fn emit_custom(
        &self,
        stage: ProcessingStage,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        self.emit_event(stage, percentage, message, current_file, eta_seconds);
    }

    /// Internal method to emit progress events
    fn emit_event(
        &self,
        stage: ProcessingStage,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        let stage_str = match stage {
            ProcessingStage::Analyzing => "analyzing",
            ProcessingStage::Converting => "converting",
            ProcessingStage::Merging => "merging",
            ProcessingStage::WritingMetadata => "writing_metadata",
            ProcessingStage::Completed => "completed",
            ProcessingStage::Failed(_) => "failed",
        };

        let event = ProgressEvent {
            stage: stage_str.to_string(),
            percentage,
            message: message.to_string(),
            current_file,
            eta_seconds,
        };

        let _ = self.window.emit("processing-progress", &event);
    }

    /// Calculates progress percentage within a stage range
    pub fn calculate_stage_progress(
        current: f64,
        total: f64,
        start_percentage: f32,
        end_percentage: f32,
    ) -> f32 {
        if total <= 0.0 {
            return start_percentage;
        }

        let progress_ratio = (current / total) as f32;
        let range = end_percentage - start_percentage;
        start_percentage + (progress_ratio * range)
    }

    /// Formats estimated time remaining into a human-readable string
    pub fn format_eta(seconds: f64) -> String {
        if seconds < SECONDS_PER_MINUTE {
            format!("{seconds:.0}s")
        } else {
            let minutes = (seconds / SECONDS_PER_MINUTE) as u32;
            let remaining_seconds = seconds % SECONDS_PER_MINUTE;
            format!("{minutes}m {remaining_seconds:.0}s")
        }
    }
}

/// Progress reporter for tracking audio processing operations
/// Maintained for compatibility with existing code
pub struct ProgressReporter {
    /// Total number of files to process
    total_files: usize,
    /// Files completed so far
    files_completed: usize,
    /// Current processing stage
    current_stage: ProcessingStage,
    /// Start time of processing
    #[allow(dead_code)]
    start_time: Instant,
    /// Current file being processed
    current_file: Option<String>,
}

impl ProgressReporter {
    /// Creates a new progress reporter
    pub fn new(total_files: usize) -> Self {
        Self {
            total_files,
            files_completed: 0,
            current_stage: ProcessingStage::Analyzing,
            start_time: Instant::now(),
            current_file: None,
        }
    }
    
    /// Updates the current processing stage
    pub fn set_stage(&mut self, stage: ProcessingStage) {
        self.current_stage = stage;
    }
    
    /// Sets the current file being processed
    #[allow(dead_code)]
    pub fn set_current_file<S: Into<String>>(&mut self, filename: S) {
        self.current_file = Some(filename.into());
    }
    
    /// Increments the completed file count
    #[allow(dead_code)]
    pub fn complete_file(&mut self) {
        self.files_completed += 1;
        self.current_file = None;
    }
    
    /// Calculates current progress as a percentage
    #[allow(dead_code)]
    pub fn calculate_progress(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        
        // Base progress on stage and files completed
        let _stage_weight = match self.current_stage {
            ProcessingStage::Analyzing => 0.1,
            ProcessingStage::Converting => 0.7,
            ProcessingStage::Merging => 0.15,
            ProcessingStage::WritingMetadata => 0.05,
            ProcessingStage::Completed => 1.0,
            ProcessingStage::Failed(_) => 0.0,
        };
        
        let file_progress = self.files_completed as f32 / self.total_files as f32;
        
        match self.current_stage {
            ProcessingStage::Analyzing => PROGRESS_ANALYZING_END * file_progress,
            ProcessingStage::Converting => PROGRESS_CONVERTING_START + (PROGRESS_CONVERTING_RANGE * file_progress),
            ProcessingStage::Merging => PROGRESS_MERGING_START + (PROGRESS_MERGING_WEIGHT * file_progress),
            ProcessingStage::WritingMetadata => PROGRESS_FINALIZING + (PROGRESS_METADATA_WEIGHT * file_progress),
            ProcessingStage::Completed => PROGRESS_COMPLETE,
            ProcessingStage::Failed(_) => 0.0,
        }
    }
    
    /// Estimates time remaining based on current progress
    #[allow(dead_code)]
    pub fn estimate_time_remaining(&self) -> Option<f64> {
        let progress = self.calculate_progress();
        if progress <= 0.0 || progress >= 100.0 {
            return None;
        }
        
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let total_estimated = elapsed / (progress as f64 / 100.0);
        Some(total_estimated - elapsed)
    }
    
    /// Gets current progress information
    #[allow(dead_code)]
    pub fn get_progress(&self) -> ProcessingProgress {
        ProcessingProgress {
            stage: self.current_stage.clone(),
            progress: self.calculate_progress(),
            current_file: self.current_file.clone(),
            files_completed: self.files_completed,
            total_files: self.total_files,
            eta_seconds: self.estimate_time_remaining(),
        }
    }
    
    /// Marks processing as completed
    pub fn complete(&mut self) {
        self.current_stage = ProcessingStage::Completed;
        self.files_completed = self.total_files;
        self.current_file = None;
    }
    
    /// Marks processing as failed
    #[allow(dead_code)]
    pub fn fail<S: Into<String>>(&mut self, error: S) {
        self.current_stage = ProcessingStage::Failed(error.into());
        self.current_file = None;
    }
}

/// Holds state for FFmpeg progress parsing
#[derive(Default)]
#[allow(dead_code)]
pub struct FFmpegProgressState {
    pub out_time_us: Option<i64>,
    pub total_size: Option<i64>,
    pub bitrate: Option<f64>,
    pub speed: Option<f64>,
}

/// Parses FFmpeg progress output to extract percentage
pub fn parse_ffmpeg_progress(line: &str) -> Option<f32> {
    // Parse FFmpeg progress output
    
    // FFmpeg with -progress outputs key=value pairs
    if line.contains("=") {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            let key = parts[0].trim();
            let value = parts[1].trim();
            
            match key {
                "out_time_us" => {
                    // Time in microseconds - we can use this for rough progress
                    if let Ok(time_us) = value.parse::<i64>() {
                        // Convert to seconds
                        let time_seconds = time_us as f64 / 1_000_000.0;
                        // Return time in seconds as a rough progress indicator
                        // The actual percentage will be calculated in the processor
                        return Some(time_seconds as f32);
                    }
                }
                "progress" => {
                    if value == "end" {
                        return Some(100.0);
                    } else if value == "continue" {
                        // Processing is continuing, not a progress value
                        return None;
                    }
                }
                _ => {}
            }
        }
    }
    
    // Fallback: parse old-style time= format
    if line.starts_with("time=") {
        if let Some(time_str) = line.strip_prefix("time=") {
            if let Ok(duration) = parse_ffmpeg_time(time_str) {
                return Some(duration as f32);
            }
        }
    }
    
    None
}

/// Parses FFmpeg time format (HH:MM:SS.ss) to seconds
fn parse_ffmpeg_time(time_str: &str) -> Result<f64, std::num::ParseFloatError> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return Ok(0.0);
    }
    
    let hours: f64 = parts[0].parse()?;
    let minutes: f64 = parts[1].parse()?;
    let seconds: f64 = parts[2].parse()?;
    
    Ok(hours * 3600.0 + minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_emitter_calculate_stage_progress() {
        // Test progress calculation within a stage
        assert_eq!(
            ProgressEmitter::calculate_stage_progress(0.0, 100.0, 10.0, 80.0),
            10.0
        );
        assert_eq!(
            ProgressEmitter::calculate_stage_progress(50.0, 100.0, 10.0, 80.0),
            45.0
        );
        assert_eq!(
            ProgressEmitter::calculate_stage_progress(100.0, 100.0, 10.0, 80.0),
            80.0
        );
        
        // Test edge cases
        assert_eq!(
            ProgressEmitter::calculate_stage_progress(50.0, 0.0, 10.0, 80.0),
            10.0
        );
    }

    #[test]
    fn test_progress_emitter_format_eta() {
        assert_eq!(ProgressEmitter::format_eta(30.0), "30s");
        assert_eq!(ProgressEmitter::format_eta(90.0), "1m 30s");
        assert_eq!(ProgressEmitter::format_eta(150.5), "2m 30s");
        assert_eq!(ProgressEmitter::format_eta(0.0), "0s");
        assert_eq!(ProgressEmitter::format_eta(59.9), "60s");
        assert_eq!(ProgressEmitter::format_eta(60.0), "1m 0s");
        assert_eq!(ProgressEmitter::format_eta(125.0), "2m 5s");
    }

    #[test]
    fn test_progress_reporter_new() {
        let reporter = ProgressReporter::new(5);
        assert_eq!(reporter.total_files, 5);
        assert_eq!(reporter.files_completed, 0);
        assert!(matches!(reporter.current_stage, ProcessingStage::Analyzing));
    }

    #[test]
    fn test_calculate_progress() {
        let mut reporter = ProgressReporter::new(4);
        
        // Initial progress
        assert_eq!(reporter.calculate_progress(), 0.0);
        
        // Complete analyzing stage
        reporter.complete_file();
        reporter.set_stage(ProcessingStage::Converting);
        assert!(reporter.calculate_progress() > 10.0);
        
        // Complete all files
        reporter.complete();
        assert_eq!(reporter.calculate_progress(), 100.0);
    }

    #[test]
    fn test_estimate_time_remaining() {
        let reporter = ProgressReporter::new(2);
        // At 0% progress, should return None
        assert!(reporter.estimate_time_remaining().is_none());
    }

    #[test]
    fn test_parse_ffmpeg_time() {
        assert_eq!(parse_ffmpeg_time("00:01:30.50").unwrap(), 90.5);
        assert_eq!(parse_ffmpeg_time("01:00:00.00").unwrap(), 3600.0);
    }

    #[test]
    fn test_parse_ffmpeg_progress() {
        // Test old format
        assert_eq!(parse_ffmpeg_progress("time=00:01:30.45").unwrap(), 90.45);
        
        // Test new -progress format
        assert_eq!(parse_ffmpeg_progress("out_time_us=90450000").unwrap(), 90.45);
        assert_eq!(parse_ffmpeg_progress("progress=end").unwrap(), 100.0);
        assert!(parse_ffmpeg_progress("progress=continue").is_none());
        assert!(parse_ffmpeg_progress("other output").is_none());
        
        // Test various progress outputs
        assert_eq!(parse_ffmpeg_progress("out_time_us=1000000").unwrap(), 1.0);
        assert_eq!(parse_ffmpeg_progress("out_time_us=60000000").unwrap(), 60.0);
    }
}