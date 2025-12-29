//! Processing context structures and builders.

use super::super::session::ProcessingSession;
use super::super::settings_encoder::EncoderSettings;
use super::super::SampleRateConfig;
use crate::audio::preview_config::PreviewConfig;
use crate::errors::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Window;

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

    /// Creates a progress emitter scoped to this processing context
    pub fn new_emitter(&self) -> crate::audio::progress::ProgressEmitter {
        crate::audio::progress::ProgressEmitter::with_context(
            self.window.clone(),
            self.job_id.clone(),
            self.input_index,
        )
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
