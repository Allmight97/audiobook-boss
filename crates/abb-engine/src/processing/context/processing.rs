use crate::output_artifact::{OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use crate::processing::session::ProcessingSession;
use crate::processing::OperationKind;
use crate::processing::ProgressEventListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

    #[cfg(test)]
    pub fn new_preview<P: Into<PathBuf>>(final_path: P) -> Self {
        Self {
            kind: OutputKind::Preview,
            ..Self::new(final_path)
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
    pub sample_rate: u32,
    pub aac_decoder: crate::audio::AacDecoder,
    pub output: OutputConfig,
    workspace_root: PathBuf,
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
            .field("sample_rate", &self.sample_rate)
            .field("aac_decoder", &self.aac_decoder)
            .field("output", &self.output)
            .field("workspace_root", &self.workspace_root)
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
        sample_rate: u32,
        aac_decoder: crate::audio::AacDecoder,
        output: OutputConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            session,
            sample_rate,
            aac_decoder,
            output,
            workspace_root,
            job_id: None,
            operation_id: None,
            input_index: None,
            operation_kind: OperationKind::ProcessingBatch,
            progress_listener: None,
            title_output: None,
        }
    }

    #[cfg(test)]
    pub fn new_headless_with_workspace_root(
        session: Arc<ProcessingSession>,
        sample_rate: u32,
        output: OutputConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self::new_with_workspace_root(
            session,
            sample_rate,
            crate::audio::AacDecoder::Auto,
            output,
            workspace_root,
        )
    }

    pub fn is_preview(&self) -> bool {
        self.output.output_kind() == OutputKind::Preview
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
}
