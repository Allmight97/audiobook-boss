//! Session rules proven without I/O: a `Desk` plays the runtime's part,
//! answering source reads and saves from an in-memory disk.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use proptest::prelude::*;

use super::*;
use crate::audio::{AudioFile, AudioIntent, AudiobookFormat, SampleRateConfig, TitleAudioRequest};
use crate::session::metadata_form::{FieldAction, FieldSnapshot};
use crate::session::staged::tests::exported;
use crate::session::submission::SubmissionStatus;
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
    TitleAudioRequest {
        format: AudiobookFormat::M4b,
        intent: AudioIntent::Auto,
        settings: None,
        sample_rate: SampleRateConfig::Auto,
    }
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
    /// Every patch a Save wrote, by file.
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
            .map(|(name, _)| audio_file(name, true))
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

    /// Replaces the selection with `indices` through the draft gate.
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
        self.state.set_field(field, value.to_string());
        self.state.settle();
    }

    fn act(&mut self, field: MetadataField, action: FieldAction) {
        self.state.set_field_action(field, action);
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

    /// What a host would show right now.
    fn metadata(&mut self) -> MetadataSnapshot {
        self.state.settle();
        self.state
            .update_since(None)
            .metadata
            .expect("metadata part")
    }

    fn cover(&mut self) -> Option<Vec<u8>> {
        self.state.settle();
        self.state.displayed_cover()
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

    /// The exports reading `in_use` are all that remain; write what is free.
    fn exports_now_reading(&mut self, in_use: &HashSet<PathBuf>) {
        let ready = self.begin_deferred(in_use);
        self.end_deferred(ready, &[]);
    }

    /// Takes the waiting writes that are free, as the deferred writer does.
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

// ---- Edit intent reaches Save unchanged ----

#[test]
fn keep_after_blank_revokes_the_bulk_clear() {
    for (field, shown) in [
        (MetadataField::Author, "Shared Author"),
        (MetadataField::Genre, ""),
        (MetadataField::Date, ""),
        (MetadataField::Title, ""),
    ] {
        let mut desk = two_books();
        desk.act(field, FieldAction::Blank);
        desk.act(field, FieldAction::Keep);
        let snapshot = desk.field(field);
        assert_eq!((snapshot.value.as_str(), snapshot.dirty), (shown, false));

        desk.type_into(MetadataField::Genre, "Mystery");
        desk.save(&[]);

        assert_eq!(
            desk.written_by_file(),
            HashMap::from([
                (path("alpha"), genre_patch()),
                (path("beta"), genre_patch())
            ])
        );
    }
}

#[test]
fn blank_clears_the_field_and_its_album_mirror_on_every_selected_title() {
    let mut desk = two_books();
    desk.act(MetadataField::Title, FieldAction::Blank);
    desk.save(&[]);

    let cleared = MetadataIntentPatch {
        title: Some(PatchOp::Clear),
        album: Some(PatchOp::Clear),
        ..Default::default()
    };
    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(path("alpha"), cleared.clone()), (path("beta"), cleared)])
    );
}

#[test]
fn an_invalid_co_selected_input_receives_no_edit() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[]);
    desk.state
        .working_set
        .append_analyzed(vec![audio_file("broken", false)], &audio_request());
    desk.select(&[0, 1]).expect("select both");
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save(&[]);

    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(path("alpha"), genre_patch())])
    );
    assert!(desk.pending("broken").is_none());
}

#[test]
fn inherited_values_are_neither_rewritten_nor_revalidated_by_an_unrelated_edit() {
    let odd = AudiobookMetadata {
        series_part: Some("7/8".to_string()),
        ..alpha_tags()
    };
    let mut desk = Desk::open(&[("alpha", Some(odd))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save(&[]);

    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(path("alpha"), genre_patch())])
    );
}

#[test]
fn a_text_edit_does_not_restage_the_unchanged_cover() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Title, "Renamed");
    desk.save(&[]);

    assert_eq!(
        desk.written_by_file(),
        HashMap::from([(
            path("alpha"),
            MetadataIntentPatch {
                title: set("Renamed"),
                album: set("Renamed"),
                ..Default::default()
            }
        )])
    );
}

