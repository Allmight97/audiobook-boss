//! Processing context structures and builders.

use crate::audio::{EncoderSettings, SampleRateConfig};
use crate::errors::sanitize_path_str_for_display;
use crate::errors::Result;
use crate::output_artifact::{OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use crate::processing::lifecycle::OperationKind;
use crate::processing::preview_config::PreviewConfig;
use crate::processing::session::ProcessingSession;
use crate::processing::ProgressEvent;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Window;

fn default_processing_workspace_root() -> PathBuf {
    std::env::temp_dir()
        .join("audiobook-boss")
        .join("processing")
        .join("sessions")
}

pub(crate) type ProgressEventListener = Arc<dyn Fn(&ProgressEvent) + Send + Sync>;

/// Output configuration derived from user input
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Final destination artifact for the processed audiobook or preview.
    final_path: PathBuf,
    kind: OutputKind,
    action: PlannedOutputAction,
}

impl OutputConfig {
    /// Creates a new output configuration
    pub fn new<P: Into<PathBuf>>(final_path: P) -> Self {
        Self {
            final_path: final_path.into(),
            kind: OutputKind::Final,
            action: PlannedOutputAction::Write,
        }
    }

    /// Creates an output configuration for a preview artifact path.
    pub fn for_preview<P: Into<PathBuf>>(final_path: P) -> Self {
        Self {
            final_path: final_path.into(),
            kind: OutputKind::Preview,
            action: PlannedOutputAction::Write,
        }
    }

    pub(crate) fn from_plan(plan: ResolvedOutputPlan) -> Self {
        Self {
            final_path: plan.resolved_path,
            kind: plan.kind,
            action: plan.action,
        }
    }

    /// Returns a reference to the final output path
    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    pub fn artifact_path(&self) -> &Path {
        &self.final_path
    }

    pub fn output_kind(&self) -> OutputKind {
        self.kind
    }

    pub fn commit_action(&self) -> PlannedOutputAction {
        self.action
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
#[derive(Clone)]
pub struct ProcessingContext {
    /// Optional Tauri window for event emission (None in headless/test runs)
    pub window: Option<Window>,
    /// Processing session with state management
    pub session: Arc<ProcessingSession>,
    /// Encoder settings
    pub encoder_settings: EncoderSettings,
    /// Sample rate configuration
    pub sample_rate: SampleRateConfig,
    /// Output configuration
    pub output: OutputConfig,
    /// App-owned local workspace root for in-flight processing artifacts.
    workspace_root: PathBuf,
    /// Optional preview configuration (when present, processing should early-stop)
    pub preview: Option<PreviewConfig>,
    /// Optional job identifier for parallel batch processing
    pub job_id: Option<String>,
    /// Optional WorkRuntime operation identifier
    pub operation_id: Option<String>,
    /// Optional index of the input file in the original request
    pub input_index: Option<usize>,
    /// Backend operation family for lifecycle events emitted by this context
    pub operation_kind: OperationKind,
    pub(crate) progress_listener: Option<ProgressEventListener>,
}

impl std::fmt::Debug for ProcessingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingContext")
            .field("window", &self.window)
            .field("session", &self.session)
            .field("encoder_settings", &self.encoder_settings)
            .field("sample_rate", &self.sample_rate)
            .field("output", &self.output)
            .field("workspace_root", &self.workspace_root)
            .field("preview", &self.preview)
            .field("job_id", &self.job_id)
            .field("operation_id", &self.operation_id)
            .field("input_index", &self.input_index)
            .field("operation_kind", &self.operation_kind)
            .field("progress_listener", &self.progress_listener.is_some())
            .finish()
    }
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
        Self::new_with_workspace_root(
            window,
            session,
            encoder_settings,
            sample_rate,
            output,
            default_processing_workspace_root(),
        )
    }

    pub fn new_with_workspace_root(
        window: Window,
        session: Arc<ProcessingSession>,
        encoder_settings: EncoderSettings,
        sample_rate: SampleRateConfig,
        output: OutputConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            window: Some(window),
            session,
            encoder_settings,
            sample_rate,
            output,
            workspace_root,
            preview: None,
            job_id: None,
            operation_id: None,
            input_index: None,
            operation_kind: OperationKind::ProcessingBatch,
            progress_listener: None,
        }
    }

    /// Creates a headless ProcessingContext (no UI event emission).
    pub fn new_headless(
        session: Arc<ProcessingSession>,
        encoder_settings: EncoderSettings,
        sample_rate: SampleRateConfig,
        output: OutputConfig,
    ) -> Self {
        Self::new_headless_with_workspace_root(
            session,
            encoder_settings,
            sample_rate,
            output,
            default_processing_workspace_root(),
        )
    }

    pub fn new_headless_with_workspace_root(
        session: Arc<ProcessingSession>,
        encoder_settings: EncoderSettings,
        sample_rate: SampleRateConfig,
        output: OutputConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            window: None,
            session,
            encoder_settings,
            sample_rate,
            output,
            workspace_root,
            preview: None,
            job_id: None,
            operation_id: None,
            input_index: None,
            operation_kind: OperationKind::ProcessingBatch,
            progress_listener: None,
        }
    }

    /// Emits an event to the frontend
    pub fn emit_event<S: serde::Serialize + Clone>(
        &self,
        event_name: &str,
        payload: S,
    ) -> Result<()> {
        use tauri::Emitter;
        if let Some(window) = &self.window {
            window.emit(event_name, payload).map_err(|e| {
                crate::errors::AppError::General(format!(
                    "Failed to emit event '{}' for session {}: {}",
                    event_name,
                    self.session.id(),
                    e
                ))
            })?;
        }
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
            sanitize_path_str_for_display(file_path),
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
    pub fn new_emitter(&self) -> crate::processing::progress::ProgressEmitter {
        let emitter = match &self.window {
            Some(window) => crate::processing::progress::ProgressEmitter::with_context(
                window.clone(),
                self.operation_kind,
                self.operation_id.clone(),
                self.job_id.clone(),
                self.input_index,
            ),
            None => crate::processing::progress::ProgressEmitter::headless_for(self.operation_kind),
        };

        emitter.with_progress_listener(self.progress_listener.clone())
    }

    pub(crate) fn processing_workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Returns the effective bitrate in kbps
    pub fn effective_bitrate_kbps(&self) -> u32 {
        self.encoder_settings.bitrate_kbps as u32
    }

    /// Returns the effective channel count
    pub fn effective_channel_count(&self) -> u8 {
        self.encoder_settings
            .channels
            .forced_channels()
            // Metrics-only; when auto is in use, channel count is resolved during probe/encoder setup.
            .unwrap_or(0)
    }
}

