use super::{JobIdentity, ProcessingSession, ProgressEventListener, PublishGate};
use crate::output_artifact::{OutputKind, PlannedOutputAction, ResolvedOutputPlan};
use abb_processing_core::OperationKind;
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
    pub aac_decoder: super::AacDecoder,
    pub output: OutputConfig,
    pub(crate) workspace_root: PathBuf,
    pub(crate) identity: JobIdentity,
    pub operation_kind: OperationKind,
    pub(crate) progress_listener: ProgressEventListener,
    /// The export title's output record; publication goes through it.
    pub(crate) publish_gate: Option<Arc<dyn PublishGate>>,
}

impl std::fmt::Debug for ProcessingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingContext")
            .field("session", &self.session)
            .field("sample_rate", &self.sample_rate)
            .field("aac_decoder", &self.aac_decoder)
            .field("output", &self.output)
            .field("workspace_root", &self.workspace_root)
            .field("identity", &self.identity)
            .field("operation_kind", &self.operation_kind)
            .field("publish_gate", &self.publish_gate.is_some())
            .finish()
    }
}

impl ProcessingContext {
    #[cfg(test)]
    pub fn new_headless_with_workspace_root(
        session: Arc<ProcessingSession>,
        sample_rate: u32,
        output: OutputConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            session,
            sample_rate,
            aac_decoder: super::AacDecoder::Auto,
            output,
            workspace_root,
            identity: JobIdentity {
                operation_id: "test-operation".to_string(),
                job_id: None,
                input_index: 0,
            },
            operation_kind: OperationKind::ProcessingBatch,
            progress_listener: Arc::new(|_| {}),
            publish_gate: None,
        }
    }

    pub fn is_preview(&self) -> bool {
        self.output.output_kind() == OutputKind::Preview
    }

    pub fn is_cancelled(&self) -> bool {
        self.session.is_cancelled()
    }

    pub fn new_emitter(&self) -> super::ProgressEmitter {
        super::ProgressEmitter::new(
            super::EmitContext {
                operation_kind: self.operation_kind,
                identity: self.identity.clone(),
            },
            self.progress_listener.clone(),
        )
    }

    pub(crate) fn processing_workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}