#[test]
fn a_failed_write_keeps_the_edit_pending_and_the_next_save_retries_it() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save(&["alpha"]);

    assert!(!desk.metadata().save_in_progress);
    assert_eq!(desk.pending("alpha"), Some(genre_patch()));
    assert!(desk.written.is_empty());

    desk.save(&[]);
    assert_eq!(desk.written, [(path("alpha"), genre_patch())]);
    assert!(desk.pending("alpha").is_none());
}

#[test]
fn an_invalid_edit_blocks_save_and_stages_nothing() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Date, "soon");

    assert!(desk.save(&[]).is_none());
    assert_eq!(desk.metadata().status, Some(MetadataStatus::SaveInvalid));
    assert!(desk.pending("alpha").is_none());
    assert!(desk.metadata().form.validation_message.is_some());
}

// ---- Processing handoff ----

#[test]
fn staging_reports_edits_with_no_valid_title_to_carry_them() {
    let mut desk = Desk::open(&[], &[]);
    desk.state
        .working_set
        .append_analyzed(vec![audio_file("broken", false)], &audio_request());
    desk.select(&[0]).expect("select");
    desk.type_into(MetadataField::Title, "Orphan");

    assert_eq!(desk.state.stage_bound_form(), StageOutcome::NoTarget);
}

#[test]
fn staging_carries_a_cover_clear_without_text_edits() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.clear_cover();

    assert_eq!(desk.state.stage_bound_form(), StageOutcome::Staged);
    assert_eq!(
        desk.state.pending_intents(&[path("alpha")]),
        [(
            path("alpha"),
            MetadataIntentPatch {
                cover_art: Some(PatchOp::Clear),
                ..Default::default()
            }
        )]
    );
}

#[test]
fn keep_after_staging_keeps_the_staged_value_and_its_intent() {
    let mut desk = two_books();
    desk.type_into(MetadataField::Genre, "Mystery");
    assert_eq!(desk.state.stage_bound_form(), StageOutcome::Staged);

    desk.act(MetadataField::Genre, FieldAction::Blank);
    desk.act(MetadataField::Genre, FieldAction::Keep);

    let field = desk.field(MetadataField::Genre);
    assert_eq!((field.value.as_str(), field.dirty), ("Mystery", false));
    assert_eq!(
        desk.state.pending_intents(&[path("alpha"), path("beta")]),
        [
            (path("alpha"), genre_patch()),
            (path("beta"), genre_patch())
        ]
    );
}

#[test]
fn earlier_text_and_cover_intent_survive_a_later_edit_and_second_handoff() {
    let other = AudiobookMetadata {
        artist: Some("Other Author".to_string()),
        ..beta_tags()
    };
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("other", Some(other))],
        &[0],
    );
    assert!(desk.state.apply_cover(vec![8, 9]));
    desk.select(&[1]).expect("select other");
    assert!(desk.state.apply_cover(vec![8, 9]));
    desk.select(&[0, 1]).expect("select both");
    desk.type_into(MetadataField::Author, "Edited Author");
    desk.state.stage_bound_form();

    let both = [path("alpha"), path("other")];
    let first = MetadataIntentPatch {
        artist: set("Edited Author"),
        cover_art: Some(PatchOp::Set(vec![8, 9])),
        ..Default::default()
    };
    assert_eq!(
        desk.state.pending_intents(&both),
        [
            (path("alpha"), first.clone()),
            (path("other"), first.clone())
        ]
    );

    desk.type_into(MetadataField::Title, "NMR 64k");
    desk.state.stage_bound_form();

    let second = MetadataIntentPatch {
        title: set("NMR 64k"),
        album: set("NMR 64k"),
        ..first
    };
    assert_eq!(
        desk.state.pending_intents(&both),
        [(path("alpha"), second.clone()), (path("other"), second)]
    );
}

// ---- Selection, the draft gate, and what the form shows ----

