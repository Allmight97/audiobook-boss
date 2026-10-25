//! What the remote UI state publishes to hosts: the remote, library, and
//! acquisition parts, each with its own revision.
use super::{ProviderId, RemoteAcquisitionSnapshot, RemoteLibrarySnapshot, RemoteUiSnapshot};
use crate::host::EngineEvent;
use crate::remote_source::RemoteSourceRuntime;

/// The remote, library, and acquisition parts, each when newer than asked.
pub(crate) type RemoteParts = (
    Option<RemoteUiSnapshot>,
    Option<RemoteLibrarySnapshot>,
    Option<RemoteAcquisitionSnapshot>,
);

impl super::UiState {
    /// Records `job`, and its update: the acquisition part, plus the remote
    /// part when the job also rejected the saved sign-in.
    pub(in crate::remote_source) fn acquisition_changed(
        &mut self,
        job: crate::remote_source::AcquisitionJob,
    ) -> crate::session::SessionUpdate {
        let mut update = crate::session::SessionUpdate::default();
        if job.provider_id == ProviderId::Audible
            && job.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::remote_source::RemoteAcquisitionFailureKind::AuthRequired
            })
        {
            self.reject_sign_in();
            self.changed();
            update.remote = Some(self.snapshot());
        }
        self.set_acquisition(|acquisition| acquisition.job = Some(Box::new(job.into())));
        self.acquisition_published = self.acquisition.revision;
        update.remote_acquisition = Some(self.acquisition.clone());
        update
    }
}

impl RemoteSourceRuntime {
    /// Adds the parts newer than `since` to `update`; every part for `None`.
    pub(crate) fn add_ui_parts(
        &self,
        update: &mut crate::session::SessionUpdate,
        since: Option<(u64, u64, u64)>,
    ) {
        (
            update.remote,
            update.remote_library,
            update.remote_acquisition,
        ) = self.ui_parts(since);
    }
    pub(crate) fn ui_revisions(&self) -> (u64, u64, u64) {
        let state = self.ui();
        (
            state.snapshot.revision,
            state.library.revision,
            state.acquisition.revision,
        )
    }
    pub(crate) fn ui_parts(&self, since: Option<(u64, u64, u64)>) -> RemoteParts {
        let state = self.ui();
        let newer = |revision: u64, at: fn((u64, u64, u64)) -> u64| {
            since.is_none_or(|since| revision > at(since))
        };
        (
            newer(state.snapshot.revision, |since| since.0).then(|| state.snapshot()),
            newer(state.library.revision, |since| since.1).then(|| state.library.clone()),
            newer(state.acquisition.revision, |since| since.2).then(|| state.acquisition.clone()),
        )
    }
    #[cfg(test)]
    pub(crate) fn ui_snapshot(&self) -> RemoteUiSnapshot {
        self.ui().snapshot()
    }
    pub(in crate::remote_source) fn publish_ui(&self) {
        let update = {
            let mut state = self.ui();
            let mut update = crate::session::SessionUpdate::remote(state.snapshot());
            if state.library.revision > state.library_published {
                update.remote_library = Some(state.library.clone());
                state.library_published = state.library.revision;
            }
            if state.acquisition.revision > state.acquisition_published {
                update.remote_acquisition = Some(state.acquisition.clone());
                state.acquisition_published = state.acquisition.revision;
            }
            update
        };
        self.inner.ui_host.emit(EngineEvent::Session(update));
    }
}
