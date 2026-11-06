use std::collections::VecDeque;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use super::*;
use crate::app_settings::{SettingsRuntime, SettingsSnapshot};
use crate::audio::{AudioFile, AudioIntent, AudiobookFormat};
use crate::host::EventSink;
use crate::metadata::{MetadataIntentPatch, PatchOp};
use crate::metadata_lookup::{
    MetadataLookupDiagnostic, MetadataLookupDiagnosticKind, OnlineMetadataResult,
};
use crate::output_artifact::NamingPreset;
use crate::power::PowerManager;
use crate::processing::JobRegistry;
use crate::session::audio_choice::AudioEdit;
use crate::session::cover_address::{self, tests::host_request};
use crate::session::lookup::LookupSnapshot;
use crate::session::lookup::LookupSource;
use crate::session::metadata_form::MetadataField;
use crate::session::state::MetadataSnapshot;

type SearchReply = Result<MetadataLookupResponse>;
/// A search as the provider saw it: the query and the sources asked.
type Search = (String, Vec<MetadataSource>);

#[derive(Default)]
struct Events(
    StdMutex<Vec<SessionUpdate>>,
    StdMutex<Vec<SettingsSnapshot>>,
);

impl EventSink for Events {
    fn emit(&self, event: EngineEvent) {
        match event {
            EngineEvent::Session(update) => self.0.lock().expect("events").push(update),
            EngineEvent::Settings(snapshot) => self.1.lock().expect("events").push(*snapshot),
            _ => {}
        }
    }
}

struct Rig {
    session: Session,
    events: Arc<Events>,
    searches: Arc<StdMutex<Vec<Search>>>,
    replies: Arc<StdMutex<VecDeque<tokio::sync::oneshot::Receiver<SearchReply>>>>,
    cover: Arc<StdMutex<Result<Vec<u8>>>>,
    removed: Arc<StdMutex<Vec<String>>>,
    /// Removals that fail before one succeeds.
    failing_removals: Arc<std::sync::atomic::AtomicUsize>,
    _config: tempfile::TempDir,
}

fn downloaded_cover() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(4, 2, image::Rgb([40, 120, 200]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode cover");
    bytes.into_inner()
}

fn written_cover() -> Vec<u8> {
    crate::metadata::optimize_cover_art(&downloaded_cover()).expect("optimize cover")
}

fn rig() -> Rig {
    rig_with_cover(None)
}

fn rig_with_cover(pending: Option<tokio::sync::oneshot::Receiver<Result<Vec<u8>>>>) -> Rig {
    let events = Arc::new(Events::default());
    let searches = Arc::new(StdMutex::new(Vec::new()));
    let replies: Arc<StdMutex<VecDeque<tokio::sync::oneshot::Receiver<SearchReply>>>> =
        Arc::default();
    let cover: Arc<StdMutex<Result<Vec<u8>>>> = Arc::new(StdMutex::new(Ok(downloaded_cover())));
    let network = Network {
        search: Box::new({
            let searches = Arc::clone(&searches);
            let replies = Arc::clone(&replies);
            move |query, sources| {
                searches.lock().expect("searches").push((query, sources));
                let reply = replies.lock().expect("replies").pop_front();
                Box::pin(async move {
                    match reply {
                        Some(reply) => reply.await.expect("scripted reply"),
                        None => Ok(found(&[])),
                    }
                })
            }
        }),
        covers: crate::cover_service::CoverService::new(Box::new({
            let cover = Arc::clone(&cover);
            let pending = StdMutex::new(pending);
            move |_url| {
                let pending = pending.lock().expect("cover reply").take();
                let result = match &*cover.lock().expect("cover") {
                    Ok(bytes) => Ok(bytes.clone()),
                    Err(error) => Err(AppError::General(error.to_string())),
                };
                Box::pin(async move {
                    match pending {
                        Some(reply) => reply.await.expect("cover reply"),
                        None => result,
                    }
                })
            }
        })),
    };
    let removed: Arc<StdMutex<Vec<String>>> = Arc::default();
    let failing_removals: Arc<std::sync::atomic::AtomicUsize> = Arc::default();
    let config = tempfile::TempDir::new().expect("settings folder");
    let (settings, _, _) =
        SettingsRuntime::start(config.path().to_path_buf(), PowerManager::default());
    let remote = crate::remote_source::tests::test_runtime_over(&config, settings.clone());
    let tasks = crate::engine::EngineTasks::default();
    let session = Session::with_network(
        SessionDeps {
            remote,
            host: Host::new(
                Arc::clone(&events) as Arc<dyn EventSink>,
                PowerManager::default(),
            ),
            work: WorkRuntime::new(tasks.clone()),
            jobs: Arc::new(JobRegistry::new(2)),
            temporary_root: PathBuf::from("/staged"),
            tasks,
            workspace_root: std::env::temp_dir().join("abb-session-tests"),
            settings,
            remove_staged: {
                let removed = Arc::clone(&removed);
                let failing = Arc::clone(&failing_removals);
                Arc::new(move |job_id| {
                    removed.lock().expect("removed").push(job_id.to_string());
                    let fail = failing
                        .try_update(
                            std::sync::atomic::Ordering::SeqCst,
                            std::sync::atomic::Ordering::SeqCst,
                            |left| left.checked_sub(1),
                        )
                        .is_ok();
                    if fail {
                        return Err(AppError::General("disk busy".to_string()));
                    }
                    Ok(())
                })
            },
        },
        network,
    );
    session.start_from_defaults(None, Some(crate::audio::encoder_settings_capabilities()));
    Rig {
        session,
        events,
        searches,
        replies,
        cover,
        removed,
        failing_removals,
        _config: config,
    }
}

fn result(title: &str) -> OnlineMetadataResult {
    OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: title.to_string(),
        title: title.to_string(),
        authors: Vec::new(),
        narrators: Vec::new(),
        series: None,
        series_part: None,
        subseries: None,
        subseries_part: None,
        description: None,
        published_date: None,
        duration_seconds: None,
        cover_url: Some("https://example.com/cover.jpg".to_string()),
        cover: None,
        audible_only: None,
    }
}