#[test]
fn a_dirty_edit_is_staged_onto_the_previous_title_before_the_next_one_shows() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Title, "Alpha draft");

    desk.select(&[1]).expect("gate accepts a valid edit");

    assert_eq!(desk.field(MetadataField::Title).value, "Beta");
    assert_eq!(
        desk.pending("alpha").and_then(|patch| patch.title),
        set("Alpha draft")
    );
    desk.select(&[0]).expect("select alpha again");
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha draft");
}

#[test]
fn an_invalid_edit_keeps_the_selection_and_the_titles_where_they_are() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Date, "soon");

    assert!(matches!(desk.select(&[1]), Err(GateBlock::Invalid(_))));
    assert!(matches!(
        desk.change(|set| {
            set.remove_file(0);
        }),
        Err(GateBlock::Invalid(_))
    ));

    assert_eq!(desk.state.working_set.files().len(), 2);
    assert_eq!(desk.state.working_set.selection(0).selected_indices, [0]);
    assert_eq!(desk.field(MetadataField::Date).value, "soon");
    assert!(matches!(
        desk.metadata().status,
        Some(MetadataStatus::DraftInvalid { .. })
    ));
}

#[test]
fn an_unreadable_file_still_shows_its_staged_edits_when_selected_again() {
    let mut desk = Desk::open(&[("alpha", None), ("beta", None)], &[0]);
    desk.type_into(MetadataField::Author, "Edited Author");
    desk.select(&[1]).expect("select beta");
    desk.select(&[0]).expect("select alpha");

    assert_eq!(desk.field(MetadataField::Author).value, "Edited Author");
}

#[test]
fn an_edit_typed_while_tags_load_survives_their_arrival() {
    let mut desk = Desk::open(&[], &[]);
    desk.disk.insert(path("alpha"), alpha_tags());
    // A first import of one valid file selects it; its tags are still loading.
    desk.state
        .working_set
        .append_analyzed(vec![audio_file("alpha", true)], &audio_request());
    desk.reads = desk.state.rebind();
    assert!(
        !desk.reads.is_empty(),
        "the selection asks for a source read"
    );

    desk.type_into(MetadataField::Author, "Typed while loading");
    desk.finish_reads();

    assert_eq!(
        desk.field(MetadataField::Author).value,
        "Typed while loading"
    );
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha");
}

#[test]
fn a_read_begun_before_a_save_cannot_replace_the_saved_author() {
    let mut desk = Desk::open(&[("alpha", None), ("beta", Some(beta_tags()))], &[0]);
    desk.type_into(MetadataField::Author, "Saved Author");
    desk.state.stage_bound_form();
    let stale = desk
        .state
        .tags
        .begin_read(&path("alpha"))
        .expect("read begins");

    desk.save(&[]);
    assert!(desk.pending("alpha").is_none());
    let binding = desk.state.binding;
    desk.state.finish_reads(
        binding,
        vec![(
            stale,
            Ok(AudiobookMetadata {
                title: Some("Alpha".to_string()),
                artist: Some("Old Author".to_string()),
                ..Default::default()
            }),
        )],
    );
    desk.select(&[1]).expect("select beta");
    desk.select(&[0]).expect("select alpha");

    assert_eq!(desk.field(MetadataField::Author).value, "Saved Author");
}

#[test]
fn one_title_draft_survives_grouping_source_reorder_save_and_separation() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Title, "Alpha draft");
    desk.select(&[1]).expect("select beta");
    desk.type_into(MetadataField::Title, "Beta draft");
    desk.select(&[0, 1]).expect("select both");
    desk.change(|set| {
        set.group_selected();
    })
    .expect("group");
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha draft");

    desk.state.working_set.reorder_sources("alpha", 0, 1);
    desk.bind();
    assert_eq!(desk.field(MetadataField::Title).value, "Alpha draft");

    // Saving a grouped title's draft keeps it for the output and writes no
    // constituent source.
    desk.type_into(MetadataField::Title, "Grouped title");
    assert!(desk.save(&[]).is_none());
    assert!(desk.written.is_empty());
    assert_eq!(
        desk.metadata().status,
        Some(MetadataStatus::GroupedEditsKept)
    );
    assert_eq!(
        desk.pending("alpha").and_then(|patch| patch.title),
        set("Grouped title")
    );

    desk.change(|set| {
        set.ungroup("alpha");
    })
    .expect("ungroup");
    desk.select(&[0]).expect("first source");
    assert_eq!(desk.field(MetadataField::Title).value, "Beta draft");
    desk.select(&[1]).expect("second source");
    assert_eq!(desk.field(MetadataField::Title).value, "Grouped title");
}

