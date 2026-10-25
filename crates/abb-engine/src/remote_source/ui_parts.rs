//! What the remote UI state publishes to hosts: the remote and library
//! parts, each with its own revision.
use super::{RemoteLibrarySnapshot, RemoteUiSnapshot};
use crate::host::EngineEvent;
use crate::remote_source::RemoteSourceRuntime;

impl RemoteSourceRuntime {
    pub(crate) fn ui_revisions(&self) -> (u64, u64) {
        let state = self.ui();
        (state.snapshot.revision, state.library.revision)
    }
    pub(crate) fn ui_parts(
        &self,
        since: Option<(u64, u64)>,
    ) -> (Option<RemoteUiSnapshot>, Option<RemoteLibrarySnapshot>) {
        let state = self.ui();
        (
            since
                .is_none_or(|revision| state.snapshot.revision > revision.0)
                .then(|| state.snapshot()),
            since
                .is_none_or(|revision| state.library.revision > revision.1)
                .then(|| state.library.clone()),
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
            update
        };
        self.inner.ui_host.emit(EngineEvent::Session(update));
    }
}
