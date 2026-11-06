use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use proptest::prelude::*;

use super::*;
use crate::audio::{AudioFile, AudioIntent, TitleAudioRequest};
use crate::errors::AppError;
use crate::metadata::PatchOp;
use crate::session::cover_address::{tests::host_request, CoverRequest};
use crate::session::lookup::QueuedTitle;
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

fn paths_of(items: &[SaveItem]) -> Vec<PathBuf> {
    items.iter().map(|item| item.path.clone()).collect()
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
        self.metadata()
            .form
            .fields
            .into_iter()
            .find(|snapshot| snapshot.field == field)
            .expect("field")
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
        let saved: Vec<SaveItem> = plan
            .immediate
            .iter()
            .filter(|item| !failing.contains(&item.path))
            .cloned()
            .collect();
        self.record_writes(&saved);
        let status = MetadataStatus::SaveComplete {
            succeeded: saved.len(),
            failed: plan.immediate.len() - saved.len(),
            cancelled: 0,
            waiting: plan.waiting,
            held: plan.held,
            outputs: Default::default(),
        };
        let epoch = self.state.epoch;
        self.state
            .finish_save(epoch, &paths_of(&plan.immediate), &saved, status);
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
        let results: Vec<(SaveItem, bool)> = taken
            .into_iter()
            .map(|item| {
                let written = !failing.contains(&item.path);
                (item, written)
            })
            .collect();
        let saved: Vec<SaveItem> = results
            .iter()
            .filter(|(_, written)| *written)
            .map(|(item, _)| item.clone())
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

const NAMES: [&str; 3] = ["a", "b", "c"];
const FIELDS: [MetadataField; 4] = [
    MetadataField::Title,
    MetadataField::Author,
    MetadataField::SeriesPart,
    MetadataField::Date,
];
const VALUES: [&str; 6] = ["", "A", "B", " 7 ", "1/2", "soon"];

#[derive(Debug, Clone)]
enum Step {
    Import(usize, bool),
    Select(Vec<usize>),
    Remove(usize),
    Group,
    Ungroup(usize),
    Type(usize, usize),
    Blank(usize, bool),
    Stage,
    Cover(u8),
    ClearCover,
    /// Save while exports read the files in `reading`; `failing` marks files
    /// whose write fails.
    Save {
        reading: u8,
        failing: u8,
    },
    /// Only the exports reading `reading` remain; waiting writes to files in
    /// `failing` fail.
    ExportsReading {
        reading: u8,
        failing: u8,
    },
    BeginRead(usize),
    LandReads,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0..3usize, any::<bool>()).prop_map(|(index, tagless)| Step::Import(index, tagless)),
        prop::collection::vec(0..3usize, 0..3).prop_map(Step::Select),
        (0..3usize).prop_map(Step::Remove),
        Just(Step::Group),
        (0..3usize).prop_map(Step::Ungroup),
        (0..4usize, 0..6usize).prop_map(|(field, value)| Step::Type(field, value)),
        (0..4usize, any::<bool>()).prop_map(|(field, blank)| Step::Blank(field, blank)),
        Just(Step::Stage),
        (1..4u8).prop_map(Step::Cover),
        Just(Step::ClearCover),
        (0..8u8, 0..8u8).prop_map(|(reading, failing)| Step::Save { reading, failing }),
        (0..8u8, 0..8u8).prop_map(|(reading, failing)| Step::ExportsReading { reading, failing }),
        (0..3usize).prop_map(Step::BeginRead),
        Just(Step::LandReads),
    ]
}

fn masked(mask: u8) -> Vec<PathBuf> {
    NAMES
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, name)| path(name))
        .collect()
}

fn pending_by_file(desk: &Desk) -> HashMap<PathBuf, MetadataIntentPatch> {
    NAMES
        .iter()
        .filter_map(|name| desk.pending(name).map(|patch| (path(name), patch)))
        .collect()
}