fn found(titles: &[&str]) -> MetadataLookupResponse {
    MetadataLookupResponse {
        results: titles.iter().map(|title| result(title)).collect(),
        diagnostics: Vec::new(),
    }
}

fn path(name: &str) -> PathBuf {
    PathBuf::from(format!("/books/{name}.m4b"))
}

impl Rig {
    fn load(&self, names: &[&str]) {
        self.session.transition(|state| {
            let files = names
                .iter()
                .map(|name| {
                    let mut file = AudioFile::new(path(name));
                    file.input_id = name.to_string();
                    file.is_valid = true;
                    file
                })
                .collect();
            let default_audio = state.audio.request();
            state.working_set.append_analyzed(files, &default_audio);
            for name in names {
                let ticket = state.tags.begin_read(&path(name)).expect("first read");
                state.tags.complete_read(
                    &ticket,
                    AudiobookMetadata {
                        title: Some(name.to_uppercase()),
                        artist: Some("Author".to_string()),
                        ..Default::default()
                    },
                );
            }
            state.rebind();
        });
    }

    fn listed_paths(&self) -> Vec<PathBuf> {
        self.session
            .snapshot()
            .titles
            .expect("titles")
            .files
            .into_iter()
            .map(|file| file.path)
            .collect()
    }

    async fn wait_for_titles(&self, count: usize) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while self.listed_paths().len() != count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("titles listed");
    }

    async fn wait_for_notice(&self, notice: InputNotice) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while self.session.snapshot().titles.expect("titles").notice != Some(notice.clone()) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("notice shown");
    }

    fn lock_order(&self, locked: bool) {
        self.session
            .transition(|state| state.working_set.set_order_locked(locked));
    }

    async fn send(&self, intent: SessionIntent) -> SessionOutcome {
        self.session.dispatch(intent).await.outcome
    }

    async fn type_into(&self, field: MetadataField, value: &str) {
        let binding = self.metadata().binding;
        let value = value.to_string();
        self.send(SessionIntent::SetField {
            binding,
            field,
            value,
        })
        .await;
    }

    fn title_id(&self, index: usize) -> String {
        self.session
            .snapshot()
            .titles
            .expect("titles")
            .files
            .get(index)
            .expect("title")
            .input_id
            .clone()
    }

    async fn select(&self, indices: &[usize]) {
        self.send(SessionIntent::ClearSelection).await;
        for index in indices {
            let outcome = self
                .send(SessionIntent::SelectFile {
                    title_id: self.title_id(*index),
                    modifiers: SelectionModifiers {
                        multi: true,
                        range: false,
                    },
                })
                .await;
            assert_eq!(outcome, SessionOutcome::Applied);
        }
    }

    fn lookup(&self) -> LookupSnapshot {
        self.session.snapshot().lookup.expect("lookup part")
    }

    fn metadata(&self) -> MetadataSnapshot {
        self.session.snapshot().metadata.expect("metadata part")
    }

    async fn cover_shown(&self) -> Option<Vec<u8>> {
        let image = self.metadata().cover.image?;
        self.session
            .cover(&host_request(&image))
            .await
            .expect("the displayed cover is served")
            .map(|cover| cover.to_vec())
    }

    fn title_shown(&self) -> String {
        self.metadata()
            .form
            .fields
            .into_iter()
            .find(|field| field.field == MetadataField::Title)
            .expect("title field")
            .value
    }

    fn pending(&self, name: &str) -> Option<MetadataIntentPatch> {
        self.session
            .lock()
            .pending_intents(&[path(name)])
            .into_iter()
            .next()
            .map(|(_, patch)| patch)
    }

    fn selected(&self) -> Vec<usize> {
        self.session
            .snapshot()
            .selection
            .expect("selection part")
            .selected_indices
    }

    fn hold_next_search(&self) -> tokio::sync::oneshot::Sender<SearchReply> {
        let (reply, wait) = tokio::sync::oneshot::channel();
        self.replies.lock().expect("replies").push_back(wait);
        reply
    }

    fn answer_next_search(&self, response: SearchReply) {
        self.hold_next_search()
            .send(response)
            .unwrap_or_else(|_| panic!("reply is waiting"));
    }

    fn searches(&self) -> Vec<Search> {
        self.searches.lock().expect("searches").clone()
    }

    fn lookup_apply(&self, index: usize) -> SessionIntent {
        SessionIntent::LookupApply {
            index,
            revision: self.lookup().revision,
        }
    }
}

