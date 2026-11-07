use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::{blocking, Rest, Session};
use crate::errors::{AppErrorEnvelope, Result};
use crate::metadata_save::{save_metadata_batch, MetadataSaveRequest, MetadataSaveResultStatus};
use crate::processing::title_output::UpdateReply;
use crate::processing::{OutputUpdate, OutputUpdateStatus};
use crate::session::exports::{OutputEdit, OutputEdits};
use crate::session::intent::SessionOutcome;
use crate::session::state::{MetadataStatus, SaveItem, SavePlan};

struct Written {
    items: Vec<bool>,
    succeeded: usize,
    failed: usize,
    cancelled: usize,
}

impl Session {
    async fn write(&self, items: &[SaveItem]) -> Result<Written> {
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
            let Some(item) = items.get(entry.input_index) else {
                continue;
            };
            if entry.status == MetadataSaveResultStatus::Success {
                state.working_set.note_saved_names(&item.path, &item.patch);
            }
            if let Some(rewrite) = &entry.rewrite {
                state.working_set.note_tag_write(&item.path, rewrite);
            }
        }
        Ok(written)
    }

    fn ensure_deferred_writer(&self) {
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

    pub(super) fn begin_save(&self) -> Rest {
        let staged = &self.inner.deps.temporary_root;
        let staged_canonical = std::fs::canonicalize(staged).ok();
        let is_temporary = |path: &Path| {
            path.starts_with(staged)
                || staged_canonical
                    .as_ref()
                    .is_some_and(|root| path.starts_with(root))
        };
        let plan = self.transition(|state| {
            if state.working_set.files().is_empty() {
                return None;
            }
            let in_use = self.inner.deps.work.sources_in_use();
            let plan = state.begin_save(&in_use, is_temporary)?;
            Some((state.epoch, plan))
        });
        match plan {
            Some((epoch, plan)) => Rest::Save { epoch, plan },
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    pub(super) async fn save(&self, epoch: u64, plan: SavePlan) -> SessionOutcome {
        if plan.waiting > 0 {
            self.ensure_deferred_writer();
        }
        let outputs = self.update_outputs(plan.outputs).await;

        let mut saved = Vec::new();
        let mut status = MetadataStatus::SaveComplete {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            waiting: plan.waiting,
            held: plan.held,
            outputs,
        };
        let nothing = plan.immediate.is_empty()
            && plan.waiting == 0
            && plan.held == 0
            && outputs == OutputEdits::default();
        if nothing {
            status = if plan.grouped {
                MetadataStatus::GroupedEditsKept
            } else {
                MetadataStatus::NoPendingChanges
            };
        }
        let written_paths: Vec<PathBuf> = plan
            .immediate
            .iter()
            .map(|item| item.path.clone())
            .collect();
        if !plan.immediate.is_empty() {
            self.publish();
            match self.write(&plan.immediate).await {
                Ok(written) => {
                    status = MetadataStatus::SaveComplete {
                        succeeded: written.succeeded,
                        failed: written.failed,
                        cancelled: written.cancelled,
                        waiting: plan.waiting,
                        held: plan.held,
                        outputs,
                    };
                    saved = plan
                        .immediate
                        .into_iter()
                        .zip(written.items)
                        .filter_map(|(item, written)| written.then_some(item))
                        .collect();
                }
                Err(error) => {
                    log::error!("Failed to save metadata: {error}");
                    status = MetadataStatus::SaveFailed {
                        error: AppErrorEnvelope::from(&error),
                    };
                }
            }
        }
        self.transition(|state| state.finish_save(epoch, &written_paths, &saved, status));
        self.sources_released();
        SessionOutcome::Applied
    }

    pub(super) async fn update_outputs(&self, edits: Vec<OutputEdit>) -> OutputEdits {
        if edits.is_empty() {
            return OutputEdits::default();
        }
        let mut replies = Vec::with_capacity(edits.len());
        for edit in edits {
            let mut reply = edit
                .title
                .update(edit.revision, &edit.intent)
                .map_err(|error| error.to_string());
            if matches!(
                reply,
                Ok(UpdateReply::Accepted {
                    published: true,
                    ..
                })
            ) {
                let title = Arc::clone(&edit.title);
                match blocking(move || {
                    title.apply_published();
                    Ok(())
                })
                .await
                {
                    Err(error) => {
                        log::warn!("Output update did not finish: {error}");
                        reply = Err(error.to_string());
                    }
                    Ok(()) => {
                        if let Some(OutputUpdate {
                            status: OutputUpdateStatus::Failed { message },
                            ..
                        }) = edit.title.update_state()
                        {
                            reply = Err(message);
                        }
                    }
                }
            }
            replies.push((edit, reply));
        }
        self.transition(|state| state.record_output_edits(replies))
    }

    /// Brings outputs linked by a just-accepted export up to the session's
    /// edits: a Save made while the submission was being prepared did not
    /// reach the export.
    pub(super) async fn catch_up_outputs(&self, title_ids: &[String]) {
        let edits = self.transition(|state| {
            let tags = &state.tags;
            state
                .exports
                .edits(|path| tags.pending(path).map(|pending| pending.patch.clone()))
                .into_iter()
                .filter(|edit| title_ids.contains(&edit.title_id))
                .collect::<Vec<_>>()
        });
        self.update_outputs(edits).await;
    }

    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        self.lock().waiting_write_paths()
    }
}
