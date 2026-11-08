use std::collections::HashSet;

use super::super::audio::{AudioDefaults, AudioSnapshot};
use super::super::exports::RestartOffer;
use super::super::lookup::LookupSnapshot;
use super::super::metadata_form::MetadataForm;
use super::super::output::{OutputPlan, OutputPreview, OutputSnapshot};
use super::super::working_set::{SelectionSnapshot, TitlesSnapshot, WorkingSet};
use super::lookup::LookupState;
use super::{CoverSnapshot, MetadataSnapshot, SessionState, SessionUpdate, TagPreview};
use crate::audio::{AudioFile, TitleAudioRequest};
use crate::metadata::{processing_album_sort, AudiobookMetadata};

pub(super) struct Parts {
    revision: u64,
    titles: TitlesSnapshot,
    titles_changes: u64,
    selection: SelectionSnapshot,
    metadata: MetadataSnapshot,
    lookup: LookupSnapshot,
    audio: AudioSnapshot,
    output: OutputSnapshot,
}

impl Parts {
    pub(super) fn initial(
        working_set: &WorkingSet,
        form: &MetadataForm,
        lookup: &LookupState,
    ) -> Self {
        Self {
            revision: 0,
            titles: working_set.titles(0),
            titles_changes: working_set.titles_changes(),
            selection: working_set.selection(0),
            metadata: MetadataSnapshot {
                revision: 0,
                binding: 0,
                form: form.snapshot(),
                cover: CoverSnapshot {
                    image: None,
                    custom: false,
                    loading: false,
                    notice: None,
                    notice_serial: 0,
                },
                tags: TagPreview::default(),
                save_in_progress: false,
                status: None,
                has_pending_edits: false,
            },
            lookup: lookup.snapshot(0),
            audio: AudioSnapshot {
                revision: 0,
                capabilities: None,
                defaults: AudioDefaults::default().defaults_view(),
                titles: Default::default(),
                selection: None,
                refusal: None,
            },
            output: OutputPlan::default().snapshot(0, OutputPreview::NoDirectory, None),
        }
    }
}

impl SessionState {
    pub(crate) fn revision(&self) -> u64 {
        self.parts.revision
    }

    pub(crate) fn lookup_revision(&self) -> u64 {
        self.parts.lookup.revision
    }

    pub(crate) fn settle(&mut self) {
        self.refresh_displayed_cover();
        let refusal_is_outdated = self.list_locked() != self.parts.titles.order_locked
            || self.working_set.selected_indices() != self.parts.selection.selected_indices;
        if refusal_is_outdated {
            self.audio_refusal = None;
        }
        let next = self.parts.revision + 1;
        let mut changed = false;

        let listed = self.working_set.source_ids();
        self.staged.finish_unlisted(&listed);
        let titles: HashSet<&str> = self
            .working_set
            .files()
            .iter()
            .map(|file| file.input_id.as_str())
            .collect();
        self.exports.retain_listed(&titles);
        let companions = self.staged.companions();
        if self.working_set.titles_changes() != self.parts.titles_changes
            || companions != self.parts.titles.companions
            || self.source_covers.unpublished()
        {
            self.parts.titles_changes = self.working_set.titles_changes();
            let mut titles = TitlesSnapshot {
                companions,
                ..self.working_set.titles(next)
            };
            self.source_covers.publish(&mut titles);
            self.parts.titles = titles;
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

    fn preview_title(&self) -> Option<&AudioFile> {
        let files = self.working_set.files();
        self.working_set
            .selected_indices()
            .iter()
            .min()
            .and_then(|index| files.get(*index))
            .or_else(|| files.iter().find(|file| file.is_valid))
    }

    pub(super) fn output_snapshot(&self, revision: u64) -> OutputSnapshot {
        let metadata = self.form.metadata();
        let title = self.preview_title().map(|file| {
            let format = self
                .working_set
                .audio_request(&file.input_id)
                .map_or_else(|| self.audio.format(), TitleAudioRequest::format);
            (&metadata, file.path.as_path(), format)
        });
        let preview = self.output.preview(title);
        OutputSnapshot {
            submission_in_progress: self.submitting.in_progress(),
            collision_review: self.submitting.review().map(|review| review.view.clone()),
            restart_offers: self.exports.offers(),
            restart_prompt: self.next_restart_prompt(),
            preview_run: self.preview.snapshot(),
            ..self
                .output
                .snapshot(revision, preview, self.submission.clone())
        }
    }

    fn next_restart_prompt(&self) -> Option<RestartOffer> {
        if self.submitting.in_progress() || self.keeping_location() || self.writes.save_running() {
            return None;
        }
        self.exports.next_prompt(
            self.output.naming_directory().as_ref(),
            &self.output.naming(),
        )
    }

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
            ..SessionUpdate::default()
        }
    }

    fn metadata_snapshot(&self, revision: u64) -> MetadataSnapshot {
        MetadataSnapshot {
            revision,
            binding: self.binding,
            form: self.form.snapshot(),
            cover: CoverSnapshot {
                image: self.displayed_cover_address(),
                custom: self.cover.custom,
                loading: self.cover.loading,
                notice: self.cover.notice.clone(),
                notice_serial: self.cover.notice_serial,
            },
            tags: self.tag_preview(),
            save_in_progress: self.writes.save_running(),
            status: self.status.clone(),
            has_pending_edits: self.tags.has_pending(),
        }
    }

    fn tag_preview(&self) -> TagPreview {
        let metadata = self.form.metadata();
        let text = |value: &Option<String>| value.clone().unwrap_or_default();
        TagPreview {
            title: text(&metadata.title),
            album: text(&metadata.title),
            artist: text(&metadata.artist),
            album_artist: text(&metadata.artist),
            composer: text(&metadata.composer),
            series: text(&metadata.series),
            series_part: text(&metadata.series_part),
            subseries: text(&metadata.subseries),
            subseries_part: text(&metadata.subseries_part),
            album_sort: self
                .album_sort_preview(metadata.clone())
                .unwrap_or_default(),
            year: text(&metadata.date),
            genre: text(&metadata.genre),
        }
    }

    fn album_sort_preview(&self, shown: AudiobookMetadata) -> Option<String> {
        let single = match self.bound.as_slice() {
            [title] => self.tags.effective(&title.path),
            _ => None,
        };
        processing_album_sort(&AudiobookMetadata {
            album_sort: single.and_then(|metadata| metadata.album_sort),
            ..shown
        })
    }
}