#[tokio::test]
async fn an_unknown_select_file_title_id_changes_nothing() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.type_into(MetadataField::Date, "soon").await;
    let before = rig.selected();

    let outcome = rig
        .send(SessionIntent::SelectFile {
            title_id: "nobody".into(),
            modifiers: SelectionModifiers::default(),
        })
        .await;

    assert_eq!(outcome, SessionOutcome::Applied);
    assert_eq!(rig.selected(), before);
    assert_eq!(rig.title_shown(), "ALPHA");
}

#[tokio::test]
async fn a_select_file_begun_after_a_reorder_still_targets_the_same_title() {
    let rig = rig();
    rig.load(&["alpha", "beta", "gamma"]);
    let reorder = rig.session.begin(SessionIntent::ReorderFiles {
        title_id: "alpha".into(),
        to: 2,
    });
    let select = rig.session.begin(SessionIntent::SelectFile {
        title_id: "beta".into(),
        modifiers: SelectionModifiers::default(),
    });
    reorder.finish().await;
    select.finish().await;

    let titles = rig.session.snapshot().titles.expect("titles");
    assert_eq!(
        titles
            .files
            .iter()
            .map(|file| file.input_id.as_str())
            .collect::<Vec<_>>(),
        ["beta", "gamma", "alpha"]
    );
    assert_eq!(
        rig.selected()
            .into_iter()
            .map(|index| titles.files[index].input_id.as_str())
            .collect::<Vec<_>>(),
        ["beta"]
    );
}

#[tokio::test]
async fn a_reply_carries_what_its_intent_changed() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;

    let reply = rig
        .session
        .dispatch(SessionIntent::SetField {
            binding: rig.metadata().binding,
            field: MetadataField::Genre,
            value: "Mystery".to_string(),
        })
        .await;

    assert_eq!(reply.outcome, SessionOutcome::Applied);
    assert!(reply.update.metadata.is_some());
    assert!(reply.update.titles.is_none() && reply.update.selection.is_none());
}

#[tokio::test]
async fn a_rejected_selection_reports_why_and_leaves_the_selection() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.type_into(MetadataField::Date, "soon").await;

    let outcome = rig
        .send(SessionIntent::SelectFile {
            title_id: rig.title_id(1),
            modifiers: SelectionModifiers::default(),
        })
        .await;

    assert!(matches!(
        outcome,
        SessionOutcome::DraftRejected { message: Some(_) }
    ));
    assert_eq!(rig.selected(), [0]);
}

#[tokio::test]
async fn importing_a_folder_with_no_audio_explains_what_is_supported() {
    let rig = rig();
    let empty = tempfile::TempDir::new().expect("temp dir");

    rig.send(SessionIntent::Import {
        paths: vec![empty.path().to_string_lossy().into_owned()],
    })
    .await;

    let titles = rig.session.snapshot().titles.expect("titles part");
    assert!(titles.files.is_empty());
    assert!(matches!(
        titles.notice,
        Some(InputNotice::NoSupportedFiles { .. })
    ));
}

#[tokio::test]
async fn importing_lists_what_it_read_and_names_what_it_skipped() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let book = staged_wav(folder.path(), "book");
    let missing = folder.path().join("missing.wav");

    rig.send(SessionIntent::Import {
        paths: vec![
            book.to_string_lossy().into_owned(),
            missing.to_string_lossy().into_owned(),
        ],
    })
    .await;

    let titles = rig.session.snapshot().titles.expect("titles part");
    assert_eq!(
        rig.listed_paths(),
        vec![book.canonicalize().expect("canonical book")]
    );
    let Some(InputNotice::SkippedPaths { skipped }) = titles.notice else {
        panic!("expected the skipped path notice, got {:?}", titles.notice);
    };
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].path, missing.display().to_string());
    assert!(
        skipped[0]
            .reason
            .starts_with("File validation failed: Cannot read it:"),
        "the path is listed beside the reason, not inside it: {}",
        skipped[0].reason
    );
}

#[tokio::test]
async fn a_cover_url_is_required_before_anything_loads() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;

    let outcome = rig
        .send(SessionIntent::LoadCoverFromUrl {
            url: "   ".to_string(),
        })
        .await;

    assert_eq!(outcome, SessionOutcome::CoverLoadFailed);
    let cover = rig.metadata().cover;
    assert_eq!(
        cover.notice,
        Some(crate::session::state::CoverNotice::UrlRequired)
    );
    assert!(!cover.loading);
}

