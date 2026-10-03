use super::*;
use crate::audio::{AudioIntent, AudiobookFormat, SampleRateConfig};
use proptest::prelude::*;

fn file(name: &str) -> AudioFile {
    let mut file = AudioFile::new(PathBuf::from(format!("/books/{name}")));
    file.input_id = name.to_string();
    file.is_valid = true;
    file
}

fn request(intent: AudioIntent) -> TitleAudioRequest {
    TitleAudioRequest {
        format: AudiobookFormat::M4b,
        intent,
        settings: None,
        sample_rate: SampleRateConfig::Auto,
    }
}

fn set_with(names: &[&str]) -> WorkingSet {
    let mut set = WorkingSet::default();
    set.append_analyzed(
        names.iter().map(|name| file(name)).collect(),
        &request(AudioIntent::Auto),
    );
    set
}

fn names(set: &WorkingSet) -> Vec<&str> {
    set.files()
        .iter()
        .map(|file| file.input_id.as_str())
        .collect()
}

fn selected_names(set: &WorkingSet) -> Vec<&str> {
    set.selected_files()
        .into_iter()
        .map(|file| file.input_id.as_str())
        .collect()
}

const NO_MODIFIERS: SelectionModifiers = SelectionModifiers {
    multi: false,
    range: false,
};
const MULTI: SelectionModifiers = SelectionModifiers {
    multi: true,
    range: false,
};
const RANGE: SelectionModifiers = SelectionModifiers {
    multi: false,
    range: true,
};

#[test]
fn first_import_replaces_and_selects_a_single_valid_file() {
    let set = set_with(&["only.m4b"]);
    assert_eq!(names(&set), ["only.m4b"]);
    assert_eq!(set.selection(0).selected_indices, [0]);

    let many = set_with(&["a.m4b", "b.m4b", "a.m4b"]);
    assert_eq!(names(&many), ["a.m4b", "b.m4b"], "duplicate paths collapse");
    assert!(many.selection(0).selected_indices.is_empty());
}

#[test]
fn later_imports_append_only_unseen_files_and_report_duplicates() {
    let mut set = set_with(&["a.m4b", "b.m4b"]);
    set.select_file(1, NO_MODIFIERS);

    set.append_analyzed(
        vec![file("b.m4b"), file("c.m4b")],
        &request(AudioIntent::Encode),
    );
    assert_eq!(names(&set), ["a.m4b", "b.m4b", "c.m4b"]);
    assert_eq!(
        selected_names(&set),
        ["b.m4b"],
        "append keeps the selection"
    );
    assert_eq!(
        set.audio_request("c.m4b").expect("request").intent,
        AudioIntent::Encode,
        "a new title takes the defaults current at its import"
    );
    assert_eq!(
        set.audio_request("a.m4b").expect("request").intent,
        AudioIntent::Auto,
        "loaded titles keep their own request"
    );

    set.append_analyzed(vec![file("a.m4b")], &request(AudioIntent::Auto));
    assert_eq!(set.titles(0).notice, Some(InputNotice::DuplicatesOnly));
    assert_eq!(names(&set), ["a.m4b", "b.m4b", "c.m4b"]);
}

#[test]
fn selection_supports_single_toggle_and_range() {
    let mut set = set_with(&["a", "b", "c", "d"]);
    set.select_file(1, NO_MODIFIERS);
    set.select_file(3, RANGE);
    assert_eq!(set.selection(0).selected_indices, [1, 2, 3]);
    assert_eq!(set.selection(0).selected_anchor, Some(3));

    set.select_file(2, MULTI);
    assert_eq!(set.selection(0).selected_indices, [1, 3]);
    set.select_file(0, MULTI);
    assert_eq!(set.selection(0).selected_indices, [0, 1, 3]);
    assert_eq!(set.selection(0).selected_anchor, Some(0));

    set.clear_selection();
    assert!(set.selection(0).selected_indices.is_empty());
    assert_eq!(set.selection(0).selected_anchor, None);
    assert_eq!(
        names(&set),
        ["a", "b", "c", "d"],
        "selection never changes files"
    );
}

