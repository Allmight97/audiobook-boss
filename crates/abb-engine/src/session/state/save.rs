use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::super::exports::OutputEdit;
use super::{DeferredPhase, DeferredWrite, MetadataStatus, SessionState, StageOutcome};
use crate::metadata::MetadataIntentPatch;

#[derive(Debug, Clone)]
pub(crate) struct SaveItem {
    pub(crate) path: PathBuf,
    pub(crate) patch: MetadataIntentPatch,
    pub(crate) revision: u64,
}

#[derive(Debug, Default)]
pub(crate) struct SavePlan {
    pub(crate) immediate: Vec<SaveItem>,
    pub(crate) waiting: usize,
    pub(crate) held: usize,
    pub(crate) outputs: Vec<OutputEdit>,
    pub(crate) grouped: bool,
}

impl SessionState {
    fn busy(&self, in_use: &HashSet<PathBuf>) -> HashSet<PathBuf> {
        in_use
            .iter()
            .chain(&self.reserved)
            .chain(&self.removing)
            .chain(&self.writing)
            .cloned()
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
        if self.save_in_progress {
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
        let tags = &self.tags;
        let outputs = match self.exports.is_empty() {
            true => Vec::new(),
            false => self
                .exports
                .edits(|path| tags.pending(path).map(|pending| pending.patch.clone())),
        };
        if items.is_empty() && outputs.is_empty() {
            self.status = Some(if grouped {
                MetadataStatus::GroupedEditsKept
            } else {
                MetadataStatus::NoPendingChanges
            });
            return None;
        }

        let mut plan = SavePlan {
            outputs,
            grouped,
            ..SavePlan::default()
        };
        for item in items {
            let being_written = self
                .deferred
                .iter()
                .any(|write| write.phase == DeferredPhase::Writing && write.item.path == item.path);
            if is_temporary(&item.path) {
                plan.held += 1;
            } else if in_use.contains(&item.path) || being_written {
                plan.waiting += 1;
                self.defer(item);
            } else {
                self.deferred.retain(|write| write.item.path != item.path);
                plan.immediate.push(item);
            }
        }
        self.save_in_progress = !plan.immediate.is_empty();
        self.writing
            .extend(plan.immediate.iter().map(|item| item.path.clone()));
        self.status = Some(MetadataStatus::PreparingSave);
        Some(plan)
    }

    fn defer(&mut self, item: SaveItem) {
        self.deferred
            .retain(|write| write.item.path != item.path || write.phase == DeferredPhase::Writing);
        self.deferred.push(DeferredWrite {
            item,
            phase: DeferredPhase::Waiting,
        });
    }

    pub(crate) fn finish_save(
        &mut self,
        epoch: u64,
        written: &[PathBuf],
        saved: &[SaveItem],
        status: MetadataStatus,
    ) {
        for path in written {
            if let Some(index) = self.writing.iter().position(|writing| writing == path) {
                self.writing.swap_remove(index);
            }
        }
        if epoch != self.epoch {
            return;
        }
        let owner = self.cover_owner();
        for item in saved {
            self.exports.acknowledge(&item.path, &item.patch);
            let cover_was_submitted = item.patch.cover_art.is_some();
            if cover_was_submitted {
                self.cover_written(&item.path);
            }
            self.tags
                .commit_saved(&item.path, &item.patch, item.revision);
            let cover_still_pending = self
                .tags
                .pending(&item.path)
                .is_some_and(|pending| pending.patch.cover_art.is_some());
            if cover_was_submitted && !cover_still_pending && owner.as_ref() == Some(&item.path) {
                self.cover.custom = false;
                self.cover.removal_requested = false;
            }
        }
        self.save_in_progress = false;
        self.status = Some(status);
    }

    pub(crate) fn take_ready_deferred(&mut self, in_use: &HashSet<PathBuf>) -> Vec<SaveItem> {
        let in_use = &self.busy(in_use);
        let writing: HashSet<PathBuf> = self
            .deferred
            .iter()
            .filter(|write| write.phase == DeferredPhase::Writing)
            .map(|write| write.item.path.clone())
            .collect();
        self.deferred
            .iter_mut()
            .filter(|write| {
                write.phase == DeferredPhase::Waiting
                    && !in_use.contains(&write.item.path)
                    && !writing.contains(&write.item.path)
            })
            .map(|write| {
                write.phase = DeferredPhase::Writing;
                write.item.clone()
            })
            .collect()
    }

    pub(crate) fn has_waiting_writes(&self) -> bool {
        self.deferred
            .iter()
            .any(|write| write.phase == DeferredPhase::Waiting)
    }

    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .deferred
            .iter()
            .filter(|write| write.phase != DeferredPhase::Failed)
            .map(|write| write.item.path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    pub(crate) fn finish_deferred(&mut self, results: &[(SaveItem, bool)]) {
        let loaded = self.working_set.source_paths();
        let mut written_count = 0;
        for (item, written) in results {
            for write in &mut self.deferred {
                if write.item.path != item.path {
                    continue;
                }
                if write.phase == DeferredPhase::Writing
                    && write.item.revision == item.revision
                    && !written
                {
                    write.phase = DeferredPhase::Failed;
                } else if write.phase == DeferredPhase::Waiting && !written {
                    let mut patch = item.patch.clone();
                    patch.merge(&write.item.patch);
                    write.item.patch = patch;
                }
            }
            if *written {
                self.deferred.retain(|write| {
                    write.item.path != item.path || write.item.revision != item.revision
                });
            }
            if *written {
                self.exports.acknowledge(&item.path, &item.patch);
                if item.patch.cover_art.is_some() {
                    self.cover_written(&item.path);
                }
            }
            if !loaded.contains(&item.path) {
            } else if *written {
                self.tags
                    .commit_saved(&item.path, &item.patch, item.revision);
            }
            written_count += usize::from(*written);
        }
        self.restore_failed_writes();
        let shown = results
            .iter()
            .any(|(item, _)| self.bound.iter().any(|title| title.path == item.path));
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
        for write in self.deferred.iter().filter(|write| {
            write.phase == DeferredPhase::Failed && loaded.contains(&write.item.path)
        }) {
            let pending = self.tags.pending(&write.item.path);
            let mut patch = write.item.patch.clone();
            if let Some(pending) = pending {
                patch.merge(&pending.patch);
            }
            if pending.is_none_or(|pending| pending.patch != patch) {
                self.tags.stage(&write.item.path, &patch);
            }
        }
    }

    pub(crate) fn removable_staged(&self, in_use: &HashSet<PathBuf>, now: Instant) -> Vec<String> {
        let writing = self.save_in_progress
            || !self.writing.is_empty()
            || self
                .deferred
                .iter()
                .any(|write| write.phase == DeferredPhase::Writing);
        if writing || self.staged.is_empty() {
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
            self.removing.extend(self.staged.paths(job_id));
        }
        jobs
    }
}
