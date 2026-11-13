use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{blocking, Rest, Session};
use crate::errors::Result;
use crate::metadata_save::{save_metadata_batch, MetadataSaveRequest, MetadataSaveResultStatus};
use crate::processing::title_output::TitleOutput;
use crate::processing::title_output::UpdateReply;
use crate::processing::OutputUpdateStatus;
use crate::session::exports::{OutputEdit, OutputEdits};
use crate::session::intent::SessionOutcome;
use crate::session::state::{SaveItem, SavePlan, WriteBatch, WriteOutcome};

pub(super) async fn apply_published(title: &Arc<TitleOutput>) -> Result<()> {
    let title = Arc::clone(title);
    blocking(move || {
        title.apply_published();
        Ok(())
    })
    .await
}

pub(super) fn output_update_failure(title: &TitleOutput) -> Option<String> {
    match title.update_state()?.status {
        OutputUpdateStatus::Failed { message } => Some(message),
        _ => None,
    }
}

impl Session {
    async fn write(&self, items: &[SaveItem]) -> Result<WriteBatch> {
        let deps = &self.inner.deps;
        let requests = items
            .iter()
            .map(|item| MetadataSaveRequest {
                file_path: item.path.to_string_lossy().into_owned(),
                metadata_patch: item.patch.clone(),
            })
            .collect();
        let result = save_metadata_batch(&deps.host, &deps.work, &deps.jobs, requests).await?;
        let mut written = WriteBatch {
            outcomes: items.iter().cloned().map(WriteOutcome::failed).collect(),
            succeeded: result.summary.succeeded,
            failed: result.summary.failed,
            cancelled: result.summary.cancelled,
        };
        for entry in result.results {
            if let Some(outcome) = written.outcomes.get_mut(entry.input_index) {
                outcome.written = entry.status == MetadataSaveResultStatus::Success;
                outcome.rewrite = entry.rewrite;
            }
        }
        Ok(written)
    }

    fn ensure_deferred_writer(&self) {
        if !self.lock().deferred_writer.take() {
            return;
        }
        let session = self.clone();
        let claim = self.writer_claim();
        self.inner
            .deps
            .tasks
            .spawn(async move { session.write_deferred_when_free(claim).await });
    }

    async fn write_deferred_when_free(&self, claim: super::in_flight::InFlight) {
        let mut changes = self.inner.deps.work.subscribe_changes();
        loop {
            changes.borrow_and_update();
            let ready = {
                let mut state = self.lock();
                let in_use = self.inner.deps.work.sources_in_use();
                if !state.has_waiting_writes() {
                    claim.end_in(&mut state);
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
            let results = match self.write(&ready).await {
                Ok(written) => written.outcomes,
                Err(error) => {
                    log::error!("Failed to save waiting metadata: {error}");
                    ready.into_iter().map(WriteOutcome::failed).collect()
                }
            };
            self.transition(|state| state.finish_deferred(&results));
            self.publish();
        }
    }

    pub(super) fn begin_save(&self) -> Rest {
        let staged = &self.inner.deps.temporary_root;
        let is_temporary = |path: &Path| path.starts_with(staged);
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

    pub(super) async fn save(&self, epoch: u64, mut plan: SavePlan) -> SessionOutcome {
        if plan.waiting > 0 {
            self.ensure_deferred_writer();
        }
        let outputs = self.update_outputs(std::mem::take(&mut plan.outputs)).await;
        let written = if plan.immediate.is_empty() {
            Ok(WriteBatch::default())
        } else {
            self.publish();
            self.write(&plan.immediate).await.inspect_err(|error| {
                log::error!("Failed to save metadata: {error}");
            })
        };
        self.transition(|state| state.finish_save(epoch, &plan, outputs, written));
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
                match apply_published(&edit.title).await {
                    Err(error) => {
                        log::warn!("Output update did not finish: {error}");
                        reply = Err(error.to_string());
                    }
                    Ok(()) => {
                        if let Some(message) = output_update_failure(&edit.title) {
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
        let edits = self.transition(|state| state.output_edits(Some(title_ids)));
        self.update_outputs(edits).await;
    }

    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        self.lock().waiting_write_paths()
    }
}