#[test]
fn removing_a_title_forgets_its_tags_so_reimport_reads_them_again() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Author, "Edited");
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    assert!(desk.state.known_tags(&path("alpha")).is_none());

    desk.disk.insert(
        path("alpha"),
        AudiobookMetadata {
            artist: Some("Changed On Disk".to_string()),
            ..alpha_tags()
        },
    );
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select");

    assert_eq!(desk.field(MetadataField::Author).value, "Changed On Disk");
}

#[test]
fn selection_cannot_change_while_a_save_is_in_progress() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk
        .state
        .begin_save(&HashSet::new(), |_| false)
        .expect("save begins");

    assert_eq!(desk.select(&[1]), Err(GateBlock::SaveInProgress));

    let epoch = desk.state.epoch;
    desk.state.finish_save(
        epoch,
        &paths_of(&plan.immediate),
        &plan.immediate,
        MetadataStatus::SaveCancelled,
    );
    assert!(desk.select(&[1]).is_ok());
}

// ---- Cover ----

#[test]
fn a_cover_belongs_to_exactly_one_valid_selected_title() {
    let mut desk = two_books();
    assert!(!desk.state.apply_cover(vec![1]), "two titles are selected");

    desk.select(&[0]).expect("select alpha");
    assert!(desk.state.apply_cover(vec![1]));
    assert_eq!(desk.cover(), Some(vec![1]));
    assert!(desk.metadata().cover.custom);
}

#[test]
fn several_selected_titles_show_a_cover_only_when_theirs_match() {
    let mut desk = two_books();
    assert_eq!(desk.cover(), Some(vec![7, 7, 7]));

    desk.select(&[1]).expect("select beta");
    desk.state.apply_cover(vec![9]);
    desk.select(&[0, 1]).expect("select both");

    assert_eq!(desk.cover(), None);
}

#[test]
fn with_nothing_selected_the_first_valid_title_cover_shows() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[],
    );
    assert_eq!(desk.cover(), Some(vec![7, 7, 7]));
}

#[test]
fn a_cover_changed_while_a_save_ran_is_still_unsaved_afterward() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.apply_cover(vec![1]);
    let plan = desk
        .state
        .begin_save(&HashSet::new(), |_| false)
        .expect("save begins");
    desk.state.apply_cover(vec![2]);

    let epoch = desk.state.epoch;
    desk.state.finish_save(
        epoch,
        &paths_of(&plan.immediate),
        &plan.immediate,
        MetadataStatus::SaveComplete {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            waiting: 0,
            held: 0,
            outputs: Default::default(),
        },
    );
    desk.state.settle();

    assert!(desk.metadata().cover.custom);
    assert_eq!(
        desk.pending("alpha").and_then(|patch| patch.cover_art),
        Some(PatchOp::Set(vec![2]))
    );
    assert_eq!(desk.cover(), Some(vec![2]));
}

#[test]
fn a_cover_load_failure_is_reported_and_changes_nothing() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.begin_cover_request(true);
    let applied = desk.state.cover_load_finished(
        true,
        Err(AppError::InvalidInput(
            "Only HTTPS image URLs are allowed.".to_string(),
        )),
    );
    desk.state.settle();

    assert!(!applied);
    let cover = desk.metadata().cover;
    assert!(!cover.loading);
    assert!(matches!(cover.notice, Some(CoverNotice::LoadFailed { .. })));
    assert!(desk.pending("alpha").is_none());
}

// ---- Lookup ----

