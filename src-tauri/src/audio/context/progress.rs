//! Progress context structures and builders.

use crate::audio::ProcessingStage;

/// Groups progress-related parameters together
///
/// This context contains all the information needed for progress reporting
/// and tracking during audio processing operations.
#[derive(Clone)]
pub struct ProgressContext {
    /// Current processing stage
    pub stage: ProcessingStage,
    /// Overall progress percentage (0.0 - 100.0)
    pub progress: f32,
    /// Optional message describing current operation
    pub message: Option<String>,
    /// Current file being processed
    pub current_file: Option<String>,
    /// Number of files completed
    pub files_completed: usize,
    /// Total number of files to process
    pub total_files: usize,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
}

impl ProgressContext {
    /// Creates a new ProgressContext with minimal information
    pub fn new(stage: ProcessingStage, progress: f32) -> Self {
        Self {
            stage,
            progress,
            message: None,
            current_file: None,
            files_completed: 0,
            total_files: 0,
            eta_seconds: None,
        }
    }

    /// Updates the progress percentage
    pub fn with_progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 100.0);
        self
    }

    /// Sets the message
    pub fn with_message<S: Into<String>>(mut self, message: S) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the current file being processed
    pub fn with_current_file<S: Into<String>>(mut self, file: S) -> Self {
        self.current_file = Some(file.into());
        self
    }

    /// Sets the file completion status
    pub fn with_file_progress(mut self, completed: usize, total: usize) -> Self {
        self.files_completed = completed;
        self.total_files = total;
        self
    }

    /// Sets the estimated time remaining
    pub fn with_eta(mut self, seconds: f64) -> Self {
        self.eta_seconds = Some(seconds);
        self
    }

    /// Calculates progress based on files completed
    pub fn calculate_file_progress(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        (self.files_completed as f32 / self.total_files as f32) * 100.0
    }

    /// Creates a formatted progress message with file context
    pub fn format_progress_message(&self) -> String {
        let mut message = format!("Stage: {:?}, Progress: {:.1}%", self.stage, self.progress);

        if let Some(ref current_file) = self.current_file {
            message.push_str(&format!(" | Current file: {current_file}"));
        }

        if self.total_files > 0 {
            message.push_str(&format!(
                " | Files: {}/{}",
                self.files_completed, self.total_files
            ));
        }

        if let Some(eta) = self.eta_seconds {
            let minutes = (eta / 60.0) as i32;
            let seconds = (eta % 60.0) as i32;
            message.push_str(&format!(" | ETA: {minutes}m {seconds}s"));
        }

        if let Some(ref msg) = self.message {
            message.push_str(&format!(" | {msg}"));
        }

        message
    }

    /// Creates an error with progress context
    pub fn create_error(&self, operation: &str, reason: &str) -> crate::errors::AppError {
        let progress_info = self.format_progress_message();
        crate::errors::AppError::General(format!(
            "Failed to {operation} during processing ({progress_info}): {reason}"
        ))
    }
}

/// Builder pattern for ProgressContext
pub struct ProgressContextBuilder {
    stage: ProcessingStage,
    progress: f32,
    message: Option<String>,
    current_file: Option<String>,
    files_completed: usize,
    total_files: usize,
    eta_seconds: Option<f64>,
}

impl ProgressContextBuilder {
    /// Creates a new builder with required fields
    pub fn new(stage: ProcessingStage) -> Self {
        Self {
            stage,
            progress: 0.0,
            message: None,
            current_file: None,
            files_completed: 0,
            total_files: 0,
            eta_seconds: None,
        }
    }

    /// Sets the progress percentage
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 100.0);
        self
    }

    /// Sets the message
    pub fn message<S: Into<String>>(mut self, message: S) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the current file
    pub fn current_file<S: Into<String>>(mut self, file: S) -> Self {
        self.current_file = Some(file.into());
        self
    }

    /// Sets the file progress
    pub fn file_progress(mut self, completed: usize, total: usize) -> Self {
        self.files_completed = completed;
        self.total_files = total;
        self
    }

    /// Sets the ETA
    pub fn eta(mut self, seconds: f64) -> Self {
        self.eta_seconds = Some(seconds);
        self
    }

    /// Builds the ProgressContext
    pub fn build(self) -> ProgressContext {
        ProgressContext {
            stage: self.stage,
            progress: self.progress,
            message: self.message,
            current_file: self.current_file,
            files_completed: self.files_completed,
            total_files: self.total_files,
            eta_seconds: self.eta_seconds,
        }
    }
}