#[test]
fn reordering_and_sorting_keep_the_selected_titles_selected() {
    let mut set = set_with(&["Part 10.m4b", "Part 2.m4b", "part 1.m4b"]);
    set.select_file(1, NO_MODIFIERS);

    set.toggle_sort();
    assert_eq!(names(&set), ["part 1.m4b", "Part 2.m4b", "Part 10.m4b"]);
    assert_eq!(selected_names(&set), ["Part 2.m4b"]);
    assert_eq!(set.titles(0).sort_direction, SortDirection::Ascending);
    assert!(set.titles(0).order_differs_from_import);

    set.toggle_sort();
    assert_eq!(names(&set), ["Part 10.m4b", "Part 2.m4b", "part 1.m4b"]);
    assert_eq!(selected_names(&set), ["Part 2.m4b"]);

    set.move_file(1, MoveDirection::Up);
    assert_eq!(names(&set), ["Part 2.m4b", "Part 10.m4b", "part 1.m4b"]);
    assert_eq!(selected_names(&set), ["Part 2.m4b"]);
    assert_eq!(
        set.titles(0).sort_direction,
        SortDirection::None,
        "a manual reorder clears the sort claim"
    );

    set.reorder_files(0, 2);
    assert_eq!(names(&set), ["Part 10.m4b", "part 1.m4b", "Part 2.m4b"]);
    assert_eq!(selected_names(&set), ["Part 2.m4b"]);

    set.restore_import_order();
    assert_eq!(names(&set), ["Part 10.m4b", "Part 2.m4b", "part 1.m4b"]);
    assert_eq!(selected_names(&set), ["Part 2.m4b"]);
    assert!(!set.titles(0).order_differs_from_import);
}

#[test]
fn a_locked_order_blocks_every_mutation_and_explains_a_refused_import() {
    let mut set = set_with(&["a", "b", "c"]);
    set.select_file(0, NO_MODIFIERS);
    set.select_file(1, MULTI);
    set.set_order_locked(true);

    set.move_file(0, MoveDirection::Down);
    set.reorder_files(0, 2);
    set.toggle_sort();
    set.restore_import_order();
    assert!(set.remove_file(0).is_empty());
    assert!(!set.clear_all());
    assert!(!set.group_selected());
    set.set_audio_request("a", request(AudioIntent::Encode));
    set.append_analyzed(vec![file("d")], &request(AudioIntent::Auto));

    assert_eq!(names(&set), ["a", "b", "c"]);
    let snapshot = set.titles(0);
    assert_eq!(snapshot.notice, Some(InputNotice::OrderLocked));
    assert_eq!(
        set.audio_request("a").expect("request").intent,
        AudioIntent::Auto
    );
}

#[test]
fn audio_choices_follow_title_identity_through_reorder_and_end_with_removal() {
    let mut set = set_with(&["a", "b", "c"]);
    set.set_audio_request("b", request(AudioIntent::Preserve));
    set.reorder_files(1, 0);
    set.toggle_sort();
    assert_eq!(
        set.audio_request("b").expect("request").intent,
        AudioIntent::Preserve
    );

    let index = set.index_of("b").expect("b is loaded");
    let removed = set.remove_file(index);
    assert_eq!(removed.len(), 1);
    assert!(set.audio_request("b").is_none());

    set.set_audio_request("b", request(AudioIntent::Encode));
    assert!(
        set.audio_request("b").is_none(),
        "a removed title cannot receive a choice"
    );
}

#[test]
fn grouping_keeps_the_first_title_as_anchor_and_splitting_restores_the_sources() {
    let mut set = set_with(&["a", "b", "c"]);
    set.select_file(0, NO_MODIFIERS);
    set.select_file(2, MULTI);
    assert!(set.group_selected());

    assert_eq!(names(&set), ["a", "b"]);
    assert_eq!(selected_names(&set), ["a"]);
    let anchor = set.files()[0].clone();
    let sources: Vec<&str> = set
        .sources_for(&anchor)
        .iter()
        .map(|source| source.input_id.as_str())
        .collect();
    assert_eq!(sources, ["a", "c"]);

    set.reorder_sources("a", 0, 1);
    assert_eq!(names(&set), ["a", "b"], "a source reorder keeps the anchor");
    assert_eq!(set.sources_for(&anchor)[0].input_id, "c");

    set.append_analyzed(vec![file("c")], &request(AudioIntent::Auto));
    assert_eq!(
        set.titles(0).notice,
        Some(InputNotice::DuplicatesOnly),
        "a source hidden inside a group is not imported again"
    );

    assert!(set.ungroup("a"));
    assert_eq!(names(&set), ["c", "a", "b"]);
    assert_eq!(selected_names(&set), ["c", "a"]);
    assert!(set.titles(0).title_sources_by_identity.is_empty());
}

