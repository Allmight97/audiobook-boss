use std::path::{Path, PathBuf};

use serde::Serialize;

use super::audio::{AudioDefaults, AudioRefusal, AudioSnapshot};
use super::companions::Companions;
use super::exports::{Exports, OutputEdits};
use super::lookup::LookupSnapshot;
use super::metadata_form::{MetadataForm, MetadataFormSnapshot};
use super::output::{OutputPlan, OutputSnapshot};
use super::plans::Plans;
use super::staged::StagedSources;
use super::submission::SubmissionStatus;
use super::tag_cache::TagCache;
use super::working_set::{SelectionSnapshot, TitlesSnapshot, WorkingSet};
use abb_media_pipeline::errors::AppErrorEnvelope;
use abb_media_pipeline::metadata::{AudiobookMetadata, MetadataIntentPatch};

mod bound_form;
mod cover;
mod default_edits;
mod import;
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
pub(crate) use lookup::{ApplyStart, ChosenResult, LookupState, NextTitle, QueuedTitle};
pub(crate) use save::{SavePlan, WriteBatch};
use snapshots::Parts;
use submitting::Submission;
use write_queue::WriteQueue;
pub(crate) use write_queue::{SaveItem, WriteOutcome};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MetadataStatus {
    DraftInvalid {
        message: String,
    },
    SaveAlreadyInProgress,
    PreparingSave,
    SaveInvalid,
    NoPendingChanges,
    GroupedEditsKept,
    #[serde(rename_all = "camelCase")]
    SaveComplete {
        #[specta(type = specta_typescript::Number)]
        succeeded: usize,
        #[specta(type = specta_typescript::Number)]
        failed: usize,
        #[specta(type = specta_typescript::Number)]
        cancelled: usize,
        #[specta(type = specta_typescript::Number)]
        waiting: usize,
        #[specta(type = specta_typescript::Number)]
        held: usize,
        outputs: OutputEdits,
    },
    TagReadFailed {
        label: String,
        #[specta(type = specta_typescript::Number)]
        more: usize,
    },
    SaveFailed {
        error: AppErrorEnvelope,
    },
    DeferredWritesFinished {
        #[specta(type = specta_typescript::Number)]
        written: usize,
        #[specta(type = specta_typescript::Number)]
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
    pub image: Option<String>,
    pub custom: bool,
    pub loading: bool,
    pub notice: Option<CoverNotice>,
    #[specta(type = specta_typescript::Number)]
    pub notice_serial: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSnapshot {
    #[specta(type = specta_typescript::Number)]
    pub revision: u64,
    #[specta(type = specta_typescript::Number)]
    pub binding: u64,
    pub form: MetadataFormSnapshot,
    pub cover: CoverSnapshot,
    pub tags: TagPreview,
    pub save_in_progress: bool,
    pub status: Option<MetadataStatus>,
    pub has_pending_edits: bool,
}

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
    pub album_sort: String,
    pub year: String,
    pub genre: String,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
    #[specta(type = specta_typescript::Number)]
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
    plans_seen: (u64, u64),
    submission: Option<SubmissionStatus>,
    review_serial: u64,
    submitting: Submission,
    audio_refusal: Option<AudioRefusal>,
    source_covers: SourceCovers,
    keeping_locations: usize,
    pub(crate) staged: StagedSources,
    pub(crate) companions: Companions,
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
            review_serial: 0,
            submitting: Submission::Idle,
            audio_refusal: None,
            source_covers: SourceCovers::default(),
            keeping_locations: 0,
            staged: StagedSources::default(),
            companions: Companions::default(),
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
