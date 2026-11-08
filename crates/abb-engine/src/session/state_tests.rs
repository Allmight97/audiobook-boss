use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use super::*;
use crate::audio::{AudioFile, AudioIntent, TitleAudioRequest};
use crate::errors::AppError;
use crate::metadata::PatchOp;
use crate::session::cover_address::{tests::host_request, CoverRequest};
use crate::session::exports::OutputEdits;
use crate::session::metadata_form::{FieldAction, FieldSnapshot, MetadataField, SeriesPartWarning};
use crate::session::staged::tests::exported;
use crate::session::submission::{SubmissionStatus, SubmitRefusal};
use crate::session::tag_cache::ReadTicket;
use crate::session::working_set::SelectionModifiers;
use crate::work_runtime::ChildJobStatus;

const ONE: SelectionModifiers = SelectionModifiers {
    multi: false,
    range: false,
};
const ADD: SelectionModifiers = SelectionModifiers {
    multi: true,
    range: false,
};

fn path(name: &str) -> PathBuf {
    PathBuf::from(format!("/books/{name}.m4b"))
}

fn audio_file(name: &str, valid: bool) -> AudioFile {
    let mut file = AudioFile::new(path(name));
    file.input_id = name.to_string();
    file.is_valid = valid;
    file
}

fn audio_request() -> TitleAudioRequest {
    TitleAudioRequest::default_m4b(AudioIntent::Auto)
}

fn written_batch(items: &[SaveItem]) -> WriteBatch {
    batch_of(items.iter().cloned().map(written_outcome).collect())
}

fn batch_of(outcomes: Vec<WriteOutcome>) -> WriteBatch {
    let succeeded = outcomes.iter().filter(|outcome| outcome.written).count();
    WriteBatch {
        failed: outcomes.len() - succeeded,
        succeeded,
        cancelled: 0,
        outcomes,
    }
}

fn write_failed() -> AppError {
    AppError::General("write failed".to_string())
}

fn written_outcome(item: SaveItem) -> WriteOutcome {
    WriteOutcome {
        item,
        written: true,
        rewrite: None,
    }
}

fn set(value: &str) -> Option<PatchOp<String>> {
    Some(PatchOp::Set(value.to_string()))
}

fn alpha_tags() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Alpha".to_string()),
        album: Some("Alpha".to_string()),
        artist: Some("Shared Author".to_string()),
        composer: Some("Alpha Reader".to_string()),
        genre: Some("Fantasy".to_string()),
        date: Some("2001".to_string()),
        series: Some("Saga".to_string()),
        series_part: Some("1".to_string()),
        subseries: Some("Arc".to_string()),
        subseries_part: Some("2".to_string()),
        description: Some("About alpha".to_string()),
        cover_art: Some(vec![7, 7, 7]),
        ..Default::default()
    }
}

fn beta_tags() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Beta".to_string()),
        album: Some("Beta".to_string()),
        genre: Some("Horror".to_string()),
        date: Some("2002".to_string()),
        ..alpha_tags()
    }
}

fn genre_patch() -> MetadataIntentPatch {
    MetadataIntentPatch {
        genre: set("Mystery"),
        ..Default::default()
    }
}

/// The session plus the files it works on. A missing disk entry is a file
/// that cannot be read.
struct Desk {
    state: SessionState,
    disk: HashMap<PathBuf, AudiobookMetadata>,
    reads: Vec<ReadTicket>,
    written: Vec<(PathBuf, MetadataIntentPatch)>,
}

impl Desk {
    fn open(books: &[(&str, Option<AudiobookMetadata>)], selected: &[usize]) -> Self {
        let mut desk = Self {
            state: SessionState::default(),
            disk: HashMap::new(),
            reads: Vec::new(),
            written: Vec::new(),
        };
        desk.import(books);
        desk.select(selected)
            .expect("an untouched form passes the gate");
        desk
    }

    fn import(&mut self, books: &[(&str, Option<AudiobookMetadata>)]) {
        for (name, tags) in books {
            if let Some(tags) = tags {
                self.disk.insert(path(name), tags.clone());
            }
        }
        let files = books
            .iter()
            .map(|(name, tags)| {
                let mut file = audio_file(name, true);
                file.set_tag_title(tags.as_ref().and_then(|tags| tags.title.clone()));
                file
            })
            .collect();
        self.state
            .working_set
            .append_analyzed(files, &audio_request());
        self.bind();
    }

    fn bind(&mut self) {
        let reads = self.state.rebind();
        self.reads.extend(reads);
        self.finish_reads();
    }

    fn finish_reads(&mut self) {
        let reads = std::mem::take(&mut self.reads)
            .into_iter()
            .map(|ticket| {
                let result = self
                    .disk
                    .get(&ticket.path)
                    .cloned()
                    .ok_or_else(|| AppError::General("file busy".to_string()));
                (ticket, result)
            })
            .collect();
        let binding = self.state.binding;
        self.state.finish_reads(binding, reads);
        self.state.settle();
    }

    fn select(&mut self, indices: &[usize]) -> Result<(), GateBlock> {
        self.gate()?;
        self.state.working_set.clear_selection();
        for (position, index) in indices.iter().enumerate() {
            let modifiers = if position == 0 { ONE } else { ADD };
            self.state.working_set.select_file(*index, modifiers);
        }
        self.bind();
        Ok(())
    }

    fn gate(&mut self) -> Result<(), GateBlock> {
        let gate = self.state.gate();
        self.state.settle();
        gate
    }