#[test]
fn a_lookup_result_applies_only_to_the_title_it_was_chosen_for() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    let alpha = QueuedTitle {
        title_id: "alpha".to_string(),
        path: path("alpha"),
    };
    let found = AudiobookMetadata {
        title: Some("Found".to_string()),
        ..Default::default()
    };

    desk.select(&[1]).expect("select beta");
    assert!(!desk.state.apply_lookup(&alpha, &found, None));
    assert_eq!(desk.field(MetadataField::Title).value, "Beta");

    desk.select(&[0]).expect("select alpha");
    assert!(desk.state.apply_lookup(&alpha, &found, None));
    desk.save(&[]);
    // Keeping the existing cover leaves cover intent absent.
    assert_eq!(
        desk.written_by_file()[&path("alpha")],
        MetadataIntentPatch {
            title: set("Found"),
            album: set("Found"),
            ..Default::default()
        }
    );
}

// ---- Save targets ----

#[test]
fn save_with_no_export_running_writes_every_pending_edit_now() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.select(&[1]).expect("select beta");
    desk.type_into(MetadataField::Genre, "Mystery");

    let plan = desk.save(&[]).expect("save runs");

    // Save covers every title with a pending edit, not only the selection.
    assert_eq!(plan.immediate.len(), 2);
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
    assert_eq!(desk.disk[&path("beta")].genre.as_deref(), Some("Mystery"));
    assert!(!desk.metadata().has_pending_edits);
}

#[test]
fn save_on_a_local_source_in_flight_waits_for_every_export_reading_it() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    let reading = HashSet::from([path("alpha")]);

    let plan = desk.save_during(&reading, &[], &[]).expect("save accepted");

    assert_eq!((plan.immediate.len(), plan.waiting, plan.held), (0, 1, 0));
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Fantasy"));
    assert_eq!(desk.metadata().waiting_writes, [path("alpha")]);

    // A second, queued export still reads the file.
    desk.exports_now_reading(&reading);
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Fantasy"));

    desk.exports_now_reading(&HashSet::new());
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
    let metadata = desk.metadata();
    assert_eq!(
        metadata.status,
        Some(MetadataStatus::DeferredWritesFinished {
            written: 1,
            failed: 0
        })
    );
    assert!(!metadata.has_pending_edits);
    assert!(metadata.waiting_writes.is_empty());
}

#[test]
fn save_on_a_temporary_source_in_flight_writes_no_file_and_keeps_the_edit() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    let reading = HashSet::from([path("alpha")]);

    let plan = desk
        .save_during(&reading, &[path("alpha")], &[])
        .expect("save accepted");

    assert_eq!((plan.immediate.len(), plan.waiting, plan.held), (0, 0, 1));
    desk.exports_now_reading(&HashSet::new());
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Fantasy"));
    assert!(desk.written.is_empty());
    assert_eq!(desk.pending("alpha"), Some(genre_patch()));
    assert!(desk.metadata().waiting_writes.is_empty());
}

#[test]
fn a_later_save_replaces_the_edit_a_waiting_write_will_apply() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let reading = HashSet::from([path("alpha")]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&reading, &[], &[]);
    desk.type_into(MetadataField::Author, "Edited Author");
    desk.save_during(&reading, &[], &[]);

    assert_eq!(desk.metadata().waiting_writes, [path("alpha")]);
    desk.exports_now_reading(&HashSet::new());

    let on_disk = &desk.disk[&path("alpha")];
    assert_eq!(on_disk.genre.as_deref(), Some("Mystery"));
    assert_eq!(on_disk.artist.as_deref(), Some("Edited Author"));
    assert!(!desk.metadata().has_pending_edits);
}

