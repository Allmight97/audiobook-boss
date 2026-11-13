use abb_remote_source_core::{acquisition_progress, AcquisitionProgress, AcquisitionStage};

use super::cancellation::remote_acquisition_cancelled;
use super::types::{
    AcquiredContent, AcquiredJob, AcquisitionHandoff, AcquisitionJob, AcquisitionJobState,
    AcquisitionSnapshot, AcquisitionWork, JobEnding, MaterializedSourceFile, PdfTitle,
    RemoteAcquisitionFailureKind, RemoteSourceDiagnostic, StagedPdf, SupplementalAsset,
    TitlesHandoff,
};

const PDF_SAVE_MESSAGE: &str = "Saving the Supplemental PDF.";
const FAILED_MESSAGE: &str = "Acquisition failed.";
const CANCELLED_MESSAGE: &str = "Acquisition cancelled.";

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
pub(super) enum JobState {
    Running,
    Validated(AcquiredContent),
    Done(JobEnding),
}

/// One registered acquisition job. State only moves forward: Running to
/// Validated or Done, Validated to Done. Progress is a display value and
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
        self.state == JobState::Done(JobEnding::Cancelled)
    }

    pub(super) fn is_done(&self) -> bool {
        matches!(self.state, JobState::Done(_))
    }

    /// The staged files are still wanted by the session: handed over and
    /// listed, or waiting to be.
    pub(super) fn holds_staged_files(&self) -> bool {
        matches!(
            self.state,
            JobState::Validated(_)
                | JobState::Done(JobEnding::Handoff {
                    handoff: AcquisitionHandoff::Titles(TitlesHandoff::Imported { .. })
                })
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
                self.end(JobEnding::Failed, FAILED_MESSAGE);
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
        self.end(JobEnding::Cancelled, CANCELLED_MESSAGE);
        true
    }

    /// Ends a running job. Its progress keeps the last working stage and
    /// shows the ending at 100%.
    fn end(&mut self, ending: JobEnding, message: &str) {
        self.job.progress = AcquisitionProgress {
            stage: self.job.progress.stage,
            percentage: 100.0,
            message: message.to_string(),
            bytes_downloaded: None,
            bytes_total: None,
            current_title_id: None,
            current_item_index: None,
            total_items: None,
        };
        self.state = JobState::Done(ending);
    }

    pub(super) fn awaits_handoff(&self) -> bool {
        matches!(self.state, JobState::Validated(_))
    }

    pub(super) fn handoff_done(&mut self, handoff: AcquisitionHandoff) -> bool {
        if !self.awaits_handoff() {
            return false;
        }
        self.state = JobState::Done(JobEnding::Handoff { handoff });
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
        let state = match &self.state {
            JobState::Running => AcquisitionJobState::Running,
            JobState::Validated(_) => AcquisitionJobState::HandingOver,
            JobState::Done(ending) => AcquisitionJobState::Done {
                ending: ending.clone(),
            },
        };
        AcquisitionSnapshot {
            job: self.job.clone(),
            state,
        }
    }
}

#[cfg(test)]
#[path = "job_state_tests.rs"]
pub(super) mod tests;
