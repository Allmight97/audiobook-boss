//! Progress reporting for audio processing operations

use super::{ProcessingProgress, ProcessingStage};
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
            ProcessingStage::Analyzing => 10.0 * file_progress,
            ProcessingStage::Converting => 10.0 + (70.0 * file_progress),
            ProcessingStage::Merging => 80.0 + (15.0 * file_progress),
            ProcessingStage::WritingMetadata => 95.0 + (5.0 * file_progress),
            ProcessingStage::Completed => 100.0,
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

/// Parses FFmpeg progress output to extract percentage
pub fn parse_ffmpeg_progress(line: &str) -> Option<f32> {
    // FFmpeg outputs progress in format: "time=00:01:30.45"
    if line.starts_with("time=") {
        if let Some(time_str) = line.strip_prefix("time=") {
            if let Ok(duration) = parse_ffmpeg_time(time_str) {
                // This is just the current time, need total duration to calculate %
                // For now, return a basic progress indicator
                return Some(duration as f32);
            }
        }
    }
    
    // Look for progress= in FFmpeg output
    if line.contains("progress=") {
        if let Some(progress_str) = line.split("progress=").nth(1) {
            if progress_str.trim() == "end" {
                return Some(100.0);
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
        assert_eq!(parse_ffmpeg_progress("time=00:01:30.45").unwrap(), 90.45);
        assert_eq!(parse_ffmpeg_progress("progress=end").unwrap(), 100.0);
        assert!(parse_ffmpeg_progress("other output").is_none());
    }
}