use abb_remote_source_core::{acquisition_progress, AcquisitionProgress, AcquisitionStage};

use super::cancellation::remote_acquisition_cancelled;
use super::types::{
    AcquisitionHandoff, AcquisitionJob, AcquisitionSnapshot, MaterializedSourceFile,
    RemoteAcquisitionFailureKind, RemoteSourceDiagnostic, SupplementalAsset,
};

/// What a provider hands back when it has worked through a plan. The lifecycle
/// applies it to the job it registered.
#[derive(Debug, Default)]
pub(crate) struct AcquisitionOutcome {
    pub(crate) files: Vec<MaterializedSourceFile>,
    pub(crate) assets: Vec<SupplementalAsset>,
    pub(crate) diagnostics: Vec<RemoteSourceDiagnostic>,
}

impl AcquisitionOutcome {
    pub(crate) fn failed(kind: RemoteAcquisitionFailureKind, message: String) -> Self {
        Self {
            diagnostics: vec![RemoteSourceDiagnostic {
                kind,
                title_id: None,
                message,
            }],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Settlement {
    Handoff(AcquisitionHandoff),
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum JobState {
    Running,
    Validated {
        files: Vec<MaterializedSourceFile>,
        assets: Vec<SupplementalAsset>,
    },
    Settled(Settlement),
}

/// One registered acquisition job. State only moves forward: Running to
/// Validated or Settled, Validated to Settled. Progress is a display value and
/// never moves it.
#[derive(Debug, Clone)]
pub(super) struct Registered {
    job: AcquisitionJob,
    state: JobState,
}

impl Registered {
    pub(super) fn new(job_id: &str) -> Self {
        Self {
            job: AcquisitionJob::new(job_id),
            state: JobState::Running,
        }
    }

    pub(super) fn provider_id(&self) -> super::types::ProviderId {
        self.job.provider_id
    }

    pub(super) fn is_running(&self) -> bool {
        self.state == JobState::Running
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.state == JobState::Settled(Settlement::Cancelled)
    }

    pub(super) fn terminal(&self) -> bool {
        !self.is_running()
    }

    pub(super) fn settled(&self) -> bool {
        matches!(self.state, JobState::Settled(_))
    }

    /// The staged files are still wanted by the session: handed over and
    /// listed, or waiting to be.
    pub(super) fn holds_staged_files(&self) -> bool {
        matches!(
            self.state,
            JobState::Validated { .. }
                | JobState::Settled(Settlement::Handoff(AcquisitionHandoff::Imported { .. }))
        )
    }

    /// Shows `progress` while the job runs; says whether its stage changed.
    pub(super) fn show_progress(&mut self, progress: AcquisitionProgress) -> Option<bool> {
        if !self.is_running() {
            return None;
        }
        let new_stage = self.job.progress.stage != progress.stage;
        self.job.progress = progress;
        Some(new_stage)
    }

    pub(super) fn finish(&mut self, outcome: AcquisitionOutcome) -> bool {
        if !self.is_running() {
            return false;
        }
        self.job.diagnostics.extend(outcome.diagnostics);
        if outcome.files.is_empty() {
            self.job.progress =
                acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None);
            self.state = JobState::Settled(Settlement::Failed);
        } else {
            self.job.progress =
                acquisition_progress(AcquisitionStage::ImportHandoff, Some(1.0), None, None);
            self.state = JobState::Validated {
                files: outcome.files,
                assets: outcome.assets,
            };
        }
        true
    }

    pub(super) fn cancel(&mut self) -> bool {
        if !self.is_running() {
            return false;
        }
        self.job.progress =
            acquisition_progress(AcquisitionStage::Cancelled, Some(1.0), None, None);
        if !self
            .job
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == RemoteAcquisitionFailureKind::Cancelled)
        {
            self.job.diagnostics.push(RemoteSourceDiagnostic {
                kind: RemoteAcquisitionFailureKind::Cancelled,
                title_id: None,
                message: remote_acquisition_cancelled().to_string(),
            });
        }
        self.state = JobState::Settled(Settlement::Cancelled);
        true
    }

    pub(super) fn handoff_done(&mut self, handoff: AcquisitionHandoff) -> bool {
        if !matches!(self.state, JobState::Validated { .. }) {
            return false;
        }
        self.job.handoff = Some(handoff.clone());
        self.state = JobState::Settled(Settlement::Handoff(handoff));
        true
    }

    /// The job as the session's handoff receives it, with its files, while
    /// the files wait for the handoff.
    pub(super) fn job_for_handoff(&self) -> Option<AcquisitionJob> {
        let JobState::Validated { files, assets } = &self.state else {
            return None;
        };
        Some(AcquisitionJob {
            materialized_files: files.clone(),
            supplemental_assets: assets.clone(),
            ..self.job.clone()
        })
    }

    pub(super) fn snapshot(&self) -> AcquisitionSnapshot {
        AcquisitionSnapshot {
            job: self.job.clone(),
            terminal: self.terminal(),
            settled: self.settled(),
        }
    }
}

#[cfg(test)]
#[path = "job_state_tests.rs"]
pub(super) mod tests;
