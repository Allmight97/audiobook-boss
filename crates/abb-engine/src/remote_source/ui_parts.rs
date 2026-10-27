//! What the remote UI state publishes to hosts: the remote, library, and
//! acquisition parts, each with its own revision.
use super::{
    ProviderId, RemoteAcquisitionSnapshot, RemoteAuthStatus, RemoteDraftStatus,
    RemoteLibrarySnapshot, RemoteUiSnapshot,
};
use crate::host::EngineEvent;
use crate::remote_source::{RemoteRevisions, RemoteSourceRuntime};

/// A snapshot that carries its own revision.
pub(super) trait Revisioned: Clone {
    fn revision(&self) -> u64;
    fn revision_mut(&mut self) -> &mut u64;
}
impl Revisioned for RemoteUiSnapshot {
    fn revision(&self) -> u64 {
        self.revision
    }
    fn revision_mut(&mut self) -> &mut u64 {
        &mut self.revision
    }
}
impl Revisioned for RemoteLibrarySnapshot {
    fn revision(&self) -> u64 {
        self.revision
    }
    fn revision_mut(&mut self) -> &mut u64 {
        &mut self.revision
    }
}
impl Revisioned for RemoteAcquisitionSnapshot {
    fn revision(&self) -> u64 {
        self.revision
    }
    fn revision_mut(&mut self) -> &mut u64 {
        &mut self.revision
    }
}

/// One published part: its value and the last revision sent to hosts.
#[derive(Default)]
pub(super) struct Part<T> {
    value: T,
    published: u64,
}
impl<T: Revisioned> Part<T> {
    pub(super) fn new(value: T) -> Self {
        Self {
            value,
            published: 0,
        }
    }
    fn current(&self) -> u64 {
        self.value.revision()
    }
    /// Advances the revision; every edit that hosts must see ends here.
    pub(super) fn bump(&mut self) {
        *self.value.revision_mut() += 1;
    }
    fn newer_than(&self, since: Option<u64>) -> Option<T> {
        let revision = self.current();
        since
            .is_none_or(|since| revision > since)
            .then(|| self.value.clone())
    }
    fn unpublished(&mut self) -> Option<T> {
        let revision = self.current();
        (revision > self.published).then(|| {
            self.published = revision;
            self.value.clone()
        })
    }
}
impl<T> std::ops::Deref for Part<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> std::ops::DerefMut for Part<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl super::UiState {
    pub(in crate::remote_source) fn snapshot(&self) -> RemoteUiSnapshot {
        RemoteUiSnapshot {
            busy: self.lane_busy(),
            ..(*self.snapshot).clone()
        }
    }
    pub(super) fn changed(&mut self) {
        self.snapshot.bump();
    }
    fn lane_busy(&self) -> bool {
        let remote = &*self.snapshot;
        remote.account_status == RemoteDraftStatus::Running
            || match remote.lane {
                ProviderId::Audible => {
                    remote.library_status == RemoteDraftStatus::Running
                        || matches!(
                            remote.auth,
                            RemoteAuthStatus::Starting | RemoteAuthStatus::Completing
                        )
                        || self.acquisition.busy()
                }
                ProviderId::Indexer => {
                    remote.indexer.searching
                        || remote.indexer.grabbing
                        || remote.connection.save == RemoteDraftStatus::Running
                }
            }
    }

    /// Changes the acquisition part; the remote part moves too when `busy` flips.
    pub(in crate::remote_source) fn set_acquisition(
        &mut self,
        change: impl FnOnce(&mut RemoteAcquisitionSnapshot),
    ) {
        let was_busy = self.acquisition.busy();
        change(&mut self.acquisition);
        self.acquisition.bump();
        if self.acquisition.busy() != was_busy {
            self.changed();
        }
    }

    /// Records `job`, and its update: the acquisition part, plus the remote
    /// part when the job changed it (a rejected sign-in, or `busy`).
    pub(in crate::remote_source) fn acquisition_changed(
        &mut self,
        job: crate::remote_source::AcquisitionJob,
    ) -> crate::session::SessionUpdate {
        let remote_before = self.snapshot.current();
        if job.provider_id == ProviderId::Audible
            && job.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::remote_source::RemoteAcquisitionFailureKind::AuthRequired
            })
        {
            self.reject_sign_in();
            self.changed();
        }
        self.set_acquisition(|acquisition| acquisition.job = Some(Box::new(job.into())));
        let remote_moved = self.snapshot.current() != remote_before;
        crate::session::SessionUpdate {
            remote: remote_moved
                .then(|| self.snapshot.unpublished().map(|_| self.snapshot()))
                .flatten(),
            remote_acquisition: self.acquisition.unpublished(),
            ..Default::default()
        }
    }
    /// Every part not yet sent to hosts, marked sent.
    fn publish(&mut self) -> crate::session::SessionUpdate {
        crate::session::SessionUpdate {
            remote: self.snapshot.unpublished().map(|_| self.snapshot()),
            remote_library: self.library.unpublished(),
            remote_acquisition: self.acquisition.unpublished(),
            ..Default::default()
        }
    }
}

impl RemoteSourceRuntime {
    /// Adds the parts newer than `since` to `update`; every part for `None`.
    pub(crate) fn add_ui_parts(
        &self,
        update: &mut crate::session::SessionUpdate,
        since: Option<RemoteRevisions>,
    ) {
        let state = self.ui();
        update.remote = state
            .snapshot
            .newer_than(since.map(|since| since.remote))
            .map(|_| state.snapshot());
        update.remote_library = state.library.newer_than(since.map(|since| since.library));
        update.remote_acquisition = state
            .acquisition
            .newer_than(since.map(|since| since.acquisition));
    }
    pub(crate) fn ui_revisions(&self) -> RemoteRevisions {
        let state = self.ui();
        RemoteRevisions {
            remote: state.snapshot.current(),
            library: state.library.current(),
            acquisition: state.acquisition.current(),
        }
    }
    #[cfg(test)]
    pub(crate) fn ui_parts(
        &self,
        since: Option<RemoteRevisions>,
    ) -> (
        Option<RemoteUiSnapshot>,
        Option<RemoteLibrarySnapshot>,
        Option<RemoteAcquisitionSnapshot>,
    ) {
        let mut update = crate::session::SessionUpdate::default();
        self.add_ui_parts(&mut update, since);
        (
            update.remote,
            update.remote_library,
            update.remote_acquisition,
        )
    }
    #[cfg(test)]
    pub(crate) fn ui_snapshot(&self) -> RemoteUiSnapshot {
        self.ui().snapshot()
    }
    pub(in crate::remote_source) fn publish_ui(&self) {
        let update = self.ui().publish();
        self.inner.ui_host.emit(EngineEvent::Session(update));
    }
}
