use super::{
    indexer_lane, lane_status, ProviderId, ReleaseGrabStatus, RemoteAcquisitionSnapshot,
    RemoteDraftStatus, RemoteLibrarySnapshot, RemoteUiSnapshot,
};
use crate::host::EngineEvent;
use crate::remote_source::{RemoteRevisions, RemoteSourceRuntime};

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
        let remote = &*self.snapshot;
        let mut snapshot = remote.clone();
        snapshot.busy = self.lane_busy();
        snapshot.connection.uses_http = indexer_lane::uses_http(&remote.connection.base_url);
        snapshot.connection.category_options = self
            .indexer
            .category_options(&remote.connection.category_ids);
        snapshot.audible_message =
            lane_status::audible_message(remote, &self.audible, &self.library);
        if remote.lane == ProviderId::Indexer {
            snapshot.indexer.message = lane_status::indexer_message(remote);
        }
        snapshot.indexer.can_grab_selection = remote.indexer.releases.iter().any(|release| {
            remote.indexer.selected_release_keys.contains(&release.key)
                && remote
                    .indexer
                    .release_grabs
                    .get(&release.key)
                    .is_none_or(|grab| grab.status != ReleaseGrabStatus::Sent)
        });
        snapshot
    }
    pub(super) fn changed(&mut self) {
        self.snapshot.bump();
    }
    fn lane_busy(&self) -> bool {
        let remote = &*self.snapshot;
        remote.account_status == RemoteDraftStatus::Running
            || match remote.lane {
                ProviderId::Audible => self.audible.is_busy(self.acquisition.busy()),
                ProviderId::Indexer => self.indexer_busy(),
            }
    }

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

    pub(in crate::remote_source) fn acquisition_changed(
        &mut self,
        job: crate::remote_source::AcquisitionSnapshot,
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
        let already_settled = self
            .acquisition
            .job
            .as_ref()
            .is_some_and(|known| known.settled && known.job_id == job.job_id);
        if job.settled && !already_settled {
            self.audible.settled_job_line = Some(lane_status::settled_job_line(&job));
        }
        self.set_acquisition(|acquisition| acquisition.job = Some(Box::new(job)));
        let remote_moved = self.snapshot.current() != remote_before;
        crate::session::SessionUpdate {
            remote: remote_moved
                .then(|| self.snapshot.unpublished().map(|_| self.snapshot()))
                .flatten(),
            remote_acquisition: self.acquisition.unpublished(),
            ..Default::default()
        }
    }
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
    pub(crate) fn ui_snapshot(&self) -> RemoteUiSnapshot {
        self.ui().snapshot()
    }
    pub(in crate::remote_source) fn publish_ui(&self) {
        let update = self.ui().publish();
        self.inner.ui_host.emit(EngineEvent::Session(update));
    }
}