#[tokio::test]
async fn a_cover_loaded_from_a_url_is_staged_on_the_selected_title() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;

    let outcome = rig
        .send(SessionIntent::LoadCoverFromUrl {
            url: " https://example.com/cover.jpg ".to_string(),
        })
        .await;

    assert_eq!(outcome, SessionOutcome::Applied);
    let cover = rig.metadata().cover;
    assert!(cover.image.is_some() && cover.custom && !cover.loading);
    assert_eq!(
        cover.notice,
        Some(crate::session::state::CoverNotice::LoadedFromUrl)
    );
    assert_eq!(rig.cover_shown().await, Some(written_cover()));
}

#[tokio::test]
async fn reset_empties_the_session() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.type_into(MetadataField::Genre, "Mystery").await;

    rig.send(SessionIntent::Reset).await;

    let snapshot = rig.session.snapshot();
    assert!(snapshot.titles.expect("titles").files.is_empty());
    assert!(snapshot
        .selection
        .expect("selection")
        .selected_indices
        .is_empty());
    assert_eq!(rig.title_shown(), "");
    assert!(rig.pending("alpha").is_none());
}

#[tokio::test]
async fn intents_take_effect_in_the_order_they_begin_whatever_finishes_first() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;

    let select = rig.session.begin(SessionIntent::SelectFile {
        title_id: rig.title_id(1),
        modifiers: SelectionModifiers::default(),
    });
    let edit = rig.session.begin(SessionIntent::SetField {
        binding: rig.metadata().binding,
        field: MetadataField::Genre,
        value: "Mystery".to_string(),
    });
    let stage = rig.session.begin(SessionIntent::SelectAll);
    assert_eq!(stage.finish().await.outcome, SessionOutcome::Applied);
    edit.finish().await;
    select.finish().await;

    assert_eq!(rig.selected(), [0, 1]);
    assert!(rig.pending("alpha").is_none());
    assert_eq!(
        rig.pending("beta").and_then(|patch| patch.genre),
        Some(PatchOp::Set("Mystery".to_string()))
    );
}

#[tokio::test]
async fn opened_files_wait_for_unlock_without_a_host_request() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let opened = staged_wav(folder.path(), "opened");
    rig.lock_order(true);
    rig.session
        .import_opened(vec![opened.clone(), opened.clone()])
        .expect("opened files admitted");
    tokio::task::yield_now().await;
    assert!(rig.listed_paths().is_empty());
    rig.lock_order(false);
    rig.session.sources_released();
    rig.wait_for_titles(1).await;
    assert_eq!(
        rig.listed_paths(),
        vec![opened.canonicalize().expect("canonical opened path")]
    );

    rig.session
        .import_opened(vec![opened])
        .expect("opened again");
    rig.wait_for_notice(InputNotice::DuplicatesOnly).await;
    assert_eq!(rig.listed_paths().len(), 1);
}

#[tokio::test]
async fn a_reset_drops_opened_files_still_waiting_and_closing_refuses_them() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let opened = staged_wav(folder.path(), "opened");
    rig.lock_order(true);
    rig.session
        .import_opened(vec![opened.clone()])
        .expect("opened files admitted");
    rig.send(SessionIntent::Reset).await;
    rig.lock_order(false);
    rig.session.sources_released();
    rig.session.inner.deps.tasks.close();
    rig.session.inner.deps.tasks.wait().await;
    assert!(rig.listed_paths().is_empty(), "the reset dropped them");

    assert!(rig.session.import_opened(vec![opened]).is_err());
}

#[tokio::test]
async fn repeated_removal_targets_one_title_and_preserves_its_neighbours_edits() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[1]).await;
    rig.send(SessionIntent::SetField {
        binding: rig.metadata().binding,
        field: MetadataField::Genre,
        value: "Mystery".into(),
    })
    .await;
    let first = rig.session.begin(SessionIntent::RemoveFile {
        input_id: "alpha".into(),
    });
    let repeated = rig.session.begin(SessionIntent::RemoveFile {
        input_id: "alpha".into(),
    });
    first.finish().await;
    repeated.finish().await;
    assert_eq!(
        rig.session
            .snapshot()
            .titles
            .expect("titles")
            .files
            .iter()
            .map(|file| file.input_id.as_str())
            .collect::<Vec<_>>(),
        ["beta"]
    );
    assert_eq!(
        rig.pending("beta").and_then(|patch| patch.genre),
        Some(PatchOp::Set("Mystery".into()))
    );
}

#[tokio::test]
async fn a_host_that_attached_mid_intent_learns_its_result_from_an_event() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.send(SessionIntent::LookupOpen).await;
    let answer = rig.hold_next_search();
    let search = rig.session.begin(SessionIntent::LookupSearch);
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by a_host_that_attached_mid_intent_learns_its_result_from_an_event"
    )]
    let finished = tokio::spawn(search.finish());
    tokio::task::yield_now().await;

    let attached = rig.session.snapshot();
    answer
        .send(Ok(found(&["Dune"])))
        .unwrap_or_else(|_| panic!("search is waiting"));
    finished.await.expect("search finished");

    let latest = rig
        .events
        .0
        .lock()
        .expect("events")
        .iter()
        .filter_map(|update| update.lookup.clone())
        .max_by_key(|lookup| lookup.revision)
        .expect("a lookup event");
    assert!(latest.revision > attached.lookup.expect("lookup").revision);
    assert_eq!(latest.results.len(), 1);
}