#[test]
fn a_save_while_a_waiting_write_runs_never_writes_the_file_alongside_it() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    let reading = HashSet::from([path("alpha")]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&reading, &[], &[]);
    let running = desk.begin_deferred(&HashSet::new());
    assert_eq!(running.len(), 1);

    // Two more Saves while that write runs: both wait for it.
    for author in ["First", "Second"] {
        desk.type_into(MetadataField::Author, author);
        let plan = desk.save_during(&HashSet::new(), &[], &[]).expect("save");
        assert_eq!((plan.immediate.len(), plan.waiting), (0, 1));
        assert!(desk.begin_deferred(&HashSet::new()).is_empty());
    }

    desk.end_deferred(running, &[]);
    let next = desk.begin_deferred(&HashSet::new());
    desk.end_deferred(next, &[]);
    let on_disk = &desk.disk[&path("alpha")];
    assert_eq!(on_disk.genre.as_deref(), Some("Mystery"));
    assert_eq!(on_disk.artist.as_deref(), Some("Second"));
    assert!(desk.metadata().waiting_writes.is_empty());
}

#[test]
fn a_waiting_write_for_a_removed_title_reaches_it_when_it_returns() {
    for fails in [false, true] {
        let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
        let reading = HashSet::from([path("alpha")]);
        desk.type_into(MetadataField::Genre, "Mystery");
        desk.save_during(&reading, &[], &[]);
        desk.change(|set| {
            set.remove_file(0);
        })
        .expect("remove");
        assert_eq!(desk.state.waiting_write_paths(), [path("alpha")]);

        // Imported again and read while the export still holds the file.
        desk.import(&[("alpha", None)]);
        desk.select(&[0]).expect("select");
        assert_eq!(desk.field(MetadataField::Genre).value, "Fantasy");

        let fail: &[&str] = if fails { &["alpha"] } else { &[] };
        let taken = desk.begin_deferred(&HashSet::new());
        desk.end_deferred(taken, fail);

        // The form follows the file: written, or still pending for Save.
        assert_eq!(desk.field(MetadataField::Genre).value, "Mystery");
        assert_eq!(desk.pending("alpha").is_some(), fails);
        assert_eq!(
            desk.metadata().status,
            Some(MetadataStatus::DeferredWritesFinished {
                written: usize::from(!fails),
                failed: usize::from(fails),
            })
        );
    }
}

#[test]
fn a_failed_waiting_write_can_be_retried_after_its_title_returns_later() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.type_into(MetadataField::Genre, "Mystery");
    desk.save_during(&HashSet::from([path("alpha")]), &[], &[]);
    desk.change(|set| {
        set.remove_file(0);
    })
    .expect("remove");
    let taken = desk.begin_deferred(&HashSet::new());
    desk.end_deferred(taken, &["alpha"]);
    assert!(desk.state.waiting_write_paths().is_empty());
    desk.import(&[("alpha", None)]);
    desk.select(&[0]).expect("select returning title");
    assert_eq!(desk.field(MetadataField::Genre).value, "Mystery");
    desk.save_during(&HashSet::new(), &[], &[]);
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
}

#[test]
fn a_save_while_a_submission_is_prepared_waits_for_its_sources() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.output.set_directory("/library".to_string());
    let draft = desk
        .state
        .begin_submission(None)
        .expect("the submission starts");
    assert!(desk.state.working_set.order_locked());

    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk.save_during(&HashSet::new(), &[], &[]).expect("save");
    assert_eq!((plan.immediate.len(), plan.waiting), (0, 1));

    // Once the submission ends (here refused), the waiting write is free.
    desk.state
        .finish_submission(&draft, SubmissionStatus::Cancelled);
    assert!(!desk.state.working_set.order_locked());
    desk.exports_now_reading(&HashSet::new());
    assert_eq!(desk.disk[&path("alpha")].genre.as_deref(), Some("Mystery"));
}

// ---- Snapshots ----

#[test]
fn an_update_carries_only_the_parts_that_changed() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    let before = desk.state.revision();

    desk.type_into(MetadataField::Genre, "Mystery");
    let update = desk.state.update_since(Some(before));
    assert!(update.metadata.is_some());
    assert!(update.titles.is_none() && update.selection.is_none() && update.lookup.is_none());

    let before = desk.state.revision();
    desk.state.settle();
    assert_eq!(
        desk.state.revision(),
        before,
        "nothing changed, nothing stamped"
    );

    desk.select(&[1]).expect("select beta");
    let update = desk.state.update_since(Some(before));
    assert!(update.selection.is_some() && update.metadata.is_some());
    assert!(
        update.titles.is_none(),
        "selecting does not resend the titles"
    );
}