#[test]
fn grouping_titles_with_different_audio_choices_requires_an_explicit_choice() {
    let mut set = set_with(&["a", "b"]);
    set.set_audio_request("b", request(AudioIntent::Preserve));
    set.select_all();
    assert!(set.group_selected());
    assert_eq!(set.titles(0).audio_choice_required, ["a"]);

    set.set_audio_request("a", request(AudioIntent::Encode));
    assert!(set.titles(0).audio_choice_required.is_empty());

    let mut same = set_with(&["a", "b"]);
    same.select_all();
    assert!(same.group_selected());
    assert!(same.titles(0).audio_choice_required.is_empty());
}

#[test]
fn removing_a_grouped_title_removes_every_source() {
    let mut set = set_with(&["a", "b", "c"]);
    set.select_file(0, NO_MODIFIERS);
    set.select_file(1, MULTI);
    assert!(set.group_selected());

    let removed = set.remove_file(0);
    assert_eq!(removed.len(), 2);
    assert_eq!(names(&set), ["c"]);
    assert!(set.titles(0).title_sources_by_identity.is_empty());
    assert_eq!(set.source_paths().len(), 1);
}

#[test]
fn a_cue_choice_confirms_or_ignores_without_touching_other_sources() {
    use crate::metadata::{ChapterPlan, ChapterSpec, CueSource};

    let embedded = vec![ChapterSpec {
        title: Some("Embedded".into()),
        start_ms: 0,
        end_ms: 1000,
    }];
    let mut with_cue = file("cue.mp3");
    with_cue.chapters = embedded.clone();
    with_cue.chapter_plan = Some(ChapterPlan {
        chapters: vec![ChapterSpec {
            title: Some("From CUE".into()),
            start_ms: 0,
            end_ms: 500,
        }],
        from_cue: true,
        source_fingerprint: "1:1".into(),
    });
    with_cue.cue_source = Some(CueSource {
        file_name: "cue.cue".into(),
        status: CueStatus::NeedsConfirmation,
        message: String::new(),
    });
    let mut set = WorkingSet::default();
    set.append_analyzed(
        vec![with_cue, file("plain.mp3")],
        &request(AudioIntent::Auto),
    );

    set.choose_cue("cue.mp3", CueChoice::ConfirmHundredths);
    let cue = |set: &WorkingSet| set.files()[0].cue_source.clone().expect("cue source");
    assert_eq!(cue(&set).status, CueStatus::Ready);

    set.choose_cue("cue.mp3", CueChoice::Ignore);
    assert_eq!(cue(&set).status, CueStatus::Ignored);
    let plan = set.files()[0].chapter_plan.clone().expect("chapter plan");
    assert!(!plan.from_cue);
    assert_eq!(plan.chapters, embedded);
    assert!(set.files()[1].cue_source.is_none());
}

#[derive(Debug, Clone)]
enum Step {
    Import(Vec<u8>),
    Select(usize, bool, bool),
    SelectAll,
    ClearSelection,
    Remove(usize),
    Move(usize, bool),
    Reorder(usize, usize),
    Sort,
    RestoreOrder,
    Group,
    Ungroup(usize),
    ReorderSources(usize, usize, usize),
    Lock(bool),
    ClearAll,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        prop::collection::vec(0u8..12, 1..4).prop_map(Step::Import),
        (0usize..8, any::<bool>(), any::<bool>()).prop_map(|(i, m, r)| Step::Select(i, m, r)),
        Just(Step::SelectAll),
        Just(Step::ClearSelection),
        (0usize..8).prop_map(Step::Remove),
        (0usize..8, any::<bool>()).prop_map(|(i, up)| Step::Move(i, up)),
        (0usize..8, 0usize..8).prop_map(|(a, b)| Step::Reorder(a, b)),
        Just(Step::Sort),
        Just(Step::RestoreOrder),
        Just(Step::Group),
        (0usize..8).prop_map(Step::Ungroup),
        (0usize..8, 0usize..4, 0usize..4).prop_map(|(i, a, b)| Step::ReorderSources(i, a, b)),
        any::<bool>().prop_map(Step::Lock),
        Just(Step::ClearAll),
    ]
}

