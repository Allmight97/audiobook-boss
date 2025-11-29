//! Context structures for reducing parameter passing in audio processing
//!
//! This module provides ProcessingContext and ProgressContext structures
//! that group related parameters together, reducing function parameter counts
//! and improving code organization.

use super::session::ProcessingSession;
use super::settings_encoder::EncoderSettings;
use super::AudioSettings;
use crate::audio::ProcessingStage;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Window;

/// Preview configuration for early-stop preview encodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewConfig {
    /// Number of seconds to encode before early-stop
    pub seconds: f64,
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
    /// Audio processing settings
    pub settings: AudioSettings,
    /// Output configuration
    pub output: OutputConfig,
    /// Optional advanced encoder settings (v2) carried through the pipeline
    pub encoder_settings_v2: Option<EncoderSettings>,
    /// Optional preview configuration (when present, processing should early-stop)
    pub preview: Option<PreviewConfig>,
}

impl ProcessingContext {
    /// Creates a new ProcessingContext with the given components
    pub fn new(
        window: Window,
        session: Arc<ProcessingSession>,
        settings: AudioSettings,
        output: OutputConfig,
    ) -> Self {
        Self {
            window,
            session,
            settings,
            output,
            encoder_settings_v2: None,
            preview: None,
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

    /// Checks if processing is currently active
    pub fn is_processing(&self) -> bool {
        self.session.is_processing()
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
        self.encoder_settings_v2
            .as_ref()
            .map(|enc| enc.bitrate_kbps as u32)
            .unwrap_or(self.settings.bitrate)
    }

    /// Returns the effective channel count (v2-aware)
    pub fn effective_channel_count(&self) -> u8 {
        self.encoder_settings_v2
            .as_ref()
            .and_then(|enc| enc.channels.forced_channels())
            .unwrap_or_else(|| self.settings.channels.channel_count())
    }
}

/// Builder pattern for ProcessingContext
pub struct ProcessingContextBuilder {
    window: Option<Window>,
    session: Option<Arc<ProcessingSession>>,
    settings: Option<AudioSettings>,
    output: Option<OutputConfig>,
}

impl ProcessingContextBuilder {
    /// Creates a new builder instance
    pub fn new() -> Self {
        Self {
            window: None,
            session: None,
            settings: None,
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

    /// Sets the audio settings
    pub fn settings(mut self, settings: AudioSettings) -> Self {
        self.settings = Some(settings);
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
        let settings = self.settings.ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Failed to build ProcessingContext: Audio settings are required for processing configuration"
                    .to_string(),
            )
        })?;
        let output = self.output.ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Failed to build ProcessingContext: Output configuration is required for finalization"
                    .to_string(),
            )
        })?;

        Ok(ProcessingContext::new(window, session, settings, output))
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