// ---- Laws over generated sequences ----

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
    Import(usize),
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
    /// A read begun now lands later, after whatever happens in between.
    BeginRead(usize),
    LandReads,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0..3usize).prop_map(Step::Import),
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

/// Pending patches by file, to check which edits a step dropped.
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

/// What a host that applied every update would be showing.
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

    #[allow(clippy::too_many_lines)] // one arm per generated step
    fn apply(&mut self, step: Step) -> Result<(), TestCaseError> {
        let desk = &mut self.desk;
        match step {
            Step::Import(index) => {
                let name = NAMES[index];
                desk.disk
                    .entry(path(name))
                    .or_insert_with(|| AudiobookMetadata {
                        title: Some(name.to_uppercase()),
                        ..Default::default()
                    });
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
                desk.state.set_field_action(FIELDS[field], action);
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
                    // The file's tags as they were before this session wrote any.
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

    /// Laws that hold after any sequence of edits, selection changes, source
    /// reads, saves, and exports starting and finishing.
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
            let pending_before = pending_by_file(&run.desk);
            let losing = matches!(
                step,
                Step::Save { .. } | Step::ExportsReading { .. } | Step::Remove(_)
            );
            let disk_before = run.desk.disk.clone();
            // Import puts a new file on the test's disk.
            let writes = matches!(
                step,
                Step::Save { .. } | Step::ExportsReading { .. } | Step::Import(_)
            );
            run.apply(step)?;
            let desk = &mut run.desk;

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
            // The edits on screen are invalid, so Save rightly wrote nothing.
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

// ---- Staged downloads ----

#[test]
fn a_download_waits_for_the_export_and_the_submission_reading_it() {
    let mut desk = Desk::open(&[("alpha", None), ("beta", None)], &[0]);
    let staged = &mut desk.state.staged;
    staged.register("job-1", "alpha", path("alpha"), Vec::new());
    staged.register("job-2", "beta", path("beta"), Vec::new());

    // Beta leaves the list while an export still reads it.
    desk.state.working_set.remove_file(1);
    desk.state.settle();
    assert!(desk
        .state
        .begin_staged_removal(&HashSet::from([path("beta")]), Instant::now())
        .is_empty());
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-2"]
    );

    // Alpha's export completed but it is submitted again.
    desk.state
        .staged
        .finish_export(&exported(&[("alpha", ChildJobStatus::Completed, false)]));
    desk.state.output.set_directory("/library".to_string());
    let draft = desk.state.begin_submission(None).expect("submission");
    assert!(desk
        .state
        .begin_staged_removal(&HashSet::new(), Instant::now())
        .is_empty());
    desk.state
        .finish_submission(&draft, SubmissionStatus::Cancelled);
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-1"]
    );
}

#[test]
fn a_download_being_removed_is_neither_written_nor_submitted() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state
        .staged
        .register("job-1", "alpha", path("alpha"), Vec::new());
    // Its export completed; the title stays listed.
    desk.state
        .staged
        .finish_export(&exported(&[("alpha", ChildJobStatus::Completed, false)]));
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-1"]
    );

    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk
        .save_during(&HashSet::new(), &[path("alpha")], &[])
        .expect("save");
    assert_eq!(
        (plan.immediate.len(), plan.held),
        (0, 1),
        "the file is held"
    );
    desk.state.output.set_directory("/library".to_string());
    assert!(desk.state.begin_submission(None).is_none());
    assert_eq!(
        desk.state.output_snapshot(0).submission,
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::SourceRemoved
        })
    );

    desk.state
        .finish_staged_removal("job-1", true, Instant::now());
    assert!(desk.state.staged.is_empty());
    assert!(
        !desk.state.working_set.files()[0].is_valid,
        "the removed download remains only as history"
    );
    let defaults = desk.state.audio.request();
    let mut next = crate::audio::AudioFile::new(path("beta"));
    next.is_valid = true;
    desk.state
        .working_set
        .append_analyzed(vec![next], &defaults);
    desk.bind();
    let draft = desk
        .state
        .begin_submission(None)
        .expect("the next batch excludes removed files");
    assert_eq!(
        draft.payload.input_files,
        vec![path("beta").to_string_lossy().into_owned()]
    );
}