/// Builder pattern for ProcessingContext
#[derive(Default)]
pub struct ProcessingContextBuilder {
    window: Option<Window>,
    session: Option<Arc<ProcessingSession>>,
    encoder_settings: Option<EncoderSettings>,
    sample_rate: Option<SampleRateConfig>,
    output: Option<OutputConfig>,
    preview: Option<PreviewConfig>,
    job_id: Option<String>,
    operation_id: Option<String>,
    input_index: Option<usize>,
    operation_kind: OperationKind,
    workspace_root: Option<PathBuf>,
    progress_listener: Option<ProgressEventListener>,
}

impl ProcessingContextBuilder {
    /// Creates a new builder instance
    pub fn new() -> Self {
        Self::default()
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

    /// Sets the preview configuration
    pub fn preview(mut self, preview: PreviewConfig) -> Self {
        self.preview = Some(preview);
        self
    }

    /// Sets the job ID
    pub fn job_id(mut self, job_id: String) -> Self {
        self.job_id = Some(job_id);
        self
    }

    /// Sets the WorkRuntime operation ID
    pub fn operation_id(mut self, operation_id: String) -> Self {
        self.operation_id = Some(operation_id);
        self
    }

    /// Sets the input index
    pub fn input_index(mut self, input_index: usize) -> Self {
        self.input_index = Some(input_index);
        self
    }

    pub fn progress_listener(mut self, progress_listener: ProgressEventListener) -> Self {
        self.progress_listener = Some(progress_listener);
        self
    }

    /// Sets the operation kind for lifecycle reporting
    pub fn operation_kind(mut self, operation_kind: OperationKind) -> Self {
        self.operation_kind = operation_kind;
        self
    }

    pub fn workspace_root(mut self, workspace_root: PathBuf) -> Self {
        self.workspace_root = Some(workspace_root);
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
        let workspace_root = self
            .workspace_root
            .unwrap_or_else(default_processing_workspace_root);

        Ok(ProcessingContext {
            window: Some(window),
            session,
            encoder_settings,
            sample_rate,
            output,
            workspace_root,
            preview: self.preview,
            job_id: self.job_id,
            operation_id: self.operation_id,
            input_index: self.input_index,
            operation_kind: self.operation_kind,
            progress_listener: self.progress_listener,
        })
    }
}