fn fields_of(patch: &MetadataIntentPatch) -> HashSet<&'static str> {
    let mut fields = HashSet::new();
    macro_rules! collect {
        ($($field:ident),+) => {$(
            if patch.$field.is_some() {
                fields.insert(stringify!($field));
            }
        )+};
    }
    collect!(title, artist, album, date, series_part, cover_art);
    fields
}

fn json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("serialize")
}

#[derive(Default)]
struct Mirror {
    seen: Option<u64>,
    titles: serde_json::Value,
    selection: serde_json::Value,
    metadata: serde_json::Value,
    lookup: serde_json::Value,
}

impl Mirror {
    fn apply(&mut self, state: &SessionState) {
        let update = state.update_since(self.seen);
        self.seen = Some(update.revision);
        if let Some(titles) = &update.titles {
            self.titles = json(titles);
        }
        if let Some(selection) = &update.selection {
            self.selection = json(selection);
        }
        if let Some(metadata) = &update.metadata {
            self.metadata = json(metadata);
        }
        if let Some(lookup) = &update.lookup {
            self.lookup = json(lookup);
        }
    }
}

struct Run {
    desk: Desk,
    /// Reads begun and not landed, with whether a save or removal since then
    /// made each stale.
    begun: Vec<(ReadTicket, bool)>,
    /// Which files are downloads the engine will remove.
    temporary: Vec<PathBuf>,
}

impl Run {
    fn invalidate_reads_of(&mut self, paths: &[PathBuf]) {
        for (ticket, stale) in &mut self.begun {
            if paths.contains(&ticket.path) {
                *stale = true;
            }
        }
    }

