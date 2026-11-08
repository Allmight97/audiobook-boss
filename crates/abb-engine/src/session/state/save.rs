use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::super::exports::{OutputEdit, OutputEdits};
use super::{MetadataStatus, SaveItem, SessionState, StageOutcome, WriteOutcome};
use crate::errors::{AppErrorEnvelope, Result};

#[derive(Debug)]
pub(crate) struct SavePlan {
    pub(crate) immediate: Vec<SaveItem>,
    pub(crate) waiting: usize,
    pub(crate) held: usize,
    pub(crate) outputs: Vec<OutputEdit>,
    pub(crate) grouped: bool,
}

/// What the writer reports for the files a Save wrote at once.
#[derive(Debug, Default)]
pub(crate) struct WriteBatch {
    pub(crate) outcomes: Vec<WriteOutcome>,
    pub(crate) succeeded: usize,
    pub(crate) failed: usize,
    pub(crate) cancelled: usize,
}

impl SessionState {
    fn busy(&self, in_use: &HashSet<PathBuf>) -> HashSet<PathBuf> {
        in_use
            .iter()
            .chain(self.submitting.sources())
            .cloned()
            .chain(self.staged.removing_paths())
            .collect()
    }

    /// Decides where each pending edit goes. `in_use` is every source an
    /// accepted export has yet to finish reading; `is_temporary` says whether
    /// a source is a download the engine will remove.
    ///
    /// Returns `None`, with the reason in the status, when there is nothing to
    /// write.
    pub(crate) fn begin_save(
        &mut self,
        in_use: &HashSet<PathBuf>,
        is_temporary: impl Fn(&Path) -> bool,
    ) -> Option<SavePlan> {
        let in_use = &self.busy(in_use);
        if self.working_set.files().is_empty() {
            return None;
        }
        if self.writes.save_running() {
            self.status = Some(MetadataStatus::SaveAlreadyInProgress);
            return None;
        }
        if let StageOutcome::Invalid(_) = self.stage_bound_form() {
            self.status = Some(MetadataStatus::SaveInvalid);
            return None;
        }

        let files = self.working_set.files();
        let grouped = files
            .iter()
            .any(|file| self.working_set.sources_for(file).len() > 1);
        let items: Vec<SaveItem> = files
            .iter()
            .filter(|file| file.is_valid && self.working_set.sources_for(file).len() == 1)
            .filter_map(|file| {
                self.tags.pending(&file.path).map(|pending| SaveItem {
                    path: file.path.clone(),
                    patch: pending.patch.clone(),
                    revision: pending.revision,
                })
            })
            .collect();
        let outputs = self.output_edits(None);
        if items.is_empty() && outputs.is_empty() {
            self.status = Some(if grouped {
                MetadataStatus::GroupedEditsKept
            } else {
                MetadataStatus::NoPendingChanges
            });
            return None;
        }

        let routing = self.writes.route(items, in_use, is_temporary);
        let plan = SavePlan {
            immediate: routing.immediate,
            waiting: routing.waiting,
            held: routing.held,
            outputs,
            grouped,
        };
        self.status = Some(MetadataStatus::PreparingSave);
        Some(plan)
    }

    /// The edit each linked output should carry: what was written to its
    /// source plus the edit still pending. `only` names the titles asked for.
    pub(crate) fn output_edits(&mut self, only: Option<&[String]>) -> Vec<OutputEdit> {
        let tags = &self.tags;
        let mut edits = self
            .exports
            .edits(|path| tags.pending(path).map(|pending| pending.patch.clone()));
        if let Some(only) = only {
            edits.retain(|edit| only.contains(&edit.title_id));
        }
        edits
    }

