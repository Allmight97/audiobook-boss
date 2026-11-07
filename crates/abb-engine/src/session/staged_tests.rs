use super::*;
use crate::audio::{AudioFile, AudioIntent, TitleAudioRequest};
use crate::session::metadata_form::MetadataField;
use crate::session::state::SessionState;
use crate::session::working_set::SelectionModifiers;
use crate::test_cases::support::form::field_of;

fn path(name: &str) -> PathBuf {
    PathBuf::from(format!("/staged/{name}.m4b"))
}

fn pdf(input_id: &str) -> SupplementalProcessingAsset {
    SupplementalProcessingAsset {
        asset_id: format!("{input_id}-pdf"),
        input_id: input_id.to_string(),
        title_id: "B0".to_string(),
        path: PathBuf::from(format!("/staged/{input_id}.pdf")),
        file_name: format!("{input_id}.pdf"),
        size_bytes: 10,
        sha256: "00".to_string(),
    }
}

pub(crate) fn exported(titles: &[(&str, ChildJobStatus, bool)]) -> Vec<ChildJobSnapshot> {
    // A grouped title lists its sources joined by "+".
    titles
        .iter()
        .map(|(input_id, status, warning)| {
            let mut child: ChildJobSnapshot = serde_json::from_value(serde_json::json!({
                "childJobId": format!("child-{input_id}"),
                "operationId": "operation-1",
                "label": input_id,
                "status": status,
                "lane": "encodeCpu",
                "progress": { "stage": "complete", "percentage": 100.0, "message": "" },
                "cancellable": false,
                "cancelRequested": false,
                "supplementalWarning": warning.then_some("PDF failed"),
                "warnings": [],
            }))
            .expect("child snapshot");
            child.source_input_ids = input_id.split('+').map(str::to_string).collect();
            child
        })
        .collect()
}

fn staged() -> StagedSources {
    let mut staged = StagedSources::default();
    staged.register("job-1", "alpha", path("alpha"), vec![pdf("alpha")]);
    staged.register("job-1", "beta", path("beta"), Vec::new());
    staged.register("job-2", "gamma", path("gamma"), Vec::new());
    staged
}

#[test]
fn a_download_goes_only_when_every_title_from_it_is_finished_with() {
    let mut staged = staged();
    let free = HashSet::new();

    assert!(staged.finish(["alpha"]));
    assert!(
        staged.removable(&free, Instant::now()).is_empty(),
        "beta still needs job-1"
    );

    assert!(staged.finish_unlisted(&HashSet::from(["alpha", "gamma"])));
    assert_eq!(staged.removable(&free, Instant::now()), ["job-1"]);

    staged.removed("job-1");
    assert!(!staged.finish(["alpha"]), "a removed job is forgotten");
}

#[test]
fn only_titles_published_with_their_companions_finish_with_their_download() {
    let mut staged = staged();
    staged.register("job-3", "delta", path("delta"), Vec::new());
    staged.register("job-4", "omega", path("omega"), Vec::new());

    staged.finish_export(&exported(&[
        ("alpha+beta", ChildJobStatus::Completed, false),
        ("gamma", ChildJobStatus::Completed, false),
        ("delta", ChildJobStatus::Completed, true),
        ("omega", ChildJobStatus::Skipped, false),
    ]));

    assert_eq!(
        staged.removable(&HashSet::new(), Instant::now()),
        ["job-1", "job-2"]
    );
}

#[test]
fn a_finished_download_waits_while_anything_reads_its_files() {
    let mut staged = staged();
    staged.finish(["alpha", "beta"]);

    let reading_audio = HashSet::from([path("beta")]);
    assert!(staged.removable(&reading_audio, Instant::now()).is_empty());
    let reading_pdf = HashSet::from([PathBuf::from("/staged/alpha.pdf")]);
    assert!(staged.removable(&reading_pdf, Instant::now()).is_empty());
    assert_eq!(staged.removable(&HashSet::new(), Instant::now()), ["job-1"]);
}