fn apply(set: &mut WorkingSet, step: Step) {
    let id_at = |set: &WorkingSet, index: usize| set.files().get(index).map(|f| f.input_id.clone());
    match step {
        Step::Import(ids) => set.append_analyzed(
            ids.iter().map(|id| file(&format!("book-{id}"))).collect(),
            &request(AudioIntent::Auto),
        ),
        Step::Select(index, multi, range) => {
            set.select_file(index, SelectionModifiers { multi, range });
        }
        Step::SelectAll => set.select_all(),
        Step::ClearSelection => set.clear_selection(),
        Step::Remove(index) => {
            set.remove_file(index);
        }
        Step::Move(index, up) => set.move_file(
            index,
            if up {
                MoveDirection::Up
            } else {
                MoveDirection::Down
            },
        ),
        Step::Reorder(from, to) => set.reorder_files(from, to),
        Step::Sort => set.toggle_sort(),
        Step::RestoreOrder => set.restore_import_order(),
        Step::Group => {
            set.group_selected();
        }
        Step::Ungroup(index) => {
            if let Some(id) = id_at(set, index) {
                set.ungroup(&id);
            }
        }
        Step::ReorderSources(index, from, to) => {
            if let Some(id) = id_at(set, index) {
                set.reorder_sources(&id, from, to);
            }
        }
        Step::Lock(locked) => set.set_order_locked(locked),
        Step::ClearAll => {
            set.clear_all();
        }
    }
}

proptest! {
    /// Laws that hold after any sequence of working-set intents.
    #[test]
    fn any_intent_sequence_keeps_the_working_set_consistent(
        steps in prop::collection::vec(step(), 0..60)
    ) {
        let mut set = WorkingSet::default();
        for step in steps {
            apply(&mut set, step);
            let snapshot = set.titles(0);
            let selection = set.selection(0);
            let count = snapshot.files.len();

            // Selection points at loaded titles, once each, in order.
            let mut sorted = selection.selected_indices.clone();
            sorted.sort_unstable();
            sorted.dedup();
            prop_assert_eq!(&sorted, &selection.selected_indices);
            prop_assert!(selection.selected_indices.iter().all(|index| *index < count));
            if let Some(anchor) = selection.selected_anchor {
                prop_assert!(anchor < count);
            }
            if selection.selected_indices.is_empty() {
                prop_assert!(selection.selected_anchor.is_none() || count > 0);
            }

            // A source file belongs to exactly one title.
            let mut paths = Vec::new();
            for file in &snapshot.files {
                for source in set.sources_for(file) {
                    paths.push(source.path.clone());
                }
            }
            let unique: HashSet<_> = paths.iter().cloned().collect();
            prop_assert_eq!(unique.len(), paths.len());

            for file in &snapshot.files {
                // Every title has an audio request, and a group contains its anchor.
                prop_assert!(set.audio_request(&file.input_id).is_some());
                if let Some(sources) = snapshot.title_sources_by_identity.get(&file.input_id) {
                    prop_assert!(sources.len() >= 2);
                    prop_assert!(sources.iter().any(|source| source.input_id == file.input_id));
                }
            }
            // Group records and pending audio choices exist only for loaded titles.
            let ids: HashSet<&str> = snapshot.files.iter().map(|f| f.input_id.as_str()).collect();
            prop_assert!(snapshot.title_sources_by_identity.keys().all(|id| ids.contains(id.as_str())));
            prop_assert!(snapshot.audio_choice_required.iter().all(|id| ids.contains(id.as_str())));
        }
    }
}