#[tokio::test]
async fn an_import_that_fails_after_a_reset_leaves_the_new_session_alone() {
    let rig = rig();
    let empty = tempfile::TempDir::new().expect("temp dir");
    let import = rig.session.begin(SessionIntent::Import {
        paths: vec![empty.path().to_string_lossy().into_owned()],
    });
    rig.send(SessionIntent::Reset).await;

    assert_eq!(import.finish().await.outcome, SessionOutcome::Superseded);
    let titles = rig.session.snapshot().titles.expect("titles part");
    assert_eq!(titles.notice, None);
}

#[tokio::test]
async fn a_cover_cleared_while_loading_is_not_replaced_by_the_load() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;

    let load = rig.session.begin(SessionIntent::LoadCoverFromUrl {
        url: "https://example.com/cover.jpg".to_string(),
    });
    rig.send(SessionIntent::ClearCover).await;

    assert_eq!(load.finish().await.outcome, SessionOutcome::Superseded);
    assert_eq!(rig.pending("alpha").and_then(|patch| patch.cover_art), None);
    let cover = rig.metadata().cover;
    assert!(!cover.loading && cover.image.is_none());
}

fn audio(rig: &Rig) -> crate::session::AudioSnapshot {
    rig.session.snapshot().audio.expect("audio part")
}

#[tokio::test]
async fn covers_are_served_only_for_what_the_session_shows() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let listed = staged_wav(folder.path(), "listed");
    let unlisted = staged_wav(folder.path(), "unlisted");
    rig.send(SessionIntent::Import {
        paths: vec![listed.to_string_lossy().into_owned()],
    })
    .await;
    let request = |path: &std::path::Path| {
        let canonical = path.canonicalize().expect("canonical");
        host_request(&cover_address::audio(&canonical, 0))
    };

    assert_eq!(
        rig.session.cover(&request(&listed)).await.expect("listed"),
        None,
        "a listed source with no cover has none"
    );
    assert!(rig.session.cover(&request(&unlisted)).await.is_err());
    assert!(rig
        .session
        .cover(&host_request(&cover_address::remote(
            "https://covers.test/never-shown.jpg"
        )))
        .await
        .is_err());
    assert_eq!(
        rig.session
            .cover(&host_request(&cover_address::session(0)))
            .await
            .expect("session"),
        None
    );
}

fn output(rig: &Rig) -> crate::session::OutputSnapshot {
    rig.session.snapshot().output.expect("output part")
}

fn preview_path(rig: &Rig) -> String {
    match output(rig).preview {
        crate::session::OutputPreview::Path { path } => path,
        other => panic!("no preview path: {other:?}"),
    }
}

#[tokio::test]
async fn the_output_preview_follows_the_form_the_directory_and_the_format() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    assert_eq!(
        output(&rig).preview,
        crate::session::OutputPreview::NoDirectory
    );

    rig.send(SessionIntent::SetOutputDirectory {
        directory: "/out".to_string(),
    })
    .await;
    let first = preview_path(&rig);
    assert!(first.starts_with("/out/Author"), "{first}");
    assert!(first.ends_with(".m4b"), "{first}");

    rig.type_into(MetadataField::Title, "Dune").await;
    rig.send(SessionIntent::SetTitleAudio {
        title_ids: vec!["alpha".to_string()],
        edit: AudioEdit::Format(AudiobookFormat::Mp3),
    })
    .await;
    let renamed = preview_path(&rig);
    assert!(
        renamed.contains("Dune") && renamed.ends_with(".mp3"),
        "{renamed}"
    );
}

#[tokio::test(start_paused = true)]
async fn output_choices_are_recorded_and_template_typing_once_it_pauses() {
    let rig = rig();
    let saved = || async {
        rig.session
            .inner
            .deps
            .settings
            .snapshot()
            .await
            .settings
            .output_defaults
    };

    rig.send(SessionIntent::SetNamingPreset {
        preset: NamingPreset::CustomTemplate,
    })
    .await;
    assert_eq!(
        saved().await.output_naming.preset,
        NamingPreset::CustomTemplate
    );

    for template in ["{a", "{author}", "{author}/{title}x"] {
        rig.send(SessionIntent::SetNamingTemplate {
            template: template.to_string(),
        })
        .await;
    }
    assert_eq!(saved().await.output_naming.custom_template, None);
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    assert_eq!(
        saved().await.output_naming.custom_template.as_deref(),
        Some("{author}/{title}x")
    );
}

