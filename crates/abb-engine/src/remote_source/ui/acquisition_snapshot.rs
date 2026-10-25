//! How an acquisition is stored on the remote snapshot hosts read.

use super::super::{AcquisitionJob, AcquisitionProgress, ProviderId, RemoteAcquisitionFailureKind};
use super::UiState;

impl UiState {
    /// Records a download tick for the job the snapshot already shows, without
    /// a new revision, so the next attach reads the latest percentage.
    /// Returns whether the tick landed. A cancel or failure that moved the
    /// stage leaves the snapshot unchanged.
    pub(in crate::remote_source) fn note_progress(
        &mut self,
        job_id: &str,
        progress: AcquisitionProgress,
    ) -> bool {
        let Some(acquisition) = self.snapshot.acquisition.as_mut() else {
            return false;
        };
        if acquisition.job_id != job_id || acquisition.progress.stage != progress.stage {
            return false;
        }
        acquisition.job.progress = progress;
        true
    }

    pub(in crate::remote_source) fn acquisition_changed(&mut self, job: AcquisitionJob) {
        if job.provider_id == ProviderId::Audible
            && job
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == RemoteAcquisitionFailureKind::AuthRequired)
        {
            self.reject_sign_in();
        }
        self.snapshot.acquisition = Some(Box::new(job.into()));
        self.changed();
    }
}
