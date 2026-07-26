//! Processing context structures and builders.

use crate::audio::{EncoderSettings, SampleRateConfig};
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
            .field("input_index", &self.input_index)
            .field("operation_kind", &self.operation_kind)
            .field("progress_listener", &self.progress_listener.is_some())
            .finish()
    }
}

impl ProcessingContext {
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

    /// Creates a progress emitter scoped to this processing context.
    ///
    /// Background (WorkRuntime) operations carry a `progress_listener` and report
    /// through snapshots; they must NOT also emit `processing-progress`/`-queue` to
    /// the window, or every progress fact is emitted twice (the snapshot AND a
    /// foreground event the Status Panel drops). So when a listener is present the
    /// emitter is built windowless. Foreground operations have no listener and emit
    /// to the window.
    pub fn new_emitter(&self) -> crate::processing::progress::ProgressEmitter {
        let window = if self.progress_listener.is_some() {
            None
        } else {
            self.window.clone()
        };
        crate::processing::progress::ProgressEmitter::with_context(
            window,
            crate::processing::progress::EmitContext {
                operation_kind: self.operation_kind,
                job_id: self.job_id.clone(),
                input_index: self.input_index,
            },
        )
        .with_progress_listener(self.progress_listener.clone())
    }

    pub(crate) fn processing_workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Returns the effective bitrate in kbps
    pub fn effective_bitrate_kbps(&self) -> u32 {
        self.encoder_settings.bitrate_kbps as u32
    }
}
