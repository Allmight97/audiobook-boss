use std::path::{Path, PathBuf};

use serde::Serialize;

use super::audio::{AudioDefaults, AudioRefusal, AudioSnapshot};
use super::exports::{Exports, OutputEdits};
use super::lookup::LookupSnapshot;
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
mod default_edits;
mod lookup;
mod save;
mod snapshots;
mod submitting;
mod title_audio;
mod write_queue;
pub(crate) use bound_form::GateBlock;
use bound_form::{BoundTitle, StageOutcome};
use cover::{Cover, SourceCovers};
pub(crate) use default_edits::{Claim, DefaultEdits};
use lookup::LookupState;
pub(crate) use lookup::{ApplyStart, ChosenResult, NextTitle, QueuedTitle};
pub(crate) use save::SavePlan;
use snapshots::Parts;
use submitting::Submitting;
use write_queue::WriteQueue;
pub(crate) use write_queue::{SaveItem, WriteOutcome};

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
    /// The shown form is not the tags of `label` or of `more` other selected
    /// titles, whose tags could not be read.
    TagReadFailed {
        label: String,
        more: usize,
    },
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
    NotAnImageUrl,
    LoadedFromUrl,
    NoTitleForCover,
    LoadFailed { error: AppErrorEnvelope },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CoverSnapshot {
    /// The address of the displayed image; absent when there is none.
    pub image: Option<String>,
    /// The user replaced the cover and has not saved or staged it yet.
    pub custom: bool,
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
    /// The album sort processing would write: the sort-album atom in the MP4
    /// family.
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

pub(crate) struct SessionState {
    pub(crate) epoch: u64,
    pub(crate) binding: u64,
    pub(crate) working_set: WorkingSet,
    pub(crate) tags: TagCache,
    pub(crate) lookup: LookupState,
    pub(crate) audio: AudioDefaults,
    pub(crate) output: OutputPlan,
    pub(crate) edits: DefaultEdits,
    pub(crate) sweeping: Claim,
    pub(crate) deferred_writer: Claim,
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
    cover: Cover,
    writes: WriteQueue,
    status: Option<MetadataStatus>,
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
            edits: DefaultEdits::default(),
            sweeping: Claim::default(),
            deferred_writer: Claim::default(),
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
            cover: Cover::default(),
            writes: WriteQueue::default(),
            status: None,
            parts,
        }
    }
}

impl SessionState {
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
