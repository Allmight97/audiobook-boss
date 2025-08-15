//! Progress event emission and reporting for audio processing

use crate::audio::{ProcessingProgress, ProcessingStage};
use crate::audio::constants::*;
use serde::Serialize;
use std::time::Instant;
use tauri::{Emitter, Window};

/// Progress event structure for frontend communication
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

    /// Emits analyzing start event
    pub fn emit_analyzing_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Analyzing,
            PROGRESS_ANALYZING_START,
            message,
            None,
            None,
        );
    }

    /// Emits analyzing end event
    pub fn emit_analyzing_end(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Analyzing,
            PROGRESS_ANALYZING_END,
            message,
            None,
            None,
        );
    }

    /// Emits converting start event
    pub fn emit_converting_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Converting,
            PROGRESS_CONVERTING_START,
            message,
            None,
            None,
        );
    }

    /// Emits converting progress with file info
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

    /// Emits metadata writing start event
    pub fn emit_metadata_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_METADATA_START,
            message,
            None,
            None,
        );
    }

    /// Emits finalizing event
    pub fn emit_finalizing(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_FINALIZING,
            message,
            None,
            None,
        );
    }

    /// Emits cleanup event
    pub fn emit_cleanup(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Completed,
            PROGRESS_CLEANUP,
            message,
            None,
            None,
        );
    }

    /// Emits completion event
    pub fn emit_complete(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Completed,
            PROGRESS_COMPLETE,
            message,
            None,
            None,
        );
    }

    /// Emits cancelled event (special-case stage not represented in ProcessingStage enum)
    pub fn emit_cancelled(&self, message: &str) {
        let event = ProgressEvent {
            stage: "cancelled".to_string(),
            percentage: 0.0,
            message: message.to_string(),
            current_file: None,
            eta_seconds: None,
        };
        let _ = self.window.emit(PROGRESS_EVENT_NAME, &event);
    }
    /// Emits custom progress event with all parameters
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
            ProcessingStage::WritingMetadata => "writing",
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

        let _ = self.window.emit(PROGRESS_EVENT_NAME, &event);
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

/// Converts seconds to converting-stage UI percentage.
pub fn converting_percentage_from_seconds(current_seconds: f64, total_duration: f64) -> f32 {
    if total_duration <= 0.0 {
        return PROGRESS_CONVERTING_START;
    }
    let ratio = (current_seconds / total_duration).clamp(0.0, 1.0);
    let pct = PROGRESS_CONVERTING_START as f64
        + ratio * PROGRESS_RANGE_MULTIPLIER;
    pct as f32
}

/// Progress reporter for tracking audio processing operations
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
    
    /// Updates processing stage
    pub fn set_stage(&mut self, stage: ProcessingStage) {
        self.current_stage = stage;
    }
    
    /// Sets current file being processed
    #[allow(dead_code)]
    pub fn set_current_file<S: Into<String>>(&mut self, filename: S) {
        self.current_file = Some(filename.into());
    }
    
    /// Increments completed file count
    #[allow(dead_code)]
    pub fn complete_file(&mut self) {
        self.files_completed += 1;
        self.current_file = None;
    }
    
    /// Calculates current progress percentage
    #[allow(dead_code)]
    pub fn calculate_progress(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        
        // Base progress on stage and files completed
        let _stage_weight = match self.current_stage {
            ProcessingStage::Analyzing => 0.1,
            ProcessingStage::Converting => 0.85,
            ProcessingStage::WritingMetadata => 0.05,
            ProcessingStage::Completed => 1.0,
            ProcessingStage::Failed(_) => 0.0,
        };
        
        let file_progress = self.files_completed as f32 / self.total_files as f32;
        
        match self.current_stage {
            ProcessingStage::Analyzing => PROGRESS_ANALYZING_END * file_progress,
            ProcessingStage::Converting => PROGRESS_CONVERTING_START + (PROGRESS_CONVERTING_RANGE * file_progress),
            ProcessingStage::WritingMetadata => PROGRESS_FINALIZING + (PROGRESS_METADATA_WEIGHT * file_progress),
            ProcessingStage::Completed => PROGRESS_COMPLETE,
            ProcessingStage::Failed(_) => 0.0,
        }
    }
    
    /// Estimates remaining time based on progress
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

    // parse_* tests moved to parser.rs in Phase 1
}