#[tokio::test]
async fn ending_a_submission_wakes_a_save_waiting_on_its_sources() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    let draft = rig
        .session
        .transition(|state| {
            state.output.set_directory("/library".to_string());
            state.begin_submission(false)
        })
        .expect("the submission is prepared");
    rig.type_into(MetadataField::Genre, "Mystery").await;
    rig.send(SessionIntent::Save).await;
    assert_eq!(rig.session.waiting_write_paths(), [path("alpha")]);

    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    rig.session
        .end_submission(&draft, SubmissionStatus::Cancelled);

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !rig.session.waiting_write_paths().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the waiting write is attempted once its source is free");
}

fn staged_wav(dir: &std::path::Path, name: &str) -> PathBuf {
    const SAMPLE_RATE: u32 = 44_100;
    let data_len = SAMPLE_RATE * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(44 + data_len as usize, 0);
    let path = dir.join(format!("{name}.wav"));
    std::fs::write(&path, bytes).expect("write staged WAV");
    path
}

fn acquired(job_id: &str, audio: &std::path::Path) -> crate::remote_source::AcquisitionJob {
    let pdf = audio.with_extension("pdf");
    let pdf_bytes = b"%PDF-1.4 companion";
    std::fs::write(&pdf, pdf_bytes).expect("write companion PDF");
    use crate::remote_source::{
        AcquisitionJob, MaterializedSourceFile, ProviderId, RemoteAcquisitionStatus,
        SupplementalAsset,
    };
    AcquisitionJob {
        job_id: job_id.to_string(),
        provider_id: ProviderId::Audible,
        status: RemoteAcquisitionStatus::Validated,
        progress: abb_remote_source_core::acquisition_progress(
            abb_remote_source_core::AcquisitionStage::Complete,
            Some(1.0),
            None,
            None,
        ),
        materialized_files: vec![MaterializedSourceFile {
            input_id: "remote-1".to_string(),
            title_id: "B0".to_string(),
            path: audio.to_path_buf(),
            size_bytes: 0,
            sha256: String::new(),
        }],
        supplemental_assets: vec![SupplementalAsset {
            asset_id: "pdf-1".to_string(),
            input_id: "remote-1".to_string(),
            title_id: "B0".to_string(),
            path: pdf,
            file_name: "Guide.pdf".to_string(),
            size_bytes: pdf_bytes.len() as u64,
            sha256: abb_media_core::sha256_hex(pdf_bytes),
        }],
        diagnostics: Vec::new(),
        handoff: None,
    }
}

impl Rig {
    async fn removed_jobs(&self) -> Vec<String> {
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                let removed = self.removed.lock().expect("removed").clone();
                if !removed.is_empty() && !self.session.inner.sweeping.load(Ordering::SeqCst) {
                    return removed;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("a download is removed")
    }
}

#[tokio::test]
async fn an_acquired_title_is_listed_with_its_companions_and_its_download_goes_when_it_leaves() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");

    let handoff = rig.session.import_acquired(acquired("job-1", &audio)).await;

    assert_eq!(
        handoff,
        crate::remote_source::AcquisitionHandoff::Imported { count: 1 }
    );
    let titles = rig.session.snapshot().titles.expect("titles");
    let input_id = titles.files[0].input_id.clone();
    assert_eq!(titles.companions[&input_id], ["Guide.pdf"]);
    assert!(rig.removed.lock().expect("removed").is_empty());

    rig.send(SessionIntent::ClearAll).await;
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    assert!(rig
        .session
        .snapshot()
        .titles
        .expect("titles")
        .companions
        .is_empty());
}

