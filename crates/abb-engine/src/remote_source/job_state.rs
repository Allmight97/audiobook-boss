use abb_remote_source_core::{acquisition_progress, AcquisitionProgress, AcquisitionStage};

use super::cancellation::remote_acquisition_cancelled;
use super::types::{
    AcquiredContent, AcquiredJob, AcquisitionHandoff, AcquisitionJob, AcquisitionSnapshot,
    AcquisitionWork, MaterializedSourceFile, PdfTitle, RemoteAcquisitionFailureKind,
    RemoteSourceDiagnostic, StagedPdf, SupplementalAsset, TitlesHandoff,
};

const PDF_SAVE_MESSAGE: &str = "Saving the Supplemental PDF.";

/// What a provider hands back when it has worked through a job. The lifecycle
/// applies it to the job it registered.
#[derive(Debug)]
pub(crate) enum AcquisitionOutcome {
    Acquired {
        content: AcquiredContent,
        diagnostics: Vec<RemoteSourceDiagnostic>,
    },
    Failed {
        diagnostics: Vec<RemoteSourceDiagnostic>,
    },
}

/// What a provider gathers while it works through titles.
#[derive(Debug, Default)]
pub(crate) struct TitlesOutcome {
    pub(crate) files: Vec<MaterializedSourceFile>,
    pub(crate) assets: Vec<SupplementalAsset>,
    pub(crate) diagnostics: Vec<RemoteSourceDiagnostic>,
}

impl From<TitlesOutcome> for AcquisitionOutcome {
    fn from(outcome: TitlesOutcome) -> Self {
        let TitlesOutcome {
            files,
            assets,
            diagnostics,
        } = outcome;
        if files.is_empty() {
            return Self::Failed { diagnostics };
        }
        Self::Acquired {
            content: AcquiredContent::Titles { files, assets },
            diagnostics,
        }
    }
}

impl AcquisitionOutcome {
    pub(crate) fn failed(kind: RemoteAcquisitionFailureKind, message: String) -> Self {
        Self::refused(RemoteSourceDiagnostic {
            kind,
            title_id: None,
            message,
        })
    }

    pub(crate) fn refused(diagnostic: RemoteSourceDiagnostic) -> Self {
        Self::Failed {
            diagnostics: vec![diagnostic],
        }
    }

    pub(crate) fn pdf(pdf: StagedPdf, title: PdfTitle) -> Self {
        Self::Acquired {
            content: AcquiredContent::Pdf { pdf, title },
            diagnostics: Vec::new(),
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
    Validated(AcquiredContent),
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
    pub(super) fn new(job_id: &str, work: &AcquisitionWork) -> Self {
        Self {
            job: AcquisitionJob::new(job_id, work),
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
            JobState::Validated(_)
                | JobState::Settled(Settlement::Handoff(AcquisitionHandoff::Titles(
                    TitlesHandoff::Imported { .. }
                )))
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
        let content = match outcome {
            AcquisitionOutcome::Failed { diagnostics } => {
                self.job.diagnostics.extend(diagnostics);
                self.job.progress =
                    acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None);
                self.state = JobState::Settled(Settlement::Failed);
                return true;
            }
            AcquisitionOutcome::Acquired {
                content,
                diagnostics,
            } => {
                self.job.diagnostics.extend(diagnostics);
                content
            }
        };
        let mut progress =
            acquisition_progress(AcquisitionStage::ImportHandoff, Some(1.0), None, None);
        if let AcquiredContent::Pdf { .. } = content {
            progress.message = PDF_SAVE_MESSAGE.to_string();
            progress.current_title_id = self.job.progress.current_title_id.clone();
        }
        self.job.progress = progress;
        self.state = JobState::Validated(content);
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

    pub(super) fn awaits_handoff(&self) -> bool {
        matches!(self.state, JobState::Validated(_))
    }

    pub(super) fn handoff_done(&mut self, handoff: AcquisitionHandoff) -> bool {
        if !self.awaits_handoff() {
            return false;
        }
        self.job.handoff = Some(handoff.clone());
        self.state = JobState::Settled(Settlement::Handoff(handoff));
        true
    }

    /// The job as the session's handoff receives it, while its content waits
    /// for the handoff.
    pub(super) fn job_for_handoff(&self) -> Option<AcquiredJob> {
        let JobState::Validated(content) = &self.state else {
            return None;
        };
        Some(AcquiredJob {
            job_id: self.job.job_id.clone(),
            content: content.clone(),
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