    #[expect(clippy::too_many_lines, reason = "one arm per generated step")]
    fn apply(&mut self, step: Step) -> Result<(), TestCaseError> {
        let desk = &mut self.desk;
        match step {
            Step::Import(index, tagless) => {
                let name = NAMES[index];
                if !tagless {
                    desk.disk
                        .entry(path(name))
                        .or_insert_with(|| AudiobookMetadata {
                            title: Some(name.to_uppercase()),
                            ..Default::default()
                        });
                }
                desk.import(&[(name, None)]);
            }
            Step::Select(indices) => {
                let count = desk.state.working_set.files().len();
                let indices: Vec<usize> =
                    indices.into_iter().filter(|index| *index < count).collect();
                let _ = desk.select(&indices);
            }
            Step::Remove(index) => {
                let removed: Vec<PathBuf> = desk
                    .state
                    .working_set
                    .files()
                    .get(index)
                    .map(|file| {
                        desk.state
                            .working_set
                            .sources_for(file)
                            .iter()
                            .map(|source| source.path.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                if desk
                    .change(|set| {
                        set.remove_file(index);
                    })
                    .is_ok()
                {
                    self.invalidate_reads_of(&removed);
                }
            }
            Step::Group => {
                let _ = desk.change(|set| {
                    set.group_selected();
                });
            }
            Step::Ungroup(index) => {
                let _ = desk.change(|set| {
                    set.ungroup(NAMES[index]);
                });
            }
            Step::Type(field, value) => desk.type_into(FIELDS[field], VALUES[value]),
            Step::Blank(field, blank) => {
                let action = if blank {
                    FieldAction::Blank
                } else {
                    FieldAction::Keep
                };
                let binding = desk.state.binding;
                desk.state.set_field_action(binding, FIELDS[field], action);
                desk.state.settle();
            }
            Step::Stage => {
                desk.state.stage_bound_form();
                desk.state.settle();
            }
            Step::Cover(byte) => {
                desk.state.apply_cover(vec![byte]);
                desk.state.settle();
            }
            Step::ClearCover => {
                desk.state.clear_cover();
                desk.state.settle();
            }
            Step::Save { reading, failing } => {
                let reading: HashSet<PathBuf> = masked(reading).into_iter().collect();
                let temporary = self.temporary.clone();
                let failing: Vec<&str> = NAMES
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| failing & (1 << index) != 0)
                    .map(|(_, name)| *name)
                    .collect();
                let disk_before = desk.disk.clone();
                let written_before = desk.written.len();
                let plan = desk.save_during(&reading, &temporary, &failing);
                let wrote: Vec<PathBuf> = desk.written[written_before..]
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect();

                // Save never writes a file an export is reading: a local one
                // waits and a temporary one is not written at all.
                for path in &reading {
                    prop_assert_eq!(desk.disk.get(path), disk_before.get(path));
                }
                if let Some(plan) = plan {
                    for item in &plan.immediate {
                        prop_assert!(!reading.contains(&item.path));
                    }
                }
                // A temporary source in flight is never queued for a later write.
                for path in desk.state.waiting_write_paths() {
                    prop_assert!(!temporary.contains(&path) || !reading.contains(&path));
                }
                self.invalidate_reads_of(&wrote);
            }
            Step::ExportsReading { reading, failing } => {
                let reading: HashSet<PathBuf> = masked(reading).into_iter().collect();
                let failing: Vec<&str> = NAMES
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| failing & (1 << index) != 0)
                    .map(|(_, name)| *name)
                    .collect();
                let disk_before = desk.disk.clone();
                let written_before = desk.written.len();
                let taken = desk.begin_deferred(&reading);
                desk.end_deferred(taken, &failing);
                let wrote: Vec<PathBuf> = desk.written[written_before..]
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect();
                for path in &reading {
                    prop_assert_eq!(desk.disk.get(path), disk_before.get(path));
                }
                self.invalidate_reads_of(&wrote);
            }
            Step::BeginRead(index) => {
                let path = path(NAMES[index]);
                if desk.state.working_set.source_paths().contains(&path) {
                    if let Some(ticket) = desk.state.tags.begin_read(&path) {
                        self.begun.push((ticket, false));
                    }
                }
            }
            Step::LandReads => {
                for (ticket, stale) in std::mem::take(&mut self.begun) {
                    let path = ticket.path.clone();
                    let known_before = desk.state.known_tags(&path);
                    let read = AudiobookMetadata {
                        title: Some("Stale".to_string()),
                        artist: Some("Stale".to_string()),
                        ..Default::default()
                    };
                    let read = if stale {
                        read
                    } else {
                        desk.disk.get(&path).cloned().unwrap_or_default()
                    };
                    let binding = desk.state.binding;
                    desk.state.finish_reads(binding, vec![(ticket, Ok(read))]);
                    if stale {
                        // A read begun before a save or a removal never lands.
                        prop_assert_eq!(desk.state.known_tags(&path), known_before);
                    }
                }
                desk.state.settle();
            }
        }
        Ok(())
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))] // PROPTEST_CASES overrides

    #[test]
    fn any_sequence_keeps_edits_reads_and_writes_truthful(
        temporary in 0..8u8,
        steps in prop::collection::vec(step(), 0..50)
    ) {
        let mut run = Run {
            desk: Desk::open(&[], &[]),
            begun: Vec::new(),
            temporary: masked(temporary),
        };
        let mut mirror = Mirror::default();
        mirror.apply(&run.desk.state);

        for step in steps {
            let covers_before: HashMap<PathBuf, Option<String>> =
                run.desk.source_covers().into_iter().collect();
            let written_before = run.desk.written.len();
            let pending_before = pending_by_file(&run.desk);
            let losing = matches!(
                step,
                Step::Save { .. } | Step::ExportsReading { .. } | Step::Remove(_)
            );
            let disk_before = run.desk.disk.clone();
            let writes = matches!(
                step,
                Step::Save { .. } | Step::ExportsReading { .. } | Step::Import(..)
            );
            run.apply(step)?;
            let desk = &mut run.desk;

            // A source's cover address changes exactly when a cover was written
            // into it, except for the sources of a grouped title.
            let wrote_cover: HashSet<PathBuf> = desk.written[written_before..]
                .iter()
                .filter(|(_, patch)| patch.cover_art.is_some())
                .map(|(path, _)| path.clone())
                .collect();
            let grouped_sources: HashSet<PathBuf> = desk
                .state
                .working_set
                .files()
                .iter()
                .map(|file| desk.state.working_set.sources_for(file))
                .filter(|sources| sources.len() > 1)
                .flatten()
                .map(|source| source.path.clone())
                .collect();
            for (path, after) in desk.source_covers() {
                let Some(before) = covers_before.get(&path) else {
                    continue;
                };
                let expected = wrote_cover.contains(&path) && !grouped_sources.contains(&path);
                prop_assert_eq!(*before != after, expected, "cover address of {:?}", path);
            }

            // An accepted edit stays pending until a write or a removal takes it.
            if !losing {
                let pending_after = pending_by_file(desk);
                for (path, before) in &pending_before {
                    let after = pending_after.get(path);
                    prop_assert!(after.is_some(), "pending edit for {path:?} vanished");
                    let after = fields_of(after.expect("checked"));
                    prop_assert!(fields_of(before).is_subset(&after));
                }
            }

            // Only Save and a finished export ever change a file.
            if !writes {
                prop_assert_eq!(&desk.disk, &disk_before);
            }

            // Only loaded source files have tags or edits, and what the
            // session knows of a loaded file never contradicts the file with
            // its pending edits applied.
            let live = desk.state.working_set.source_paths();
            for name in NAMES {
                let file = path(name);
                let Some(known) = desk.state.known_tags(&file) else {
                    continue;
                };
                prop_assert!(live.contains(&file), "{:?} has tags but is not loaded", file);
                let disk = desk.disk.get(&file).cloned().unwrap_or_default();
                let expected = desk
                    .state
                    .tags
                    .pending(&file)
                    .map_or(disk.clone(), |pending| pending.patch.overlay(&disk));
                for (shown, actual) in [
                    (&known.title, &expected.title),
                    (&known.artist, &expected.artist),
                    (&known.genre, &expected.genre),
                    (&known.date, &expected.date),
                ] {
                    if shown.is_some() {
                        prop_assert_eq!(shown, actual, "known tags of {:?}", file);
                    }
                }
            }

            // A clean form over one readable title shows exactly what Save
            // and processing would send.
            let form = desk.metadata().form;
            let selected = desk.state.working_set.selected_files();
            if let [title] = selected.as_slice() {
                if title.is_valid && form.fields.iter().all(|field| !field.dirty) {
                    let effective = desk.state.known_tags(&title.path).unwrap_or_default();
                    let shown = |field: MetadataField| {
                        form.fields
                            .iter()
                            .find(|snapshot| snapshot.field == field)
                            .map(|snapshot| snapshot.value.trim().to_string())
                            .unwrap_or_default()
                    };
                    // Staging trims, and the form keeps what was typed.
                    let trimmed = |value: Option<String>| {
                        value.unwrap_or_default().trim().to_string()
                    };
                    prop_assert_eq!(shown(MetadataField::Title), trimmed(effective.title));
                    prop_assert_eq!(shown(MetadataField::Author), trimmed(effective.artist));
                    prop_assert_eq!(
                        shown(MetadataField::SeriesPart),
                        trimmed(effective.series_part)
                    );
                }
            }

            // A host that applied every update shows what a fresh host would.
            mirror.apply(&desk.state);
            let full = desk.state.update_since(None);
            prop_assert_eq!(&mirror.titles, &json(&full.titles));
            prop_assert_eq!(&mirror.selection, &json(&full.selection));
            prop_assert_eq!(&mirror.metadata, &json(&full.metadata));
            prop_assert_eq!(&mirror.lookup, &json(&full.lookup));
        }

        // Once every export has finished and a Save runs with nothing in the
        // way, each local single-source title's file carries its edits.
        let desk = &mut run.desk;
        desk.exports_now_reading(&HashSet::new());
        desk.save(&[]);
        if desk.metadata().status == Some(MetadataStatus::SaveInvalid) {
            return Ok(());
        }
        let files: Vec<AudioFile> = desk.state.working_set.files().to_vec();
        for file in files {
            if !file.is_valid || desk.state.working_set.sources_for(&file).len() != 1 {
                continue;
            }
            prop_assert!(
                desk.state.tags.pending(&file.path).is_none(),
                "an edit for {:?} was never written",
                file.path
            );
        }
    }
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