#[tokio::test]
async fn a_download_with_an_unreadable_file_is_not_listed_at_all() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");
    let mut job = acquired("job-1", &audio);
    let mut missing = job.materialized_files[0].clone();
    missing.path = staging.path().join("part 2.wav");
    job.materialized_files.push(missing);

    let handoff = rig.session.import_acquired(job).await;

    assert!(
        matches!(
            handoff,
            crate::remote_source::AcquisitionHandoff::Removed {
                reason: crate::remote_source::HandoffRefusal::ImportFailed { .. }
            }
        ),
        "{handoff:?}"
    );
    assert!(rig.listed_paths().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_failed_removal_waits_out_the_retry_delay_before_the_next_change_retries_it() {
    let rig = rig();
    rig.failing_removals
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    let attempts = || rig.removed.lock().expect("removed").len();

    rig.send(SessionIntent::ClearAll).await;
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    rig.send(SessionIntent::SetOutputDirectory {
        directory: "/library".to_string(),
    })
    .await;
    tokio::time::sleep(crate::session::staged::RETRY_DELAY / 2).await;
    rig.send(SessionIntent::ClearAll).await;
    assert_eq!(attempts(), 1);

    tokio::time::sleep(crate::session::staged::RETRY_DELAY).await;
    rig.send(SessionIntent::ClearAll).await;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !rig.session.lock().staged.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the change after the delay retries the removal");
    assert_eq!(attempts(), 2);
}

#[tokio::test]
async fn a_download_goes_once_its_title_is_exported() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let output = tempfile::TempDir::new().expect("output");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    assert!(rig.removed.lock().expect("removed").is_empty());

    rig.send(SessionIntent::SetOutputDirectory {
        directory: output.path().to_string_lossy().into_owned(),
    })
    .await;
    rig.type_into(MetadataField::Title, "Book").await;
    rig.send(SessionIntent::Submit).await;
    assert!(
        matches!(
            rig.session.snapshot().output.expect("output").submission,
            Some(SubmissionStatus::Submitted { .. })
        ),
        "{:?}",
        rig.session.snapshot().output.expect("output").submission
    );
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    assert_eq!(
        rig.session.snapshot().titles.expect("titles").files.len(),
        1
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reset_during_preflight_drops_the_review_and_unlocks_the_list() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let out = tempfile::TempDir::new().expect("output");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    rig.send(SessionIntent::SetOutputDirectory {
        directory: out.path().to_string_lossy().into_owned(),
    })
    .await;
    rig.type_into(MetadataField::Title, "Book").await;
    let dest = preview_path(&rig);
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).expect("output parent");
    }
    std::fs::write(&dest, b"already there").expect("colliding output");

    rig.send(SessionIntent::Submit).await;
    assert!(
        matches!(
            output(&rig).submission,
            Some(SubmissionStatus::ReviewRequired)
        ),
        "this output must collide so hold_for_review is reached: {:?}",
        output(&rig).submission
    );
    let review_id = output(&rig)
        .collision_review
        .expect("held review")
        .review_id;
    rig.send(SessionIntent::CancelCollisionReview { review_id })
        .await;
    assert!(!rig.session.snapshot().titles.expect("titles").order_locked);

    let submitting = rig.session.begin(SessionIntent::Submit);
    let _reset = rig.session.begin(SessionIntent::Reset);
    submitting.finish().await;

    let titles = rig.session.snapshot().titles.expect("titles");
    assert!(titles.files.is_empty());
    assert!(
        !titles.order_locked,
        "a reset during preflight unlocks the list"
    );
    assert!(
        output(&rig).collision_review.is_none(),
        "a reset during preflight does not hold a review: {:?}",
        output(&rig).collision_review
    );
    assert!(!output(&rig).submission_in_progress);
}

#[tokio::test]
async fn a_download_that_finishes_while_the_list_is_locked_is_imported_once_it_unlocks() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    let draft = rig
        .session
        .transition(|state| {
            state.output.set_directory("/library".to_string());
            state.begin_submission(false)
        })
        .expect("a submission locks the list");

    let audio = staged_wav(staging.path(), "book");
    let session = rig.session.clone();
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by a_download_that_finishes_while_the_list_is_locked_is_imported_once_it_unlocks"
    )]
    let handoff =
        tokio::spawn(async move { session.import_acquired(acquired("job-1", &audio)).await });
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert!(!handoff.is_finished(), "the download waits for the list");

    rig.session
        .end_submission(&draft, SubmissionStatus::Cancelled);
    assert_eq!(
        handoff.await.expect("handoff"),
        crate::remote_source::AcquisitionHandoff::Imported { count: 1 }
    );
    assert!(rig.removed.lock().expect("removed").is_empty());
}

