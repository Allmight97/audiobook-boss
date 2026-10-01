//! The working session's state and every transition that needs no I/O.
//!
//! The runtime holds this behind one lock and never across an await, so each
//! method here is atomic. Anything that must read or write a file returns to
//! the runtime as data (`ReadTicket`s, save plans) and comes back as a
//! completion (`finish_reads`, `finish_save`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::audio::{AudioDefaults, AudioSnapshot, TitleAudio};
use super::audio_choice::AudioEdit;
use super::lookup::{LookupSnapshot, LookupState, QueuedTitle};
use super::metadata_form::{MetadataField, MetadataForm, MetadataFormSnapshot};
use super::output::{OutputPlan, OutputPreview, OutputSnapshot};
use super::plans::{estimate_size, PlanInput, PlanTicket, Plans};
use super::staged::StagedSources;
use super::submission::{
    build_draft, title_label, Draft, DraftInputs, SubmissionStatus, SubmitRefusal, SubmittedTitle,
};
use super::tag_cache::{ReadTicket, TagCache};
use super::working_set::{SelectionSnapshot, TitlesSnapshot, WorkingSet};
use crate::audio::AudioFile;
use crate::errors::{AppError, AppErrorEnvelope};
use crate::metadata::{
    processing_album_sort, validate_metadata_intent_patch, AudiobookMetadata, MetadataIntentPatch,
    PatchOp,
};

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
        /// Temporary downloads an export is reading; never written.
        held: usize,
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
    /// Advances whenever the displayed image changes; fetch the bytes with
    /// the session's cover query.
    pub image_revision: u64,
    pub present: bool,
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
    /// The album sort (TSOA) processing would write.
    pub album_sort: String,
    pub year: String,
    pub genre: String,
}

/// Everything that changed since a revision. A part is present only when it
/// changed; each part carries the revision of its own last change, so a host
/// keeps whichever copy of a part is newest.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
    pub revision: u64,
    pub titles: Option<TitlesSnapshot>,
    pub selection: Option<SelectionSnapshot>,
    pub metadata: Option<MetadataSnapshot>,
    pub lookup: Option<LookupSnapshot>,
    pub audio: Option<AudioSnapshot>,
    pub output: Option<OutputSnapshot>,
}