    fn change(&mut self, change: impl FnOnce(&mut WorkingSet)) -> Result<(), GateBlock> {
        self.gate()?;
        change(&mut self.state.working_set);
        self.bind();
        Ok(())
    }

    fn type_into(&mut self, field: MetadataField, value: &str) {
        let binding = self.state.binding;
        self.state.set_field(binding, field, value.to_string());
        self.state.settle();
    }

    fn act(&mut self, field: MetadataField, action: FieldAction) {
        let binding = self.state.binding;
        self.state.set_field_action(binding, field, action);
        self.state.settle();
    }

    fn field(&mut self, field: MetadataField) -> FieldSnapshot {
        crate::test_cases::support::form::field_of(&self.metadata().form, field)
    }

    fn metadata(&mut self) -> MetadataSnapshot {
        self.state.settle();
        self.state
            .update_since(None)
            .metadata
            .expect("metadata part")
    }

    fn cover(&mut self) -> Option<Vec<u8>> {
        let image = self.metadata().cover.image?;
        match CoverRequest::parse(&host_request(&image)) {
            Ok(CoverRequest::Session { revision }) => self.state.displayed_cover_at(revision),
            other => panic!("the displayed cover is a session cover: {other:?}"),
        }
    }

    fn source_covers(&mut self) -> Vec<(PathBuf, Option<String>)> {
        self.state.settle();
        let titles = self.state.update_since(None).titles.expect("titles part");
        let sources = titles.title_sources_by_identity.into_values().flatten();
        titles
            .files
            .into_iter()
            .chain(sources)
            .map(|file| (file.path, file.cover))
            .collect()
    }

    fn source_cover(&mut self, name: &str) -> Option<String> {
        let path = path(name);
        let covers = self.source_covers();
        covers.into_iter().find(|(source, _)| *source == path)?.1
    }

    /// Saves with no export running. `fail` names files whose write fails.
    fn save(&mut self, fail: &[&str]) -> Option<SavePlan> {
        self.save_during(&HashSet::new(), &[], fail)
    }

    fn save_during(
        &mut self,
        in_use: &HashSet<PathBuf>,
        temporary: &[PathBuf],
        fail: &[&str],
    ) -> Option<SavePlan> {
        let plan = self
            .state
            .begin_save(in_use, |path| temporary.iter().any(|temp| temp == path));
        self.state.settle();
        let plan = plan?;
        let failing: Vec<PathBuf> = fail.iter().map(|name| path(name)).collect();
        let batch = batch_of(
            plan.immediate
                .iter()
                .cloned()
                .map(|item| {
                    if failing.contains(&item.path) {
                        WriteOutcome::failed(item)
                    } else {
                        written_outcome(item)
                    }
                })
                .collect(),
        );
        let saved: Vec<SaveItem> = batch
            .outcomes
            .iter()
            .filter(|outcome| outcome.written)
            .map(|outcome| outcome.item.clone())
            .collect();
        self.record_writes(&saved);
        let epoch = self.state.epoch;
        self.state
            .finish_save(epoch, &plan, OutputEdits::default(), Ok(batch));
        self.state.settle();
        Some(plan)
    }

    fn record_writes(&mut self, saved: &[SaveItem]) {
        for item in saved {
            let on_disk = self.disk.entry(item.path.clone()).or_default();
            *on_disk = item.patch.overlay(on_disk);
            self.written.push((item.path.clone(), item.patch.clone()));
        }
    }

    fn exports_now_reading(&mut self, in_use: &HashSet<PathBuf>) {
        let ready = self.begin_deferred(in_use);
        self.end_deferred(ready, &[]);
    }

    fn begin_deferred(&mut self, in_use: &HashSet<PathBuf>) -> Vec<SaveItem> {
        let ready = self.state.take_ready_deferred(in_use);
        self.state.settle();
        ready
    }

    /// Finishes taken writes; `fail` names files whose write fails.
    fn end_deferred(&mut self, taken: Vec<SaveItem>, fail: &[&str]) {
        let failing: Vec<PathBuf> = fail.iter().map(|name| path(name)).collect();
        let results: Vec<WriteOutcome> = taken
            .into_iter()
            .map(|item| {
                if failing.contains(&item.path) {
                    WriteOutcome::failed(item)
                } else {
                    written_outcome(item)
                }
            })
            .collect();
        let saved: Vec<SaveItem> = results
            .iter()
            .filter(|outcome| outcome.written)
            .map(|outcome| outcome.item.clone())
            .collect();
        self.record_writes(&saved);
        self.state.finish_deferred(&results);
        self.state.settle();
    }

    fn written_by_file(&self) -> HashMap<PathBuf, MetadataIntentPatch> {
        self.written.iter().cloned().collect()
    }

    fn pending(&self, name: &str) -> Option<MetadataIntentPatch> {
        self.state
            .tags
            .pending(&path(name))
            .map(|pending| pending.patch.clone())
    }
}

fn two_books() -> Desk {
    Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0, 1],
    )
}

#[path = "state/cover_tests.rs"]
mod cover;

#[path = "state/snapshots_tests.rs"]
mod snapshots;

#[path = "state/title_audio_tests.rs"]
mod title_audio;

#[path = "state/submitting_tests.rs"]
mod submitting;

#[path = "state/bound_form_tests.rs"]
mod bound_form;

#[path = "state/save_tests.rs"]
mod save;

#[path = "state/lookup_tests.rs"]
mod lookup;

#[path = "state/sequence_tests.rs"]
mod sequence;
