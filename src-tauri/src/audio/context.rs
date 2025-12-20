// REFACTOR: Module exceeds 400 LOC (508). Consider splitting before adding new code.
//! Context structures for reducing parameter passing in audio processing
//!
//! This module provides ProcessingContext and ProgressContext structures
//! that group related parameters together, reducing function parameter counts
//! and improving code organization.

use super::session::ProcessingSession;
use super::settings_encoder::EncoderSettings;
use super::SampleRateConfig;
use crate::audio::ProcessingStage;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Window;

/// Minimum segment duration per file (in seconds) for adaptive preview
pub const PREVIEW_MIN_SEGMENT_SECONDS: f64 = 5.0;

/// Preview configuration for early-stop preview encodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewConfig {
    /// Total preview duration requested by user (15/30/45/60s)
    pub total_seconds: f64,
    /// Minimum segment per file (default 5.0s)
    pub min_segment_seconds: f64,
}

impl PreviewConfig {
    /// Creates a new preview configuration with the given total duration
    pub fn new(total_seconds: f64) -> Self {
        Self {
            total_seconds,
            min_segment_seconds: PREVIEW_MIN_SEGMENT_SECONDS,
        }
    }

    /// Calculate per-file excerpt duration based on file count
    ///
    /// The duration is divided equally across files, with a floor at
    /// `min_segment_seconds` to avoid fragments that are too short.
    pub fn per_file_seconds(&self, file_count: usize) -> f64 {
        if file_count == 0 {
            return self.total_seconds;
        }
        let calculated = self.total_seconds / file_count as f64;
        calculated.max(self.min_segment_seconds)
    }
}

/// Output configuration derived from user input
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Final destination for the processed audiobook
    final_path: PathBuf,
}

impl OutputConfig {
    /// Creates a new output configuration
    pub fn new<P: Into<PathBuf>>(final_path: P) -> Self {
        Self {
            final_path: final_path.into(),
        }
    }

    /// Returns a reference to the final output path
    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    /// Consumes the config and returns the final path
    pub fn into_final_path(self) -> PathBuf {
        self.final_path
    }
}

/// Groups core processing dependencies together
///
/// This context contains the essential components needed for audio processing,
/// reducing the need to pass multiple parameters through function calls.
#[derive(Clone, Debug)]
pub struct ProcessingContext {
    /// Tauri window for event emission
    pub window: Window,
    /// Processing session with state management
    pub session: Arc<ProcessingSession>,
    /// Encoder settings (v2-only)
    pub encoder_settings: EncoderSettings,
    /// Sample rate configuration
    pub sample_rate: SampleRateConfig,
    /// Output configuration
    pub output: OutputConfig,
    /// Optional preview configuration (when present, processing should early-stop)
    pub preview: Option<PreviewConfig>,
    /// Optional job identifier for parallel batch processing
    pub job_id: Option<String>,
    /// Optional index of the input file in the original request
    pub input_index: Option<usize>,
}

impl ProcessingContext {
    /// Creates a new ProcessingContext with the given components
    pub fn new(
        window: Window,
        session: Arc<ProcessingSession>,
        encoder_settings: EncoderSettings,
        sample_rate: SampleRateConfig,
        output: OutputConfig,
    ) -> Self {
        Self {
            window,
            session,
            encoder_settings,
            sample_rate,
            output,
            preview: None,
            job_id: None,
            input_index: None,
        }
    }

    /// Emits an event to the frontend
    pub fn emit_event<S: serde::Serialize + Clone>(
        &self,
        event_name: &str,
        payload: S,
    ) -> Result<()> {
        use tauri::Emitter;
        self.window.emit(event_name, payload).map_err(|e| {
            crate::errors::AppError::General(format!(
                "Failed to emit event '{}' for session {}: {}",
                event_name,
                self.session.id(),
                e
            ))
        })?;
        Ok(())
    }

    /// Checks if the current processing has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.session.is_cancelled()
    }

    /// Creates an error with session context
    pub fn create_error(&self, operation: &str, reason: &str) -> crate::errors::AppError {
        crate::errors::AppError::General(format!(
            "Failed to {operation} for session {}: {reason}",
            self.session.id()
        ))
    }

    /// Creates a file validation error with session and file context
    pub fn create_file_error(
        &self,
        operation: &str,
        file_path: &str,
        reason: &str,
    ) -> crate::errors::AppError {
        crate::errors::AppError::FileValidation(format!(
            "Failed to {operation} file '{}' in session {}: {reason}",
            file_path,
            self.session.id()
        ))
    }

    /// Creates an input validation error with session context
    pub fn create_input_error(&self, field: &str, reason: &str) -> crate::errors::AppError {
        crate::errors::AppError::InvalidInput(format!(
            "Failed to validate {field} for session {}: {reason}",
            self.session.id()
        ))
    }

    /// Returns the effective bitrate in kbps (v2-aware)
    pub fn effective_bitrate_kbps(&self) -> u32 {
        self.encoder_settings.bitrate_kbps as u32
    }

    /// Returns the effective channel count (v2-aware)
    pub fn effective_channel_count(&self) -> u8 {
        self.encoder_settings
            .channels
            .forced_channels()
            // Metrics-only; when auto is in use, channel count is resolved during probe/encoder setup.
            .unwrap_or(0)
    }
}

