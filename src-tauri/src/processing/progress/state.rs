//! Progress state machine for tracking audio processing operations

use crate::processing::{ProcessingProgress, ProcessingStage};
use std::time::Instant;

/// Progress reporter for tracking audio processing operations
pub struct ProgressReporter {
    /// Total number of files to process
    total_files: usize,
    /// Files completed so far
    files_completed: usize,
    /// Current processing stage
    current_stage: ProcessingStage,
    /// Start time of processing
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
    pub fn set_current_file<S: Into<String>>(&mut self, filename: S) {
        self.current_file = Some(filename.into());
    }

    /// Increments completed file count
    pub fn complete_file(&mut self) {
        self.files_completed += 1;
        self.current_file = None;
    }

    /// Calculates current progress percentage
    pub fn calculate_progress(&self) -> f32 {
        use crate::audio::constants::*;

        if self.total_files == 0 {
            return 0.0;
        }

        let file_progress = self.files_completed as f32 / self.total_files as f32;

        match self.current_stage {
            ProcessingStage::Analyzing => PROGRESS_ANALYZING_END * file_progress,
            ProcessingStage::Converting => {
                PROGRESS_CONVERTING_START + (PROGRESS_CONVERTING_RANGE * file_progress)
            }
            ProcessingStage::WritingMetadata => {
                PROGRESS_FINALIZING + (PROGRESS_METADATA_WEIGHT * file_progress)
            }
            ProcessingStage::Completed => PROGRESS_COMPLETE,
            ProcessingStage::Failed(_) => 0.0,
        }
    }

    /// Estimates remaining time based on progress
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
    pub fn fail<S: Into<String>>(&mut self, error: S) {
        self.current_stage = ProcessingStage::Failed(error.into());
        self.current_file = None;
    }
}