#[tokio::test]
async fn reset_supersedes_an_acquired_handoff_waiting_for_an_earlier_import() {
    use std::future::Future;
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");
    let in_order = rig
        .session
        .inner
        .imports
        .wait(rig.session.inner.imports.take())
        .await;
    let mut handoff = Box::pin(
        rig.session
            .import_acquired(acquired("job-before-reset", &audio)),
    );
    std::future::poll_fn(|cx| {
        assert!(handoff.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    rig.send(SessionIntent::Reset).await;
    drop(in_order);
    assert_eq!(
        handoff.await,
        crate::remote_source::AcquisitionHandoff::Removed {
            reason: HandoffRefusal::NothingAdded
        }
    );
    assert_eq!(rig.removed_jobs().await, ["job-before-reset"]);
    assert!(rig
        .session
        .snapshot()
        .titles
        .expect("titles")
        .files
        .is_empty());
}

#[tokio::test]
async fn preview_cancel_stops_a_scheduler_wait_and_survives_a_dropped_host_reply() {
    let rig = rig();
    let lane = crate::test_cases::media_fixtures::MediaLane::with_fixtures(&[0.2]);
    let source = lane
        .process(Some(AudiobookMetadata {
            title: Some("Preview".to_string()),
            ..Default::default()
        }))
        .await;
    std::fs::create_dir_all(rig._config.path().join("previews")).expect("preview folder");
    rig.send(SessionIntent::Import {
        paths: vec![source.to_string_lossy().into_owned()],
    })
    .await;
    rig.send(SessionIntent::SetOutputDirectory {
        directory: rig
            ._config
            .path()
            .join("previews")
            .to_string_lossy()
            .into_owned(),
    })
    .await;
    let custom = rig._config.path().join("custom.png");
    image::RgbImage::from_pixel(1, 1, image::Rgb([180, 20, 90]))
        .save(&custom)
        .expect("custom cover");
    rig.send(SessionIntent::LoadCoverFromFile {
        path: custom.to_string_lossy().into_owned(),
    })
    .await;
    let cover = rig.cover_shown().await.expect("accepted normalized cover");
    let jobs = &rig.session.inner.deps.jobs;
    let (first, first_permit) = jobs.register_job().await.expect("hold first slot");
    let (second, second_permit) = jobs.register_job().await.expect("hold second slot");
    let run = rig.session.begin(SessionIntent::Preview);
    drop(run);
    let id = output(&rig)
        .preview_run
        .expect("accepted preview")
        .operation
        .operation_id;
    wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Running).await;
    let offered = output(&rig)
        .preview_run
        .and_then(|run| run.cover)
        .expect("the running preview offers its artwork");
    let shown = rig
        .session
        .cover(&host_request(&offered))
        .await
        .expect("the running preview's artwork")
        .expect("a custom cover");
    assert_eq!(
        shown.to_vec(),
        crate::metadata::render_display_thumbnail(&cover).expect("thumbnail")
    );
    assert_eq!(
        rig.send(SessionIntent::CancelPreview {
            run_id: id.to_string(),
            child_job_id: None
        })
        .await,
        SessionOutcome::Applied
    );
    let preview = wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Cancelled).await;
    assert!(preview.operation.cancel_requested);
    assert!(!preview.open_ready);
    assert!(preview
        .operation
        .children
        .iter()
        .all(|child| child.output_path.is_none()
            && child.progress.stage == crate::work_runtime::WorkProgressStage::Cancelled));
    assert_eq!(
        rig.send(SessionIntent::TakePreviewOutput {
            run_id: id.to_string()
        })
        .await,
        SessionOutcome::PreviewOutput { path: None }
    );
    assert!(!rig.session.snapshot().titles.expect("titles").order_locked);
    jobs.complete_job(first).await;
    jobs.complete_job(second).await;
    drop((first_permit, second_permit));
}

#[tokio::test]
async fn an_unsupported_cover_drop_publishes_the_ingestion_diagnostic() {
    let rig = rig();
    rig.load(&["alpha"]);
    let file = rig._config.path().join("cover.gif");
    std::fs::write(&file, b"GIF89a").expect("unsupported image fixture");
    assert_eq!(
        rig.send(SessionIntent::LoadCoverFromDrop {
            paths: vec![file.to_string_lossy().into_owned()]
        })
        .await,
        SessionOutcome::CoverLoadFailed
    );
    assert!(matches!(
        rig.session
            .snapshot()
            .metadata
            .expect("metadata")
            .cover
            .notice,
        Some(crate::session::CoverNotice::LoadFailed { .. })
    ));
}

#[tokio::test]
async fn an_accepted_indexer_save_finishes_when_the_host_drops_its_reply() {
    use crate::remote_source::{RemoteDraftStatus, RemoteUiIntent};
    let rig = rig();
    rig.send(SessionIntent::Remote {
        intent: RemoteUiIntent::EditConnection {
            base_url: Some("https://proof.test".into()),
            category_ids: None,
            api_key: Some("proof-key".into()),
        },
    })
    .await;
    drop(rig.session.begin(SessionIntent::Remote {
        intent: RemoteUiIntent::SaveConnection,
    }));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if rig
                .session
                .snapshot()
                .remote
                .expect("remote attachment")
                .connection
                .save
                == RemoteDraftStatus::Succeeded
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("accepted Save finished");
    let deps = &rig.session.inner.deps;
    assert_eq!(
        deps.settings.indexer_connection().base_url.as_deref(),
        Some("https://proof.test")
    );
    assert!(deps
        .remote
        .indexer_api_key_configured()
        .await
        .expect("key fact for the saved URL"));
}

async fn wait_preview(
    rig: &Rig,
    status: crate::work_runtime::WorkOperationStatus,
) -> super::super::preview::PreviewSnapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let preview = output(rig).preview_run.expect("retained preview");
            if preview.operation.status == status {
                return preview;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|error| {
        panic!(
            "preview did not reach {status:?}: {error}; {:?}",
            output(rig).preview_run
        )
    })
}

#[tokio::test]
async fn moves_name_the_title_so_a_second_click_moves_the_same_title_again() {
    let rig = rig();
    rig.load(&["alpha", "beta", "gamma"]);
    let up = || SessionIntent::MoveFile {
        title_id: "gamma".to_string(),
        direction: crate::session::MoveDirection::Up,
    };

    rig.send(up()).await;
    rig.send(up()).await;

    let order: Vec<String> = rig
        .session
        .snapshot()
        .titles
        .expect("titles")
        .files
        .into_iter()
        .map(|file| file.input_id)
        .collect();
    assert_eq!(order, ["gamma", "alpha", "beta"]);
}

#[path = "runtime_audio_tests.rs"]
mod audio;
#[path = "runtime_field_edit_tests.rs"]
mod field_edit;
#[path = "runtime/lookup_flow_tests.rs"]
mod lookup_flow;