/// Why the metadata draft gate refused a change of selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GateBlock {
    SaveInProgress,
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StageOutcome {
    /// Edits were staged, or there were none.
    Staged,
    /// There are edits and no valid title to carry them.
    NoTarget,
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundTitle {
    path: PathBuf,
    input_id: String,
    is_valid: bool,
}

/// One file's edit on its way to disk.
#[derive(Debug, Clone)]
pub(crate) struct SaveItem {
    pub(crate) path: PathBuf,
    pub(crate) patch: MetadataIntentPatch,
    pub(crate) revision: u64,
}

/// What a Save decided to do with each pending edit.
#[derive(Debug, Default)]
pub(crate) struct SavePlan {
    /// No export is reading these; write them now.
    pub(crate) immediate: Vec<SaveItem>,
    /// Local sources in flight; written when their exports finish reading.
    pub(crate) waiting: usize,
    /// Temporary sources in flight; not written.
    pub(crate) held: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredPhase {
    Waiting,
    /// Taken by the deferred writer; the file is being written now.
    Writing,
}

#[derive(Debug)]
struct DeferredWrite {
    item: SaveItem,
    phase: DeferredPhase,
}

#[derive(Debug, Default)]
struct Cover {
    custom: bool,
    removal_requested: bool,
    loading: bool,
    notice: Option<CoverNotice>,
    notice_serial: u64,
    displayed: Option<Vec<u8>>,
    image_revision: u64,
    /// Advances with every cover choice or Clear; a load started before the
    /// latest one is dropped when it finishes.
    request: u64,
}

struct Parts {
    revision: u64,
    titles: TitlesSnapshot,
    titles_changes: u64,
    selection: SelectionSnapshot,
    metadata: MetadataSnapshot,
    lookup: LookupSnapshot,
    audio: AudioSnapshot,
    output: OutputSnapshot,
}

pub(crate) struct SessionState {
    /// Advances on reset; completions from before a reset are dropped.
    pub(crate) epoch: u64,
    /// Advances whenever the form binds to a different selection.
    pub(crate) binding: u64,
    pub(crate) working_set: WorkingSet,
    pub(crate) tags: TagCache,
    pub(crate) lookup: LookupState,
    pub(crate) audio: AudioDefaults,
    pub(crate) output: OutputPlan,
    plans: Plans,
    /// The title and audio-request changes plans were last refreshed for.
    plans_seen: (u64, u64),
    /// How the latest submission or preview is going.
    submission: Option<SubmissionStatus>,
    /// A submission waiting for the user's collision choice.
    pending_review: Option<Draft>,
    /// A submission or preview is between `begin_submission` and
    /// `finish_submission`. Kept apart from `submission`, which a refusal of
    /// a later request overwrites.
    submitting: bool,
    /// Sources a submission being prepared will read; Save treats them as busy.
    reserved: Vec<PathBuf>,
    /// Downloads the session imported, and when they may be removed.
    pub(crate) staged: StagedSources,
    /// Files of downloads being removed: nothing may write or submit them.
    removing: Vec<PathBuf>,
    /// A title left the list, so a download may now be removable.
    staged_released: bool,
    form: MetadataForm,
    bound: Vec<BoundTitle>,
    selection_key: Vec<PathBuf>,
    cover: Cover,
    save_in_progress: bool,
    /// Files a Save is writing right now. Kept through a Reset, which
    /// forgets the Save's form but cannot stop its write.
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
        let parts = Parts {
            revision: 0,
            titles: working_set.titles(0),
            titles_changes: working_set.titles_changes(),
            selection: working_set.selection(0),
            metadata: MetadataSnapshot {
                revision: 0,
                binding: 0,
                form: form.snapshot(),
                cover: CoverSnapshot {
                    image_revision: 0,
                    present: false,
                    custom: false,
                    removal_requested: false,
                    loading: false,
                    notice: None,
                    notice_serial: 0,
                },
                tags: TagPreview::default(),
                save_in_progress: false,
                status: None,
                has_pending_edits: false,
                waiting_writes: Vec::new(),
            },
            lookup: lookup.snapshot(0),
            audio: AudioSnapshot {
                revision: 0,
                capabilities: None,
                defaults: AudioDefaults::default().defaults_view(),
                titles: Default::default(),
            },
            output: OutputPlan::default().snapshot(0, OutputPreview::NoDirectory, None),
        };
        Self {
            epoch: 0,
            binding: 0,
            working_set,
            tags: TagCache::default(),
            lookup,
            audio: AudioDefaults::default(),
            output: OutputPlan::default(),
            plans: Plans::default(),
            plans_seen: (u64::MAX, u64::MAX),
            submission: None,
            pending_review: None,
            submitting: false,
            reserved: Vec::new(),
            staged: StagedSources::default(),
            removing: Vec::new(),
            staged_released: false,
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
    // ---- Snapshots ----

    pub(crate) fn revision(&self) -> u64 {
        self.parts.revision
    }

    /// Re-derives what hosts see and stamps each changed part with a new
    /// revision. The runtime calls this once after every transition.
    pub(crate) fn settle(&mut self) {
        self.refresh_displayed_cover();
        let next = self.parts.revision + 1;
        let mut changed = false;

        let listed = self.working_set.source_ids();
        if self.staged.finish_unlisted(&listed) {
            self.staged_released = true;
        }
        let companions = self.staged.companions();
        if self.working_set.titles_changes() != self.parts.titles_changes
            || companions != self.parts.titles.companions
        {
            self.parts.titles_changes = self.working_set.titles_changes();
            self.parts.titles = TitlesSnapshot {
                companions,
                ..self.working_set.titles(next)
            };
            changed = true;
        }
        let selection = self.working_set.selection(self.parts.selection.revision);
        if selection != self.parts.selection {
            self.parts.selection = SelectionSnapshot {
                revision: next,
                ..selection
            };
            changed = true;
        }
        let metadata = self.metadata_snapshot(self.parts.metadata.revision);
        if metadata != self.parts.metadata {
            self.parts.metadata = MetadataSnapshot {
                revision: next,
                ..metadata
            };
            changed = true;
        }
        let lookup = self.lookup.snapshot(self.parts.lookup.revision);
        if lookup != self.parts.lookup {
            self.parts.lookup = LookupSnapshot {
                revision: next,
                ..lookup
            };
            changed = true;
        }
        let audio = self.audio_snapshot(self.parts.audio.revision);
        if audio != self.parts.audio {
            self.parts.audio = AudioSnapshot {
                revision: next,
                ..audio
            };
            changed = true;
        }
        let output = self.output_snapshot(self.parts.output.revision);
        if output != self.parts.output {
            self.parts.output = OutputSnapshot {
                revision: next,
                ..output
            };
            changed = true;
        }
        if changed {
            self.parts.revision = next;
        }
    }

    fn audio_snapshot(&self, revision: u64) -> AudioSnapshot {
        let titles = self
            .working_set
            .files()
            .iter()
            .filter_map(|file| {
                let id = &file.input_id;
                let request = self.working_set.audio_request(id)?;
                let view = self.audio.title_view(request);
                let plan = self.plans.plan(id);
                let estimate = estimate_size(
                    request,
                    &plan,
                    self.working_set.sources_for(file),
                    view.facts.estimate_kbps,
                );
                Some((
                    id.clone(),
                    TitleAudio {
                        choice: view.choice,
                        facts: view.facts,
                        request: view.request,
                        plan,
                        estimate,
                    },
                ))
            })
            .collect();
        AudioSnapshot {
            revision,
            capabilities: self.audio.capabilities().cloned(),
            defaults: self.audio.defaults_view(),
            titles,
        }
    }

    /// The title the output preview names: the first selected one, or the
    /// first valid one.
    fn preview_title(&self) -> Option<&AudioFile> {
        let files = self.working_set.files();
        self.working_set
            .selected_indices()
            .iter()
            .min()
            .and_then(|index| files.get(*index))
            .or_else(|| files.iter().find(|file| file.is_valid))
    }

    fn output_snapshot(&self, revision: u64) -> OutputSnapshot {
        let value = |field| {
            let value = self.form.trimmed(field);
            (!value.is_empty()).then(|| value.to_string())
        };
        let metadata = AudiobookMetadata {
            title: value(MetadataField::Title),
            artist: value(MetadataField::Author),
            composer: value(MetadataField::Narrator),
            date: value(MetadataField::Date),
            series: value(MetadataField::Series),
            series_part: value(MetadataField::SeriesPart),
            subseries: value(MetadataField::Subseries),
            subseries_part: value(MetadataField::SubseriesPart),
            ..Default::default()
        };
        let title = self.preview_title().map(|file| {
            let format = self
                .working_set
                .audio_request(&file.input_id)
                .map_or_else(|| self.audio.request().format, |request| request.format);
            (&metadata, file.path.as_path(), format)
        });
        let preview = self.output.preview(title);
        self.output
            .snapshot(revision, preview, self.submission.clone())
    }

    // ---- Submission ----

    /// Starts a submission or preview: accepts the edits on screen, builds
    /// what will be sent, reserves its sources, and locks the list.
    pub(crate) fn begin_submission(&mut self, preview_seconds: Option<f64>) -> Option<Draft> {
        match self.prepare_submission(preview_seconds) {
            Ok(draft) => {
                self.submitting = true;
                self.reserved.extend(draft.sources.iter().cloned());
                self.working_set.set_order_locked(true);
                self.submission = Some(SubmissionStatus::Preparing {
                    preview: draft.preview(),
                });
                Some(draft)
            }
            Err(reason) => {
                self.submission = Some(SubmissionStatus::Refused { reason });
                None
            }
        }
    }

    fn prepare_submission(&mut self, preview_seconds: Option<f64>) -> Result<Draft, SubmitRefusal> {
        if self.submitting {
            return Err(SubmitRefusal::Busy);
        }
        let writing = !self.writing.is_empty()
            || self
                .deferred
                .iter()
                .any(|write| write.phase == DeferredPhase::Writing);
        if self.save_in_progress || writing {
            return Err(SubmitRefusal::SaveInProgress);
        }
        if !self.working_set.files().is_empty() {
            match self.stage_bound_form() {
                StageOutcome::Staged => {}
                StageOutcome::NoTarget => return Err(SubmitRefusal::NoTarget),
                StageOutcome::Invalid(message) => {
                    return Err(SubmitRefusal::DraftInvalid { message })
                }
            }
        }
        let required = self.working_set.audio_choice_required();
        let titles: Vec<SubmittedTitle<'_>> = self
            .working_set
            .files()
            .iter()
            .map(|file| SubmittedTitle {
                anchor: file,
                sources: self.working_set.sources_for(file),
                request: self.working_set.audio_request(&file.input_id).map_or_else(
                    || self.audio.request(),
                    |request| self.audio.title_view(request).request,
                ),
                choice_required: required.contains(&file.input_id),
            })
            .collect();
        let supplemental_assets = self.staged.assets_for(
            titles
                .iter()
                .filter(|title| title.anchor.is_valid)
                .flat_map(|title| title.sources)
                .map(|source| source.input_id.as_str()),
        );
        let draft = build_draft(
            &titles,
            DraftInputs {
                output_directory: self.output.naming_directory(),
                naming: self.output.naming(),
                supplemental_assets,
                preview_seconds,
            },
            |paths| {
                let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
                self.pending_intents(&paths)
                    .into_iter()
                    .map(|(path, patch)| (path.to_string_lossy().into_owned(), patch))
                    .collect()
            },
            title_label,
        )?;
        if draft
            .sources
            .iter()
            .any(|source| self.removing.contains(source))
        {
            return Err(SubmitRefusal::SourceRemoved);
        }
        Ok(draft)
    }

    // ---- Staged downloads ----

    /// Whether a title left the list since this was last asked.
    pub(crate) fn take_staged_released(&mut self) -> bool {
        std::mem::take(&mut self.staged_released)
    }

    /// Picks the downloads to remove now and holds their files so nothing
    /// writes or submits them meanwhile. `in_use` is every source of an
    /// unfinished export. Nothing is removed while a Save writes.
    pub(crate) fn begin_staged_removal(&mut self, in_use: &HashSet<PathBuf>) -> Vec<String> {
        let writing = self.save_in_progress
            || !self.writing.is_empty()
            || self
                .deferred
                .iter()
                .any(|write| write.phase == DeferredPhase::Writing);
        if writing {
            return Vec::new();
        }
        let busy = self.busy(in_use);
        let jobs = self.staged.removable(&busy);
        for job_id in &jobs {
            self.removing.extend(self.staged.paths(job_id));
        }
        jobs
    }

    /// Ends a removal; a failed one stays registered for the next attempt.
    pub(crate) fn finish_staged_removal(&mut self, job_id: &str, removed: bool) {
        let paths = self.staged.paths(job_id);
        self.removing.retain(|path| !paths.contains(path));
        if removed {
            self.staged.removed(job_id);
        }
    }

    /// Holds a submission for the user's collision choice.
    pub(crate) fn await_review(
        &mut self,
        mut draft: Draft,
        outputs: Vec<crate::output_artifact::PlannedOutput>,
    ) {
        draft.payload.collision_policy = None;
        draft.reviewed = Some(super::submission::collisions(&outputs));
        self.submission = Some(SubmissionStatus::ReviewRequired {
            outputs,
            preview: draft.preview(),
        });
        self.pending_review = Some(draft);
    }

    pub(crate) fn refuse_submission(&mut self, reason: SubmitRefusal) {
        self.submission = Some(SubmissionStatus::Refused { reason });
    }

    /// Drops a submission held for review.
    pub(crate) fn cancel_review(&mut self) {
        if let Some(draft) = self.pending_review.take() {
            self.finish_submission(&draft, SubmissionStatus::Cancelled);
        }
    }

    /// The submission waiting for a collision choice, if any.
    pub(crate) fn take_review(&mut self) -> Option<Draft> {
        let draft = self.pending_review.take()?;
        self.submission = Some(SubmissionStatus::Preparing {
            preview: draft.preview(),
        });
        Some(draft)
    }

    pub(crate) fn start_preview(&mut self) {
        self.submission = Some(SubmissionStatus::Previewing);
    }

    /// Ends a submission: frees its sources and the list, and records how it
    /// ended.
    pub(crate) fn finish_submission(&mut self, draft: &Draft, status: SubmissionStatus) {
        for source in &draft.sources {
            if let Some(index) = self.reserved.iter().position(|reserved| reserved == source) {
                self.reserved.swap_remove(index);
            }
        }
        if self.reserved.is_empty() {
            self.working_set.set_order_locked(false);
        }
        self.submitting = false;
        self.submission = Some(status);
    }

    // ---- Title plans ----

    /// The plans to resolve because a title's request or sources changed.
    pub(crate) fn take_plan_tickets(&mut self) -> Vec<PlanTicket> {
        let seen = (
            self.working_set.titles_changes(),
            self.working_set.audio_changes(),
        );
        if seen == self.plans_seen {
            return Vec::new();
        }
        self.plans_seen = seen;
        let required = self.working_set.audio_choice_required();
        let inputs: Vec<PlanInput<'_>> = self
            .working_set
            .files()
            .iter()
            .filter_map(|file| {
                let request = self.working_set.audio_request(&file.input_id)?;
                Some(PlanInput {
                    title_id: &file.input_id,
                    request,
                    sources: self.working_set.sources_for(file).to_vec(),
                    choice_required: required.contains(&file.input_id),
                })
            })
            .collect();
        self.plans.refresh(inputs)
    }

    pub(crate) fn finish_plan(
        &mut self,
        ticket: &PlanTicket,
        result: Result<crate::audio::TitleAudioPlan, String>,
    ) {
        self.plans.finish(ticket, result);
    }

    /// The parts that changed after `revision`; everything for `None`.
    pub(crate) fn update_since(&self, revision: Option<u64>) -> SessionUpdate {
        let newer = |part: u64| revision.is_none_or(|revision| part > revision);
        SessionUpdate {
            revision: self.parts.revision,
            titles: newer(self.parts.titles.revision).then(|| self.parts.titles.clone()),
            selection: newer(self.parts.selection.revision).then(|| self.parts.selection.clone()),
            metadata: newer(self.parts.metadata.revision).then(|| self.parts.metadata.clone()),
            lookup: newer(self.parts.lookup.revision).then(|| self.parts.lookup.clone()),
            audio: newer(self.parts.audio.revision).then(|| self.parts.audio.clone()),
            output: newer(self.parts.output.revision).then(|| self.parts.output.clone()),
        }
    }

    fn metadata_snapshot(&self, revision: u64) -> MetadataSnapshot {
        MetadataSnapshot {
            revision,
            binding: self.binding,
            form: self.form.snapshot(),
            cover: CoverSnapshot {
                image_revision: self.cover.image_revision,
                present: self.cover.displayed.is_some(),
                custom: self.cover.custom,
                removal_requested: self.cover.removal_requested,
                loading: self.cover.loading,
                notice: self.cover.notice.clone(),
                notice_serial: self.cover.notice_serial,
            },
            tags: self.tag_preview(),
            save_in_progress: self.save_in_progress,
            status: self.status.clone(),
            has_pending_edits: self.tags.has_pending(),
            waiting_writes: self.waiting_write_paths(),
        }
    }

    fn tag_preview(&self) -> TagPreview {
        let value = |field| self.form.trimmed(field).to_string();
        TagPreview {
            title: value(MetadataField::Title),
            album: value(MetadataField::Title),
            artist: value(MetadataField::Author),
            album_artist: value(MetadataField::Author),
            composer: value(MetadataField::Narrator),
            series: value(MetadataField::Series),
            series_part: value(MetadataField::SeriesPart),
            subseries: value(MetadataField::Subseries),
            subseries_part: value(MetadataField::SubseriesPart),
            album_sort: self.album_sort_preview().unwrap_or_default(),
            year: value(MetadataField::Date),
            genre: value(MetadataField::Genre),
        }
    }

    fn album_sort_preview(&self) -> Option<String> {
        let value = |field| {
            let value = self.form.trimmed(field);
            (!value.is_empty()).then(|| value.to_string())
        };
        let single = match self.bound.as_slice() {
            [title] => self.tags.effective(&title.path),
            _ => None,
        };
        processing_album_sort(&AudiobookMetadata {
            title: value(MetadataField::Title),
            series: value(MetadataField::Series),
            series_part: value(MetadataField::SeriesPart),
            album_sort: single.and_then(|metadata| metadata.album_sort),
            ..Default::default()
        })
    }

    // ---- Selection and the draft gate ----

    fn selected_titles(&self) -> Vec<BoundTitle> {
        self.working_set
            .selected_files()
            .into_iter()
            .map(|file| BoundTitle {
                path: file.path.clone(),
                input_id: file.input_id.clone(),
                is_valid: file.is_valid,
            })
            .collect()
    }

    /// Validates the edits on screen and stages them onto every valid bound
    /// title. Invalid inputs never receive intent.
    pub(crate) fn stage_bound_form(&mut self) -> StageOutcome {
        let patch = self.form.compose_intent();
        if !patch.is_actionable() {
            return StageOutcome::Staged;
        }
        let targets: Vec<&BoundTitle> = self.bound.iter().filter(|title| title.is_valid).collect();
        if targets.is_empty() {
            return StageOutcome::NoTarget;
        }
        let validation = validate_metadata_intent_patch(&patch);
        if let Some(error) = validation.field_errors.first() {
            return StageOutcome::Invalid(error.message.clone());
        }
        for target in targets {
            self.tags.stage(&target.path, &validation.metadata_patch);
        }
        // Staged values become the baseline Keep restores.
        self.form.reset_dirty();
        StageOutcome::Staged
    }

    /// The metadata draft gate: the edits on screen must be accepted before
    /// the selection, the grouping, or the set of titles may change.
    pub(crate) fn gate(&mut self) -> Result<(), GateBlock> {
        if self.save_in_progress {
            return Err(GateBlock::SaveInProgress);
        }
        if self.bound.is_empty() || !self.form.has_dirty_fields() {
            return Ok(());
        }
        match self.stage_bound_form() {
            StageOutcome::Staged => {
                self.cover.custom = false;
                self.cover.removal_requested = false;
                Ok(())
            }
            // Only invalid inputs are bound; they cannot carry edits.
            StageOutcome::NoTarget => Ok(()),
            StageOutcome::Invalid(message) => {
                self.status = Some(MetadataStatus::DraftInvalid {
                    message: message.clone(),
                });
                Err(GateBlock::Invalid(message))
            }
        }
    }

    /// Binds the form to the current selection after the titles or the
    /// selection changed, and returns the source reads that would complete
    /// what the session knows about it.
    pub(crate) fn rebind(&mut self) -> Vec<ReadTicket> {
        if self.working_set.files().is_empty() {
            self.reset_metadata();
            return Vec::new();
        }
        self.tags.retain_paths(&self.working_set.source_paths());

        let selected = self.selected_titles();
        let mut key: Vec<PathBuf> = selected.iter().map(|title| title.path.clone()).collect();
        key.sort();
        let mut tickets = Vec::new();
        if key != self.selection_key {
            self.binding += 1;
            self.selection_key = key;
            self.bound = selected;
            self.status = None;
            self.cover = Cover {
                displayed: self.cover.displayed.take(),
                image_revision: self.cover.image_revision,
                notice_serial: self.cover.notice_serial,
                ..Cover::default()
            };
            self.form = self.form_from_known_tags();
            let paths: Vec<PathBuf> = self
                .bound
                .iter()
                .filter(|title| title.is_valid)
                .map(|title| title.path.clone())
                .collect();
            tickets.extend(paths.iter().filter_map(|path| self.tags.begin_read(path)));
        }
        // The cover comes from the same read when the selection asked for it.
        if let Some(cover) = self.cover_read() {
            if !tickets.iter().any(|ticket| ticket.path == cover.path) {
                tickets.push(cover);
            }
        }
        tickets
    }

    fn form_from_known_tags(&self) -> MetadataForm {
        let known = |title: &BoundTitle| {
            title
                .is_valid
                .then(|| self.tags.effective(&title.path))
                .flatten()
                .unwrap_or_default()
        };
        match self.bound.as_slice() {
            [] => MetadataForm::single(&AudiobookMetadata::default()),
            [title] => MetadataForm::single(&known(title)),
            titles => {
                let metadata: Vec<AudiobookMetadata> = titles.iter().map(known).collect();
                MetadataForm::multi(&metadata, titles.len())
            }
        }
    }

    /// Applies finished source reads. Reads land in the cache even when the
    /// selection moved on; the form refreshes only if it is still bound to
    /// the selection that asked for them.
    pub(crate) fn finish_reads(
        &mut self,
        binding: u64,
        reads: Vec<(ReadTicket, Result<AudiobookMetadata, AppError>)>,
    ) {
        let cover_path = self.cover_read_target();
        for (ticket, result) in reads {
            match result {
                Ok(metadata) => {
                    self.tags.complete_read(&ticket, metadata);
                }
                Err(error) => {
                    log::warn!("Failed to read metadata: {error}");
                    if binding == self.binding && cover_path.as_ref() == Some(&ticket.path) {
                        self.set_cover_notice(CoverNotice::LoadFailed {
                            error: AppErrorEnvelope::from(&error),
                        });
                    }
                }
            }
        }
        if binding == self.binding {
            let fresh = self.form_from_known_tags();
            self.form.rehydrate(fresh);
        }
    }

    fn reset_metadata(&mut self) {
        self.epoch += 1;
        self.binding += 1;
        self.tags.clear();
        self.form = MetadataForm::single(&AudiobookMetadata::default());
        self.bound.clear();
        self.selection_key.clear();
        self.cover = Cover {
            displayed: self.cover.displayed.take(),
            image_revision: self.cover.image_revision,
            notice_serial: self.cover.notice_serial,
            ..Cover::default()
        };
        self.save_in_progress = false;
        self.status = None;
    }

    // ---- Audio ----

    /// Edits each named title's audio choice; refused edits and a locked list
    /// change nothing.
    pub(crate) fn edit_title_audio(&mut self, title_ids: &[String], edit: AudioEdit) {
        for id in title_ids {
            let Some(request) = self.working_set.audio_request(id) else {
                continue;
            };
            if let Some(next) = self.audio.edit_title(request, edit) {
                self.working_set.set_audio_request(id, next);
            }
        }
    }

    pub(crate) fn apply_default_audio(&mut self, title_ids: &[String]) {
        let request = self.audio.request();
        for id in title_ids {
            self.working_set.set_audio_request(id, request.clone());
        }
    }

    /// Returns the session to empty. Writes already waiting on an export stay
    /// accepted.
    pub(crate) fn reset(&mut self) {
        // A submission held for review goes with the titles; a running one
        // keeps its sources until it ends.
        self.cancel_review();
        self.working_set.reset();
        if self.submitting {
            self.working_set.set_order_locked(true);
        }
        self.lookup = LookupState {
            request: self.lookup.request + 1,
            ..LookupState::default()
        };
        self.reset_metadata();
    }

    // ---- Form ----

    // An edit ends the last action's status; what shows next is whatever is
    // wrong with the values on screen.
    pub(crate) fn set_field(&mut self, field: MetadataField, value: String) {
        self.form.set_value(field, value);
        self.status = None;
    }

    pub(crate) fn set_field_action(
        &mut self,
        field: MetadataField,
        action: super::metadata_form::FieldAction,
    ) {
        self.form.set_action(field, action);
        self.status = None;
    }

    /// Applies a lookup result to the form when `title` is the one title both
    /// selected and bound. Returns whether it applied.
    pub(crate) fn apply_lookup(
        &mut self,
        title: &QueuedTitle,
        metadata: &AudiobookMetadata,
        cover: Option<Vec<u8>>,
    ) -> bool {
        let is_title = |titles: &[BoundTitle]| matches!(titles, [only] if only.path == title.path && only.input_id == title.title_id);
        if self.save_in_progress || !is_title(&self.bound) || !is_title(&self.selected_titles()) {
            return false;
        }
        self.form.apply_lookup(metadata);
        if let Some(cover) = cover.filter(|cover| !cover.is_empty()) {
            self.apply_cover(cover);
        }
        true
    }

    // ---- Cover ----

    /// The one title a cover change applies to: exactly one valid selected title.
    fn cover_owner(&self) -> Option<PathBuf> {
        let mut valid = self
            .working_set
            .selected_files()
            .into_iter()
            .filter(|file| file.is_valid);
        match (valid.next(), valid.next()) {
            (Some(file), None) => Some(file.path.clone()),
            _ => None,
        }
    }

    /// Whose cover is shown: the selected title's; for several selected
    /// titles, theirs only when they all match; with nothing selected, the
    /// first valid title's.
    fn cover_display_path(&self) -> Option<PathBuf> {
        let valid: Vec<&AudioFile> = self
            .working_set
            .selected_files()
            .into_iter()
            .filter(|file| file.is_valid)
            .collect();
        match valid.as_slice() {
            [] => self
                .working_set
                .files()
                .iter()
                .find(|file| file.is_valid)
                .map(|file| file.path.clone()),
            [only] => Some(only.path.clone()),
            [first, rest @ ..] => {
                let cover = self.tags.effective_cover(&first.path);
                rest.iter()
                    .all(|file| self.tags.effective_cover(&file.path) == cover)
                    .then(|| first.path.clone())
            }
        }
    }

    fn refresh_displayed_cover(&mut self) {
        let cover = self
            .cover_display_path()
            .and_then(|path| self.tags.effective_cover(&path));
        if cover != self.cover.displayed {
            self.cover.displayed = cover;
            self.cover.image_revision += 1;
        }
    }

    /// A read of the file whose cover should be shown, unless the session
    /// already knows whether that file has art.
    fn cover_read(&mut self) -> Option<ReadTicket> {
        let target = self.cover_read_target()?;
        let answered = self.tags.effective_cover(&target).is_some()
            || self
                .tags
                .pending(&target)
                .is_some_and(|pending| pending.patch.cover_art.is_some())
            || self.tags.has_source_read(&target);
        if answered {
            return None;
        }
        self.tags.begin_read(&target)
    }

    /// The first valid selected title, or the first valid title when none is
    /// selected.
    fn cover_read_target(&self) -> Option<PathBuf> {
        let selected = self.working_set.selected_files();
        selected
            .into_iter()
            .chain(self.working_set.files())
            .find(|file| file.is_valid)
            .map(|file| file.path.clone())
    }

    pub(crate) fn displayed_cover(&self) -> Option<Vec<u8>> {
        self.cover.displayed.clone()
    }

    fn set_cover_notice(&mut self, notice: CoverNotice) {
        self.cover.notice = Some(notice);
        self.cover.notice_serial += 1;
    }

    /// Starts a cover load and returns its request; a later choice or Clear
    /// supersedes it.
    pub(crate) fn begin_cover_request(&mut self, from_url: bool) -> u64 {
        self.cover.request += 1;
        if from_url {
            self.cover.loading = true;
            self.cover.notice = None;
        }
        self.cover.request
    }

    pub(crate) fn cover_request(&self) -> u64 {
        self.cover.request
    }

    pub(crate) fn cover_url_required(&mut self) {
        self.set_cover_notice(CoverNotice::UrlRequired);
    }

    /// Stages `bytes` as the selected title's cover. Returns whether a title
    /// took it.
    pub(crate) fn apply_cover(&mut self, bytes: Vec<u8>) -> bool {
        let Some(owner) = self.cover_owner() else {
            return false;
        };
        self.tags.stage(
            &owner,
            &MetadataIntentPatch {
                cover_art: Some(PatchOp::Set(bytes)),
                ..Default::default()
            },
        );
        self.cover.custom = true;
        self.cover.removal_requested = false;
        true
    }

    pub(crate) fn clear_cover(&mut self) {
        if let Some(owner) = self.cover_owner() {
            self.tags.stage(
                &owner,
                &MetadataIntentPatch {
                    cover_art: Some(PatchOp::Clear),
                    ..Default::default()
                },
            );
        }
        self.cover.removal_requested = true;
        self.cover.custom = false;
        self.cover.notice = None;
        self.cover.loading = false;
        self.cover.request += 1;
    }

    /// Ends a cover load that was shown as loading.
    pub(crate) fn cover_load_finished(
        &mut self,
        from_url: bool,
        result: Result<Vec<u8>, AppError>,
    ) -> bool {
        self.cover.loading = false;
        match result {
            Ok(bytes) => {
                let applied = self.apply_cover(bytes);
                if from_url {
                    self.set_cover_notice(CoverNotice::LoadedFromUrl);
                }
                applied
            }
            Err(error) => {
                log::error!("Failed to load cover art: {error}");
                self.set_cover_notice(CoverNotice::LoadFailed {
                    error: AppErrorEnvelope::from(&error),
                });
                false
            }
        }
    }

    // ---- Save ----

    /// Files exports read, plus those a submission being prepared will read
    /// and downloads being removed.
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

        // A grouped title's draft is kept for its output and never written
        // into a constituent source.
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
        if items.is_empty() {
            self.status = Some(if grouped {
                MetadataStatus::GroupedEditsKept
            } else {
                MetadataStatus::NoPendingChanges
            });
            return None;
        }

        let mut plan = SavePlan::default();
        for item in items {
            // A file the deferred writer is writing right now is as busy as
            // one an export is reading.
            let being_written = self
                .deferred
                .iter()
                .any(|write| write.phase == DeferredPhase::Writing && write.item.path == item.path);
            if in_use.contains(&item.path) && is_temporary(&item.path) {
                plan.held += 1;
            } else if in_use.contains(&item.path) || being_written {
                plan.waiting += 1;
                self.defer(item);
            } else {
                // This write carries everything an earlier waiting one would.
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

    /// Queues `item`, replacing an older waiting edit for the file. A write
    /// already running stays recorded, so the file stays busy until it ends.
    fn defer(&mut self, item: SaveItem) {
        self.deferred
            .retain(|write| write.item.path != item.path || write.phase == DeferredPhase::Writing);
        self.deferred.push(DeferredWrite {
            item,
            phase: DeferredPhase::Waiting,
        });
    }

    /// Records what a Save wrote. `saved` are the items the file now carries;
    /// `written` every file it was writing, which are free again even if a
    /// Reset forgot the Save.
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
            let cover_was_submitted = item.patch.cover_art.is_some();
            self.tags
                .commit_saved(&item.path, &item.patch, item.revision);
            // A cover changed while the save ran is still unsaved.
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

    /// Takes the waiting writes whose files no export is reading any more.
    /// Each stays taken until `finish_deferred` reports its result.
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

    /// Files with a Save accepted and not yet written.
    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .deferred
            .iter()
            .map(|write| write.item.path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    /// Records what the deferred writer wrote. The title may have left the
    /// list and come back meanwhile: a written edit still becomes the known
    /// tags of a loaded file, and a failed one is pending again there so Save
    /// retries it.
    pub(crate) fn finish_deferred(&mut self, results: &[(SaveItem, bool)]) {
        let loaded = self.working_set.source_paths();
        let mut written_count = 0;
        for (item, written) in results {
            self.deferred.retain(|write| {
                write.phase != DeferredPhase::Writing
                    || write.item.path != item.path
                    || write.item.revision != item.revision
            });
            if !loaded.contains(&item.path) {
                // Not loaded: nothing on screen describes this file.
            } else if *written {
                self.tags
                    .commit_saved(&item.path, &item.patch, item.revision);
            } else if self.tags.pending(&item.path).is_none() {
                self.tags.stage(&item.path, &item.patch);
            }
            written_count += usize::from(*written);
        }
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

    // ---- Processing handoff ----

    /// The pending edits for `paths`, as processing takes them. The engine
    /// reads each source's own tags during processing; only edits cross here.
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
