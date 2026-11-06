use std::sync::atomic::Ordering;

use super::{SaveItem, Session};
use crate::errors::Result;
use crate::metadata_save::{save_metadata_batch, MetadataSaveRequest, MetadataSaveResultStatus};

pub(super) struct Written {
    pub(super) items: Vec<bool>,
    pub(super) succeeded: usize,
    pub(super) failed: usize,
    pub(super) cancelled: usize,
}

impl Session {
    pub(super) async fn write(&self, items: &[SaveItem]) -> Result<Written> {
        let deps = &self.inner.deps;
        let requests = items
            .iter()
            .map(|item| MetadataSaveRequest {
                file_path: item.path.to_string_lossy().into_owned(),
                metadata_patch: item.patch.clone(),
            })
            .collect();
        let result = save_metadata_batch(&deps.host, &deps.work, &deps.jobs, requests).await?;
        let mut written = Written {
            items: vec![false; items.len()],
            succeeded: result.summary.succeeded,
            failed: result.summary.failed,
            cancelled: result.summary.cancelled,
        };
        let mut state = self.lock();
        for entry in result.results {
            if let Some(slot) = written.items.get_mut(entry.input_index) {
                *slot = entry.status == MetadataSaveResultStatus::Success;
            }
            if let (Some(item), Some(rewrite)) = (items.get(entry.input_index), &entry.rewrite) {
                state.working_set.note_tag_write(&item.path, rewrite);
            }
        }
        Ok(written)
    }

    pub(super) fn ensure_deferred_writer(&self) {
        if self
            .inner
            .deferred_writer_running
            .swap(true, Ordering::SeqCst)
        {
            return;
        }
        let session = self.clone();
        self.inner
            .deps
            .tasks
            .spawn(async move { session.write_deferred_when_free().await });
    }

    async fn write_deferred_when_free(&self) {
        let mut changes = self.inner.deps.work.subscribe_changes();
        loop {
            changes.borrow_and_update();
            let ready = {
                let mut state = self.lock();
                let in_use = self.inner.deps.work.sources_in_use();
                if !state.has_waiting_writes() {
                    self.inner
                        .deferred_writer_running
                        .store(false, Ordering::SeqCst);
                    return;
                }
                state.take_ready_deferred(&in_use)
            };
            if ready.is_empty() {
                tokio::select! {
                    changed = changes.changed() => {
                        if changed.is_err() {
                            return;
                        }
                    }
                    () = self.inner.sources_freed.notified() => {}
                }
                continue;
            }
            let written = self.write(&ready).await;
            let results: Vec<(SaveItem, bool)> = ready
                .into_iter()
                .enumerate()
                .map(|(index, item)| {
                    let ok = written
                        .as_ref()
                        .is_ok_and(|written| written.items.get(index) == Some(&true));
                    (item, ok)
                })
                .collect();
            self.transition(|state| state.finish_deferred(&results));
            self.publish();
        }
    }
}