#[test]
fn exports_take_each_title_companions_by_input_id() {
    let staged = staged();

    let assets = staged
        .assets_for(["alpha", "beta", "unknown"])
        .expect("assets");
    assert_eq!(assets.keys().collect::<Vec<_>>(), ["alpha"]);
    assert!(staged.assets_for(["beta"]).is_none());
    assert_eq!(
        staged.companions(),
        BTreeMap::from([("alpha".to_string(), vec!["alpha.pdf".to_string()])])
    );
}

#[test]
fn a_failed_removal_is_tried_again_only_after_the_retry_delay() {
    let mut staged = staged();
    staged.finish(["alpha", "beta"]);
    let failed = Instant::now();
    staged.removal_failed("job-1", failed);

    let free = HashSet::new();
    assert!(staged.removable(&free, failed).is_empty());
    assert!(staged.removable(&free, failed + RETRY_DELAY / 2).is_empty());
    assert_eq!(staged.removable(&free, failed + RETRY_DELAY), ["job-1"]);
}

fn listed(names: &[&str]) -> SessionState {
    let mut state = SessionState::default();
    let files = names
        .iter()
        .map(|name| AudioFile {
            is_valid: true,
            input_id: name.to_string(),
            ..AudioFile::new(path(name))
        })
        .collect();
    state
        .working_set
        .append_analyzed(files, &TitleAudioRequest::default_m4b(AudioIntent::Auto));
    for index in 0..names.len() {
        let modifiers = SelectionModifiers {
            multi: index > 0,
            range: false,
        };
        state.working_set.select_file(index, modifiers);
    }
    state.rebind();
    state
}

fn remove_download(state: &mut SessionState, name: &str) {
    state.staged.register("job", name, path(name), Vec::new());
    state
        .staged
        .finish_export(&exported(&[(name, ChildJobStatus::Completed, false)]));
    let held = HashSet::new();
    assert_eq!(state.begin_staged_removal(&held, Instant::now()), ["job"]);
    state.finish_staged_removal("job", true, Instant::now());
}

fn typed(state: &mut SessionState, value: &str) {
    state.set_field(state.binding, MetadataField::Genre, value.to_string());
    state.settle();
}

fn shown_genre(state: &mut SessionState) -> String {
    state.settle();
    let metadata = state.update_since(None).metadata.expect("metadata part");
    field_of(&metadata.form, MetadataField::Genre).value
}

fn pending_genre(state: &SessionState, name: &str) -> bool {
    state
        .tags
        .pending(&path(name))
        .is_some_and(|pending| pending.patch.genre.is_some())
}

#[test]
fn typing_on_a_title_survives_its_download_leaving_even_when_invalid() {
    let mut state = listed(&["alpha"]);
    state.set_field(state.binding, MetadataField::Date, "soon".to_string());
    typed(&mut state, "Mystery");

    remove_download(&mut state, "alpha");

    assert_eq!(shown_genre(&mut state), "Mystery");
    let metadata = state.update_since(None).metadata.expect("metadata part");
    let date = field_of(&metadata.form, MetadataField::Date);
    assert_eq!(date.value, "soon");
    assert!(date.dirty);
}

#[test]
fn typing_on_a_multi_selection_survives_and_saves_to_the_local_title_too() {
    let mut state = listed(&["alpha", "local"]);
    typed(&mut state, "Mystery");

    remove_download(&mut state, "alpha");
    assert_eq!(shown_genre(&mut state), "Mystery");

    let plan = state
        .begin_save(&HashSet::new(), |_| false)
        .expect("a save");
    assert_eq!(plan.immediate.len(), 1);
    assert_eq!(plan.immediate[0].path, path("local"));
    assert!(pending_genre(&state, "alpha"), "kept for alpha's output");
}

#[test]
fn typing_on_a_grouped_title_survives_its_first_source_leaving() {
    let mut state = listed(&["alpha", "local"]);
    assert!(state.working_set.group_selected());
    state.rebind();
    typed(&mut state, "Mystery");

    remove_download(&mut state, "alpha");
    assert_eq!(shown_genre(&mut state), "Mystery");

    state.begin_save(&HashSet::new(), |_| false);
    assert!(pending_genre(&state, "alpha"), "kept for the output");
}