/// Builder pattern for ProcessingContext
pub struct ProcessingContextBuilder {
    window: Option<Window>,
    session: Option<Arc<ProcessingSession>>,
    encoder_settings: Option<EncoderSettings>,
    sample_rate: Option<SampleRateConfig>,
    output: Option<OutputConfig>,
}

impl ProcessingContextBuilder {
    /// Creates a new builder instance
    pub fn new() -> Self {
        Self {
            window: None,
            session: None,
            encoder_settings: None,
            sample_rate: None,
            output: None,
        }
    }

    /// Sets the Tauri window
    pub fn window(mut self, window: Window) -> Self {
        self.window = Some(window);
        self
    }

    /// Sets the processing session
    pub fn session(mut self, session: Arc<ProcessingSession>) -> Self {
        self.session = Some(session);
        self
    }

    /// Sets the encoder settings
    pub fn encoder_settings(mut self, encoder_settings: EncoderSettings) -> Self {
        self.encoder_settings = Some(encoder_settings);
        self
    }

    /// Sets the sample rate configuration
    pub fn sample_rate(mut self, sample_rate: SampleRateConfig) -> Self {
        self.sample_rate = Some(sample_rate);
        self
    }

    /// Sets the output configuration
    pub fn output(mut self, output: OutputConfig) -> Self {
        self.output = Some(output);
        self
    }

    /// Builds the ProcessingContext
    ///
    /// # Errors
    /// Returns an error if any required field is missing
    pub fn build(self) -> Result<ProcessingContext> {
        let window = self.window.ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Failed to build ProcessingContext: Tauri window is required for event emission"
                    .to_string(),
            )
        })?;
        let session = self.session.ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Failed to build ProcessingContext: Processing session is required for state management"
                    .to_string(),
            )
        })?;
        let encoder_settings = self.encoder_settings.ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Failed to build ProcessingContext: Encoder settings are required for processing configuration"
                    .to_string(),
            )
        })?;
        let sample_rate = self.sample_rate.ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Failed to build ProcessingContext: Sample rate configuration is required for processing"
                    .to_string(),
            )
        })?;
        let output = self.output.ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Failed to build ProcessingContext: Output configuration is required for finalization"
                    .to_string(),
            )
        })?;

        Ok(ProcessingContext::new(
            window,
            session,
            encoder_settings,
            sample_rate,
            output,
        ))
    }
}

impl Default for ProcessingContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_config_per_file_seconds_basic_division() {
        let cfg = PreviewConfig::new(30.0);
        // 30s / 3 files = 10s each
        assert!((cfg.per_file_seconds(3) - 10.0).abs() < 0.001);
    }

    #[test]
    fn preview_config_per_file_seconds_floor_applied() {
        let cfg = PreviewConfig::new(30.0);
        // 30s / 7 files = 4.28s, but floor is 5.0s
        assert!((cfg.per_file_seconds(7) - 5.0).abs() < 0.001);
    }

    #[test]
    fn preview_config_per_file_seconds_single_file() {
        let cfg = PreviewConfig::new(30.0);
        // 30s / 1 file = 30s
        assert!((cfg.per_file_seconds(1) - 30.0).abs() < 0.001);
    }

    #[test]
    fn preview_config_per_file_seconds_zero_files() {
        let cfg = PreviewConfig::new(30.0);
        // Edge case: 0 files returns total_seconds
        assert!((cfg.per_file_seconds(0) - 30.0).abs() < 0.001);
    }

    #[test]
    fn preview_config_per_file_seconds_exact_floor_boundary() {
        let cfg = PreviewConfig::new(30.0);
        // 30s / 6 files = 5.0s exactly (at floor boundary)
        assert!((cfg.per_file_seconds(6) - 5.0).abs() < 0.001);
    }

    #[test]
    fn preview_config_different_durations() {
        // Test with 15s preset
        let cfg15 = PreviewConfig::new(15.0);
        assert!((cfg15.per_file_seconds(3) - 5.0).abs() < 0.001); // 15/3 = 5s

        // Test with 45s preset
        let cfg45 = PreviewConfig::new(45.0);
        assert!((cfg45.per_file_seconds(3) - 15.0).abs() < 0.001); // 45/3 = 15s

        // Test with 60s preset
        let cfg60 = PreviewConfig::new(60.0);
        assert!((cfg60.per_file_seconds(4) - 15.0).abs() < 0.001); // 60/4 = 15s
    }
}
