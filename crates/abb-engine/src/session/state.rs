use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use super::audio::{AudioDefaults, AudioRefusal, AudioSnapshot};
use super::exports::{Exports, OutputEdit, OutputEdits};
use super::lookup::{LookupSnapshot, LookupState};
use super::metadata_form::{MetadataForm, MetadataFormSnapshot};
use super::output::{OutputPlan, OutputSnapshot};
use super::plans::Plans;
use super::staged::StagedSources;
use super::submission::SubmissionStatus;
use super::tag_cache::TagCache;
use super::working_set::{SelectionSnapshot, TitlesSnapshot, WorkingSet};
use crate::errors::AppErrorEnvelope;
use crate::metadata::{AudiobookMetadata, MetadataIntentPatch};

mod bound_form;
mod cover;
mod snapshots;
mod submitting;
mod title_audio;
pub(crate) use bound_form::GateBlock;
use bound_form::{BoundTitle, StageOutcome};
use cover::{Cover, SourceCovers};
use snapshots::Parts;
use submitting::Submitting;

/// Why the last metadata action ended the way it did. Hosts word these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MetadataStatus {
    /// The edits on screen were not accepted, so the selection did not change.
    DraftInvalid {
        message: String,
    },
    SaveAlreadyInProgress,
    PreparingSave,
    SaveInvalid,
    NoPendingChanges,
    /// Only grouped titles have edits; those are written with their output.
    GroupedEditsKept,
    #[serde(rename_all = "camelCase")]
    SaveComplete {
        succeeded: usize,
        failed: usize,
        cancelled: usize,
        /// Local sources an export is still reading; written when it finishes.
        waiting: usize,
        /// Temporary downloads; never written, their edits go with exports.
        held: usize,
        outputs: OutputEdits,
    },
    SaveCancelled,
    SaveFailed {
        error: AppErrorEnvelope,
    },
    /// Saves that waited for an export have run. A failed write keeps its
    /// edit pending on a title still in the list, so Save retries it.
    DeferredWritesFinished {
        written: usize,
        failed: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CoverNotice {
    UrlRequired,
    LoadedFromUrl,
    LoadFailed { error: AppErrorEnvelope },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CoverSnapshot {
    /// The address of the displayed image; absent when there is none.
    pub image: Option<String>,
    /// The user replaced the cover and has not saved or staged it yet.
    pub custom: bool,
    pub removal_requested: bool,
    pub loading: bool,
    pub notice: Option<CoverNotice>,
    /// Advances with every notice, so a repeated notice is still new.
    pub notice_serial: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSnapshot {
    pub revision: u64,
    /// Advances whenever the form binds to a different selection, so a host
    /// can tell which form its unconfirmed typing belongs to.
    pub binding: u64,
    pub form: MetadataFormSnapshot,
    pub cover: CoverSnapshot,
    /// The tags Save or processing would write for the values on screen.
    pub tags: TagPreview,
    pub save_in_progress: bool,
    pub status: Option<MetadataStatus>,
    pub has_pending_edits: bool,
    /// Files with a Save waiting for the exports reading them to finish.
    pub waiting_writes: Vec<PathBuf>,
}

/// The tags the values on screen become. Title is also the album; author is
/// also the album artist.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TagPreview {
    pub title: String,
    pub album: String,
    pub artist: String,
    pub album_artist: String,
    pub composer: String,
    pub series: String,
    pub series_part: String,
    pub subseries: String,
    pub subseries_part: String,
    /// The album sort processing would write: the TSOA frame in MP3, the
    /// sort-album atom in the MP4 family.
    pub album_sort: String,
    pub year: String,
    pub genre: String,
}

/// Everything that changed since a revision. A part is present only when it
/// changed; each part carries the revision of its own last change, so a host
/// keeps whichever copy of a part is newest.
#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
    pub revision: u64,
    pub titles: Option<TitlesSnapshot>,
    pub selection: Option<SelectionSnapshot>,
    pub metadata: Option<MetadataSnapshot>,
    pub lookup: Option<LookupSnapshot>,
    pub audio: Option<AudioSnapshot>,
    pub output: Option<OutputSnapshot>,
    pub remote: Option<crate::remote_source::RemoteUiSnapshot>,
    pub remote_library: Option<crate::remote_source::RemoteLibrarySnapshot>,
    pub remote_acquisition: Option<crate::remote_source::RemoteAcquisitionSnapshot>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredPhase {
    Waiting,
    Writing,
    Failed,
}

#[derive(Debug)]
struct DeferredWrite {
    item: SaveItem,
    phase: DeferredPhase,
}

pub(crate) struct SessionState {
    pub(crate) epoch: u64,
    pub(crate) binding: u64,
    pub(crate) working_set: WorkingSet,
    pub(crate) tags: TagCache,
    pub(crate) lookup: LookupState,
    pub(crate) audio: AudioDefaults,
    pub(crate) output: OutputPlan,
    pub(crate) preview: super::preview::Preview,
    plans: Plans,
    /// The title and audio-request changes plans were last refreshed for.
    plans_seen: (u64, u64),
    submission: Option<SubmissionStatus>,
    pending_review: Option<super::submission::PendingReview>,
    review_serial: u64,
    submitting: Option<Submitting>,
    audio_refusal: Option<AudioRefusal>,
    source_covers: SourceCovers,
    keeping_locations: usize,
    reserved: Vec<PathBuf>,
    pub(crate) staged: StagedSources,
    pub(super) removing: Vec<PathBuf>,
    pub(crate) exports: Exports,
    form: MetadataForm,
    bound: Vec<BoundTitle>,
    selection_key: Vec<PathBuf>,
    cover: Cover,
    save_in_progress: bool,
    writing: Vec<PathBuf>,
    status: Option<MetadataStatus>,
    deferred: Vec<DeferredWrite>,
    parts: Parts,
}

impl Default for SessionState {
    fn default() -> Self {
        let working_set = WorkingSet::default();
        let lookup = LookupState::default();
        let form = MetadataForm::single(&AudiobookMetadata::default());
        let parts = Parts::initial(&working_set, &form, &lookup);
        Self {
            epoch: 0,
            binding: 0,
            working_set,
            tags: TagCache::default(),
            lookup,
            audio: AudioDefaults::default(),
            output: OutputPlan::default(),
            preview: super::preview::Preview::default(),
            plans: Plans::default(),
            plans_seen: (u64::MAX, u64::MAX),
            submission: None,
            pending_review: None,
            review_serial: 0,
            submitting: None,
            audio_refusal: None,
            source_covers: SourceCovers::default(),
            keeping_locations: 0,
            reserved: Vec::new(),
            staged: StagedSources::default(),
            removing: Vec::new(),
            exports: Exports::default(),
            form,
            bound: Vec::new(),
            selection_key: Vec::new(),
            cover: Cover::default(),
            save_in_progress: false,
            writing: Vec::new(),
            status: None,
            deferred: Vec::new(),
            parts,
        }
    }
}

impl SessionState {
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

    pub(crate) fn reset(&mut self) {
        self.cancel_review();
        self.working_set.reset();
        self.audio_refusal = None;
        if self.submitting.is_some() {
            self.working_set.set_order_locked(true);
        }
        self.lookup = LookupState {
            request: self.lookup.request + 1,
            ..LookupState::default()
        };
        self.reset_metadata();
    }

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

    fn restore_failed_writes(&mut self) {
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

    pub(crate) fn pending_intents(&self, paths: &[PathBuf]) -> Vec<(PathBuf, MetadataIntentPatch)> {
        paths
            .iter()
            .filter_map(|path| {
                self.tags
                    .pending(path)
                    .filter(|pending| pending.patch.is_actionable())
                    .map(|pending| (path.clone(), pending.patch.clone()))
            })
            .collect()
    }

    pub(crate) fn known_tags(&self, path: &Path) -> Option<AudiobookMetadata> {
        self.tags.effective(path)
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
