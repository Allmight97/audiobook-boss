use crate::audio::{EncoderSettings, SampleRateConfig};
use crate::errors::Result;
use crate::output_artifact::{OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use crate::processing::lifecycle::OperationKind;
use crate::processing::preview_config::PreviewConfig;
use crate::processing::session::ProcessingSession;
use crate::processing::ProgressEvent;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(test)]
fn default_processing_workspace_root() -> PathBuf {
    std::env::temp_dir()
        .join("audiobook-boss")
        .join("processing")
        .join("sessions")
}

pub(crate) type ProgressEventListener = Arc<dyn Fn(&ProgressEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct OutputConfig {
    final_path: PathBuf,
    kind: OutputKind,
    action: PlannedOutputAction,
}

impl OutputConfig {
    #[cfg(test)]
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

    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    pub fn output_kind(&self) -> OutputKind {
        self.kind
    }

    pub fn commit_action(&self) -> PlannedOutputAction {
        self.action
    }
}

#[derive(Clone)]
pub struct ProcessingContext {
    pub session: Arc<ProcessingSession>,
    pub encoder_settings: Option<EncoderSettings>,
    pub sample_rate: SampleRateConfig,
    pub output: OutputConfig,
    workspace_root: PathBuf,
    pub preview: Option<PreviewConfig>,
    pub job_id: Option<String>,
    /// WorkRuntime operation identity; absent for foreground processing.
    pub(crate) operation_id: Option<String>,
    pub input_index: Option<usize>,
    pub operation_kind: OperationKind,
    pub(crate) progress_listener: Option<ProgressEventListener>,
    /// The export title's output record; publication goes through it.
    pub(crate) title_output: Option<Arc<crate::processing::TitleOutput>>,
}

impl std::fmt::Debug for ProcessingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingContext")
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
    pub(crate) fn new_with_workspace_root(
        session: Arc<ProcessingSession>,
        encoder_settings: impl Into<Option<EncoderSettings>>,
        sample_rate: SampleRateConfig,
        output: OutputConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            session,
            encoder_settings: encoder_settings.into(),
            sample_rate,
            output,
            workspace_root,
            preview: None,
            job_id: None,
            operation_id: None,
            input_index: None,
            operation_kind: OperationKind::ProcessingBatch,
            progress_listener: None,
            title_output: None,
        }
    }

    #[cfg(test)]
    pub fn new_headless(
        session: Arc<ProcessingSession>,
        encoder_settings: impl Into<Option<EncoderSettings>>,
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

    #[cfg(test)]
    pub fn new_headless_with_workspace_root(
        session: Arc<ProcessingSession>,
        encoder_settings: impl Into<Option<EncoderSettings>>,
        sample_rate: SampleRateConfig,
        output: OutputConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self::new_with_workspace_root(
            session,
            encoder_settings,
            sample_rate,
            output,
            workspace_root,
        )
    }

    pub(crate) fn required_encoder_settings(&self) -> Result<&EncoderSettings> {
        self.encoder_settings.as_ref().ok_or_else(|| {
            crate::errors::AppError::InvalidInput(
                "Encoder settings are required for encode processing.".into(),
            )
        })
    }

    pub fn is_cancelled(&self) -> bool {
        self.session.is_cancelled()
    }

    pub fn new_emitter(&self) -> crate::processing::progress::ProgressEmitter {
        crate::processing::progress::ProgressEmitter::with_context(
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

    pub fn effective_bitrate_kbps(&self) -> u32 {
        self.encoder_settings
            .as_ref()
            .map_or(0, |settings| settings.bitrate_kbps as u32)
    }
}