    /// Records how a Save ended. `written` is the writer's report for
    /// `plan.immediate`; a plan with nothing to write passes an empty batch.
    pub(crate) fn finish_save(
        &mut self,
        epoch: u64,
        plan: &SavePlan,
        outputs: OutputEdits,
        written: Result<WriteBatch>,
    ) {
        let nothing = plan.immediate.is_empty()
            && plan.waiting == 0
            && plan.held == 0
            && outputs == OutputEdits::default();
        let (outcomes, status) = if nothing {
            let status = if plan.grouped {
                MetadataStatus::GroupedEditsKept
            } else {
                MetadataStatus::NoPendingChanges
            };
            (Vec::new(), status)
        } else {
            match written {
                Ok(batch) => (
                    batch.outcomes,
                    MetadataStatus::SaveComplete {
                        succeeded: batch.succeeded,
                        failed: batch.failed,
                        cancelled: batch.cancelled,
                        waiting: plan.waiting,
                        held: plan.held,
                        outputs,
                    },
                ),
                Err(error) => (
                    Vec::new(),
                    MetadataStatus::SaveFailed {
                        error: AppErrorEnvelope::from(&error),
                    },
                ),
            }
        };
        let paths: Vec<PathBuf> = plan
            .immediate
            .iter()
            .map(|item| item.path.clone())
            .collect();
        self.writes.release(&paths);
        let current = epoch == self.epoch;
        if current {
            let loaded = self.working_set.source_paths();
            let owner = self.cover_owner();
            for outcome in outcomes.iter().filter(|outcome| outcome.written) {
                let item = &outcome.item;
                let cover_was_submitted = item.patch.cover_art.is_some();
                self.record_written(outcome, loaded.contains(&item.path));
                let cover_still_pending = self
                    .tags
                    .pending(&item.path)
                    .is_some_and(|pending| pending.patch.cover_art.is_some());
                if cover_was_submitted && !cover_still_pending && owner.as_ref() == Some(&item.path)
                {
                    self.cover.custom = false;
                }
            }
        }
        if current {
            self.status = Some(status);
        }
    }

    fn record_written(&mut self, outcome: &WriteOutcome, loaded: bool) {
        let item = &outcome.item;
        self.working_set.note_saved_names(&item.path, &item.patch);
        if let Some(rewrite) = &outcome.rewrite {
            self.working_set.note_tag_write(&item.path, rewrite);
        }
        self.exports.acknowledge(&item.path, &item.patch);
        if item.patch.cover_art.is_some() {
            self.source_covers.record_write(&item.path);
        }
        if loaded {
            self.tags
                .commit_saved(&item.path, &item.patch, item.revision);
        }
    }

    pub(crate) fn take_ready_deferred(&mut self, in_use: &HashSet<PathBuf>) -> Vec<SaveItem> {
        let in_use = &self.busy(in_use);
        self.writes.take_ready(in_use)
    }

    pub(crate) fn has_waiting_writes(&self) -> bool {
        self.writes.has_waiting()
    }

    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        self.writes.waiting_paths()
    }

    pub(crate) fn finish_deferred(&mut self, results: &[WriteOutcome]) {
        let loaded = self.working_set.source_paths();
        let mut written_count = 0;
        for outcome in results {
            self.writes.finish_write(&outcome.item, outcome.written);
            if outcome.written {
                self.record_written(outcome, loaded.contains(&outcome.item.path));
            }
            written_count += usize::from(outcome.written);
        }
        self.restore_failed_writes();
        let shown = results.iter().any(|outcome| {
            self.bound
                .iter()
                .any(|title| title.path == outcome.item.path)
        });
        if shown {
            let fresh = self.form_from_known_tags();
            self.form.rehydrate(fresh);
        }
        if !results.is_empty() {
            self.status = Some(MetadataStatus::DeferredWritesFinished {
                written: written_count,
                failed: results.len() - written_count,
            });
        }
    }

    pub(super) fn restore_failed_writes(&mut self) {
        let loaded = self.working_set.source_paths();
        for item in self
            .writes
            .failed()
            .filter(|item| loaded.contains(&item.path))
        {
            let pending = self.tags.pending(&item.path);
            let mut patch = item.patch.clone();
            if let Some(pending) = pending {
                patch.merge(&pending.patch);
            }
            if pending.is_none_or(|pending| pending.patch != patch) {
                self.tags.stage(&item.path, &patch);
            }
        }
    }

    pub(crate) fn removable_staged(&self, in_use: &HashSet<PathBuf>, now: Instant) -> Vec<String> {
        if self.writes.is_writing() || self.staged.is_empty() {
            return Vec::new();
        }
        self.staged.removable(&self.busy(in_use), now)
    }

    pub(crate) fn begin_staged_removal(
        &mut self,
        in_use: &HashSet<PathBuf>,
        now: Instant,
    ) -> Vec<String> {
        let jobs = self.removable_staged(in_use, now);
        for job_id in &jobs {
            self.staged.removing(job_id);
        }
        jobs
    }
}