#[test]
fn no_download_is_removed_while_a_save_writes() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state
        .staged
        .register("job-1", "beta", path("beta"), Vec::new());
    desk.state.settle();
    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk
        .state
        .begin_save(&HashSet::new(), |_| false)
        .expect("save");
    assert!(desk
        .state
        .begin_staged_removal(&HashSet::new(), Instant::now())
        .is_empty());

    let epoch = desk.state.epoch;
    desk.state.finish_save(
        epoch,
        &paths_of(&plan.immediate),
        &plan.immediate,
        MetadataStatus::SaveComplete {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            waiting: 0,
            held: 0,
            outputs: Default::default(),
        },
    );
    assert_eq!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now()),
        ["job-1"]
    );
}

#[test]
fn a_save_still_writing_after_a_reset_keeps_its_file_busy() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state
        .staged
        .register("job-1", "alpha", path("alpha"), Vec::new());
    desk.type_into(MetadataField::Genre, "Mystery");
    let plan = desk
        .state
        .begin_save(&HashSet::new(), |_| false)
        .expect("save");
    let epoch = desk.state.epoch;

    // Reset forgets the Save's form; its write keeps running.
    desk.state.reset();
    desk.state.settle();
    assert!(
        desk.state
            .begin_staged_removal(&HashSet::new(), Instant::now())
            .is_empty(),
        "the download is not removed under the write"
    );
    desk.import(&[("alpha", Some(alpha_tags()))]);
    desk.select(&[0]).expect("select");
    desk.state.output.set_directory("/library".to_string());
    assert!(desk.state.begin_submission(None).is_none());
    assert_eq!(
        desk.state.output_snapshot(0).submission,
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::SaveInProgress
        })
    );

    desk.state.finish_save(
        epoch,
        &paths_of(&plan.immediate),
        &plan.immediate,
        MetadataStatus::SaveCancelled,
    );
    assert!(desk.state.begin_submission(None).is_some());
}

#[test]
fn a_refused_second_submission_does_not_let_a_third_through() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.output.set_directory("/library".to_string());
    let first = desk.state.begin_submission(None).expect("first submission");
    desk.state.await_review(first, Vec::new());

    for _ in 0..2 {
        assert!(desk.state.begin_submission(None).is_none());
        assert_eq!(
            desk.state.output_snapshot(0).submission,
            Some(SubmissionStatus::Refused {
                reason: SubmitRefusal::Busy
            })
        );
    }

    // Cancelling the held review frees its sources and the list.
    desk.state.cancel_review();
    assert!(!desk.state.working_set.order_locked());
    assert!(desk.state.begin_submission(None).is_some());
}

#[test]
fn grouped_surround_sources_publish_downmix_warning_only_for_explicit_channels() {
    let mut state = SessionState {
        audio: AudioDefaults::new(None, Some(crate::audio::encoder_settings_capabilities())),
        ..SessionState::default()
    };
    let mut first = audio_file("first", true);
    first.channels = Some(2);
    let mut second = audio_file("second", true);
    second.channels = Some(6);
    let request = state.audio.request();
    state
        .working_set
        .append_analyzed(vec![first, second], &request);
    state.working_set.select_file(0, ONE);
    state.working_set.select_file(1, ADD);
    state.working_set.group_selected();
    assert!(
        !state.audio_snapshot(0).titles["first"]
            .facts
            .downmix_warning
    );
    let request = state
        .audio
        .edit_title(
            state
                .working_set
                .audio_request("first")
                .expect("group audio"),
            AudioEdit::Channels(crate::audio::ChannelConfig::Stereo),
        )
        .expect("accepted channel choice");
    state.working_set.set_audio_request("first", request);
    assert!(
        state.audio_snapshot(0).titles["first"]
            .facts
            .downmix_warning
    );
}
