//! The session runtime's own behavior: ordering between an intent and its
//! file or network work, and what a host is told along the way. Rules with
//! no I/O are proven in `state_tests.rs`; real files in the engine's
//! integration tests.

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
use crate::session::lookup::LookupSnapshot;
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

/// A session over files that do not exist, with a scripted network.
struct Rig {
    session: Session,
    events: Arc<Events>,
    searches: Arc<StdMutex<Vec<Search>>>,
    /// Replies to hand out, in order. A search with no reply waiting gets an
    /// empty result.
    replies: Arc<StdMutex<VecDeque<tokio::sync::oneshot::Receiver<SearchReply>>>>,
    cover: Arc<StdMutex<Result<Vec<u8>>>>,
    /// Acquisition jobs whose downloads the session removed.
    removed: Arc<StdMutex<Vec<String>>>,
    /// Removals that fail before one succeeds.
    failing_removals: Arc<std::sync::atomic::AtomicUsize>,
    /// Holds the settings the session records defaults into.
    _config: tempfile::TempDir,
}

fn rig() -> Rig {
    rig_with_cover(None)
}

fn rig_with_cover(pending: Option<tokio::sync::oneshot::Receiver<Result<Vec<u8>>>>) -> Rig {
    let events = Arc::new(Events::default());
    let searches = Arc::new(StdMutex::new(Vec::new()));
    let replies: Arc<StdMutex<VecDeque<tokio::sync::oneshot::Receiver<SearchReply>>>> =
        Arc::default();
    let cover: Arc<StdMutex<Result<Vec<u8>>>> = Arc::new(StdMutex::new(Ok(vec![4, 2])));
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
        cover_from_url: Box::new({
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
        }),
    };
    let removed: Arc<StdMutex<Vec<String>>> = Arc::default();
    let failing_removals: Arc<std::sync::atomic::AtomicUsize> = Arc::default();
    let config = tempfile::TempDir::new().expect("settings folder");
    let (settings, _, _) =
        SettingsRuntime::start(config.path().to_path_buf(), PowerManager::default());
    let remote = crate::remote_source::tests::test_runtime(&config);
    let session = Session::with_network(
        SessionDeps {
            remote,
            host: Host::new(
                Arc::clone(&events) as Arc<dyn EventSink>,
                PowerManager::default(),
            ),
            work: WorkRuntime::default(),
            jobs: Arc::new(JobRegistry::new(2)),
            temporary_root: PathBuf::from("/staged"),
            tasks: crate::engine::EngineTasks::default(),
            workspace_root: std::env::temp_dir().join("abb-session-tests"),
            settings,
            remove_staged: {
                let removed = Arc::clone(&removed);
                let failing = Arc::clone(&failing_removals);
                Arc::new(move |job_id| {
                    removed.lock().expect("removed").push(job_id.to_string());
                    let fail = failing
                        .fetch_update(
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
    /// Loads titles without analysis and records what is known of their tags.
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

    /// Locks the list, as a submission or preview does.
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

    async fn select(&self, indices: &[usize]) {
        self.send(SessionIntent::ClearSelection).await;
        for index in indices {
            let outcome = self
                .send(SessionIntent::SelectFile {
                    index: *index,
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

    /// Scripts the next search to wait until the returned sender answers.
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
}

// ---- Lookup ----

#[tokio::test]
async fn lookup_opened_with_no_valid_title_says_so_and_does_not_search() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[]).await;

    rig.send(SessionIntent::LookupOpen).await;

    let lookup = rig.lookup();
    assert!(lookup.open);
    assert_eq!(lookup.status, Some(LookupStatus::NoValidTitle));
    assert!(rig.searches().is_empty());
}

#[tokio::test]
async fn lookup_queues_the_selected_titles_and_searches_for_the_first() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.answer_next_search(Ok(found(&["Found A", "Found B"])));

    rig.send(SessionIntent::LookupOpen).await;

    assert_eq!(
        rig.searches(),
        [(
            "ALPHA Author".to_string(),
            vec![MetadataSource::Audnexus, MetadataSource::Openlibrary]
        )]
    );
    let lookup = rig.lookup();
    assert!(lookup.is_queue_mode);
    assert_eq!(lookup.apply_mode, LookupApplyMode::Queue);
    assert_eq!(
        lookup
            .queue_position
            .map(|at| (at.index, at.total, at.path)),
        Some((0, 2, path("alpha")))
    );
    assert_eq!(
        lookup.status,
        Some(LookupStatus::Found {
            count: 2,
            partial: false,
            after: None
        })
    );
    assert!(lookup.has_searched);
}

#[tokio::test]
async fn the_open_lookup_is_published_before_its_search_answers() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    let answer = rig.hold_next_search();

    let opening = tokio::spawn({
        let session = rig.session.clone();
        async move { session.dispatch(SessionIntent::LookupOpen).await }
    });
    // The search is waiting on `answer`, so what was published came first.
    while rig.searches().is_empty() {
        tokio::task::yield_now().await;
    }
    let published = rig.events.0.lock().expect("events").clone();
    let lookup = published
        .iter()
        .rev()
        .find_map(|update| update.lookup.clone())
        .expect("lookup was published");
    assert!(lookup.open);
    assert_eq!(lookup.status, Some(LookupStatus::Searching));

    answer
        .send(Ok(found(&["Found"])))
        .unwrap_or_else(|_| panic!("search is waiting"));
    opening.await.expect("open finishes");
}

#[tokio::test]
async fn an_empty_search_asks_for_a_query_without_calling_a_provider() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.send(SessionIntent::LookupOpen).await;
    let searched = rig.searches().len();
    rig.send(SessionIntent::LookupSetTitleQuery {
        value: "  ".to_string(),
    })
    .await;
    rig.send(SessionIntent::LookupSetAuthorQuery {
        value: String::new(),
    })
    .await;

    rig.send(SessionIntent::LookupSearch).await;

    assert_eq!(rig.lookup().status, Some(LookupStatus::QueryRequired));
    assert_eq!(rig.searches().len(), searched);
}

#[tokio::test]
async fn a_search_sends_the_chosen_source_and_reports_partial_results() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetSource {
        source: LookupSource::Openlibrary,
    })
    .await;
    rig.answer_next_search(Ok(MetadataLookupResponse {
        diagnostics: vec![MetadataLookupDiagnostic {
            kind: MetadataLookupDiagnosticKind::SourceFailedPartialResults,
            source: Some(MetadataSource::Audnexus),
            message: "Audnexus was unavailable".to_string(),
        }],
        ..found(&["Found"])
    }));

    rig.send(SessionIntent::LookupSearch).await;

    assert_eq!(
        rig.searches().last().map(|(_, sources)| sources.clone()),
        Some(vec![MetadataSource::Openlibrary])
    );
    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::Found {
            count: 1,
            partial: true,
            after: None
        })
    );
}

#[tokio::test]
async fn a_failed_search_is_not_shown_as_no_matches() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.answer_next_search(Err(AppError::General("network down".to_string())));

    rig.send(SessionIntent::LookupSearch).await;

    let lookup = rig.lookup();
    assert_eq!(
        lookup.status,
        Some(LookupStatus::SearchFailed { after: None })
    );
    assert!(lookup.results.is_empty());
    assert!(!lookup.has_searched);
}

#[tokio::test]
async fn a_newer_search_keeps_its_results_when_an_earlier_one_answers_last() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.send(SessionIntent::LookupOpen).await;
    let slow = rig.hold_next_search();
    let earlier = tokio::spawn({
        let session = rig.session.clone();
        async move { session.dispatch(SessionIntent::LookupSearch).await.outcome }
    });
    let searched = rig.searches().len();
    while rig.searches().len() == searched {
        tokio::task::yield_now().await;
    }

    rig.answer_next_search(Ok(found(&["Newer"])));
    rig.send(SessionIntent::LookupSearch).await;
    slow.send(Ok(found(&["Earlier"])))
        .unwrap_or_else(|_| panic!("earlier search is waiting"));

    assert_eq!(
        earlier.await.expect("earlier search"),
        SessionOutcome::Superseded
    );
    assert_eq!(rig.lookup().results[0].title, "Newer");
}

#[tokio::test]
async fn applying_to_the_current_title_edits_its_form_and_stays_on_it() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    let searched = rig.searches().len();

    rig.send(SessionIntent::LookupApply { index: 0 }).await;

    assert_eq!(rig.title_shown(), "Found");
    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::Applied {
            cover_failed: false
        })
    );
    assert_eq!(rig.searches().len(), searched, "no advance, no new search");
    assert_eq!(rig.selected(), [0]);
    // Keeping the title's cover leaves cover intent absent.
    assert!(
        rig.pending("alpha").is_none(),
        "applied values are form edits"
    );
}

#[tokio::test]
async fn applying_in_a_queue_stages_the_edit_moves_on_and_searches_the_next_title() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.answer_next_search(Ok(found(&["Found A"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.answer_next_search(Ok(found(&["Found B"])));

    rig.send(SessionIntent::LookupApply { index: 0 }).await;

    assert_eq!(
        rig.pending("alpha"),
        Some(MetadataIntentPatch {
            title: Some(PatchOp::Set("Found A".to_string())),
            album: Some(PatchOp::Set("Found A".to_string())),
            ..Default::default()
        })
    );
    assert_eq!(rig.selected(), [1]);
    assert_eq!(
        rig.searches().last().map(|(query, _)| query.as_str()),
        Some("BETA Author")
    );
    let lookup = rig.lookup();
    assert_eq!(lookup.queue_position.map(|at| at.index), Some(1));
    assert_eq!(
        lookup.status,
        Some(LookupStatus::Found {
            count: 1,
            partial: false,
            after: Some(QueueStep::Applied)
        })
    );
    assert_eq!(lookup.results[0].title, "Found B");
}

#[tokio::test]
async fn replacing_the_cover_stages_the_result_image_with_the_text() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;

    rig.send(SessionIntent::LookupApply { index: 0 }).await;

    assert_eq!(
        rig.pending("alpha").and_then(|patch| patch.cover_art),
        Some(PatchOp::Set(vec![4, 2]))
    );
    assert_eq!(rig.session.cover_art(), Some(vec![4, 2]));
    assert_eq!(rig.title_shown(), "Found");
}

#[tokio::test]
async fn a_cover_that_fails_to_load_does_not_stop_the_text_from_applying() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.answer_next_search(Ok(found(&["Found A"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    *rig.cover.lock().expect("cover") = Err(AppError::General("unreachable".to_string()));

    rig.send(SessionIntent::LookupApply { index: 0 }).await;

    let alpha = rig.pending("alpha").expect("text applied");
    assert_eq!(alpha.title, Some(PatchOp::Set("Found A".to_string())));
    assert_eq!(alpha.cover_art, None);
    assert!(matches!(
        rig.lookup().status,
        Some(LookupStatus::Found {
            after: Some(QueueStep::AppliedWithoutCover),
            ..
        })
    ));
}

#[tokio::test]
async fn skipping_leaves_the_title_untouched_and_the_last_skip_completes_the_queue() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    rig.send(SessionIntent::LookupOpen).await;

    rig.send(SessionIntent::LookupSkip).await;
    assert!(rig.pending("alpha").is_none());
    assert_eq!(rig.lookup().queue_position.map(|at| at.index), Some(1));
    assert!(matches!(
        rig.lookup().status,
        Some(LookupStatus::Found {
            after: Some(QueueStep::Skipped),
            ..
        })
    ));

    rig.send(SessionIntent::LookupSkip).await;
    assert_eq!(
        rig.lookup().status,
        Some(LookupStatus::QueueComplete {
            cover_failed: false
        })
    );
}

#[tokio::test]
async fn a_result_the_gate_rejects_keeps_the_queue_on_its_title() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    let invalid = OnlineMetadataResult {
        published_date: Some("soon".to_string()),
        ..result("Found A")
    };
    rig.answer_next_search(Ok(MetadataLookupResponse {
        results: vec![invalid],
        diagnostics: Vec::new(),
    }));
    rig.send(SessionIntent::LookupOpen).await;
    let searched = rig.searches().len();

    rig.send(SessionIntent::LookupApply { index: 0 }).await;

    let lookup = rig.lookup();
    assert_eq!(lookup.status, Some(LookupStatus::NextTitleRejected));
    assert_eq!(lookup.queue_position.map(|at| at.index), Some(0));
    assert_eq!(lookup.results.len(), 1, "results stay for another try");
    assert_eq!(rig.searches().len(), searched);
    assert_eq!(rig.selected(), [0]);
    assert!(
        rig.pending("alpha").is_none(),
        "an invalid edit is never staged"
    );
}

#[tokio::test]
async fn a_result_is_not_applied_when_its_title_cannot_be_selected() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    // The user selects another title and leaves an invalid edit on it.
    rig.select(&[1]).await;
    rig.send(SessionIntent::SetField {
        field: MetadataField::Date,
        value: "soon".to_string(),
    })
    .await;

    rig.send(SessionIntent::LookupApply { index: 0 }).await;

    assert_eq!(rig.lookup().status, Some(LookupStatus::ApplyRejected));
    assert_eq!(rig.selected(), [1]);
    assert_eq!(rig.title_shown(), "BETA");
}

#[tokio::test]
async fn an_unknown_result_index_changes_nothing() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found"])));
    rig.send(SessionIntent::LookupOpen).await;
    let before = rig.lookup();

    rig.send(SessionIntent::LookupApply { index: 9 }).await;

    assert_eq!(rig.lookup().status, before.status);
    assert_eq!(rig.title_shown(), "ALPHA");
}

// ---- Intents and what the host is told ----

#[tokio::test]
async fn a_reply_carries_what_its_intent_changed() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;

    let reply = rig
        .session
        .dispatch(SessionIntent::SetField {
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
    rig.send(SessionIntent::SetField {
        field: MetadataField::Date,
        value: "soon".to_string(),
    })
    .await;

    let outcome = rig
        .send(SessionIntent::SelectFile {
            index: 1,
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
    assert!(cover.present && cover.custom && !cover.loading);
    assert_eq!(
        cover.notice,
        Some(crate::session::state::CoverNotice::LoadedFromUrl)
    );
    assert_eq!(rig.session.cover_art(), Some(vec![4, 2]));
}

#[tokio::test]
async fn reset_empties_the_session() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.send(SessionIntent::SetField {
        field: MetadataField::Genre,
        value: "Mystery".to_string(),
    })
    .await;

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

    // The user selects the second title, then types. Each intent's
    // immediate effect applies as it begins; their remaining work is
    // finished in the opposite order.
    let select = rig.session.begin(SessionIntent::SelectFile {
        index: 1,
        modifiers: SelectionModifiers::default(),
    });
    let edit = rig.session.begin(SessionIntent::SetField {
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
    let finished = tokio::spawn(search.finish());
    tokio::task::yield_now().await;

    // A new frontend attaches while the search runs, then the search answers.
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
    // The title had no cover, so nothing is staged and nothing shows.
    assert_eq!(rig.pending("alpha").and_then(|patch| patch.cover_art), None);
    let cover = rig.metadata().cover;
    assert!(!cover.loading && !cover.present);
}

// ---- Audio ----

fn audio(rig: &Rig) -> crate::session::AudioSnapshot {
    rig.session.snapshot().audio.expect("audio part")
}

#[tokio::test]
async fn a_new_title_starts_from_the_default_audio_choice() {
    let rig = rig();
    rig.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Format(AudiobookFormat::MkaOpus),
    })
    .await;
    rig.load(&["alpha"]);

    let title = &audio(&rig).titles["alpha"];
    assert_eq!(title.choice.format, AudiobookFormat::MkaOpus);
}

#[tokio::test]
async fn a_default_audio_edit_is_recorded_in_the_settings_and_announced() {
    let rig = rig();
    rig.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Bitrate(96),
    })
    .await;

    let settings = rig.session.inner.deps.settings.snapshot().await;
    let saved = settings.settings.encoder_defaults;
    assert_eq!(saved.settings.bitrate_kbps, 96);
    assert_eq!(saved.intent, AudioIntent::Encode);
    let announced = rig.events.1.lock().expect("events").last().cloned();
    assert_eq!(
        announced.map(|snapshot| snapshot.revision),
        Some(settings.revision)
    );

    // An edit that changes nothing records nothing.
    let before = rig.events.1.lock().expect("events").len();
    rig.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Bitrate(96),
    })
    .await;
    assert_eq!(rig.events.1.lock().expect("events").len(), before);
}

#[tokio::test]
async fn title_audio_edits_change_only_the_named_titles_and_not_while_locked() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);

    rig.send(SessionIntent::SetTitleAudio {
        title_ids: vec!["alpha".to_string()],
        edit: AudioEdit::Format(AudiobookFormat::Mp3),
    })
    .await;
    let titles = audio(&rig).titles;
    assert_eq!(titles["alpha"].choice.intent, AudioIntent::Preserve);
    assert_eq!(titles["beta"].choice.format, AudiobookFormat::M4b);

    rig.lock_order(true);
    rig.send(SessionIntent::ApplyDefaultAudio {
        title_ids: vec!["alpha".to_string()],
    })
    .await;
    assert_eq!(
        audio(&rig).titles["alpha"].choice.format,
        AudiobookFormat::Mp3
    );

    rig.lock_order(false);
    rig.send(SessionIntent::ApplyDefaultAudio {
        title_ids: vec!["alpha".to_string()],
    })
    .await;
    assert_eq!(
        audio(&rig).titles["alpha"].choice.format,
        AudiobookFormat::M4b
    );
    // The defaults themselves never moved.
    assert_eq!(audio(&rig).defaults.choice.format, AudiobookFormat::M4b);
}

#[tokio::test]
async fn a_batch_audio_edit_applies_to_every_title_or_none() {
    use crate::audio::{EncoderType, FaacProfile, SampleRateConfig};
    use crate::session::{AudioField, AudioRefusal};

    let rig = rig();
    rig.load(&["alpha", "beta"]);
    let both = vec!["alpha".to_string(), "beta".to_string()];
    let set = |title_ids: Vec<String>, edit| SessionIntent::SetTitleAudio { title_ids, edit };
    rig.send(set(both.clone(), AudioEdit::Encoder(EncoderType::Faac)))
        .await;
    rig.send(set(
        vec!["alpha".to_string()],
        AudioEdit::FaacProfile(FaacProfile::HeAacV1),
    ))
    .await;
    rig.send(set(
        vec!["beta".to_string()],
        AudioEdit::FaacProfile(FaacProfile::AacLc),
    ))
    .await;
    let titles = audio(&rig).titles;
    let only_lc = titles["beta"]
        .facts
        .allowed_sample_rates
        .iter()
        .copied()
        .find(|rate| !titles["alpha"].facts.allowed_sample_rates.contains(rate))
        .expect("a rate only AAC-LC accepts");
    let beta_rate = titles["beta"].choice.sample_rate;

    rig.send(set(
        both.clone(),
        AudioEdit::SampleRate(SampleRateConfig::Explicit(only_lc)),
    ))
    .await;
    let after = audio(&rig);
    assert_eq!(
        after.titles["beta"].choice.sample_rate, beta_rate,
        "nothing changed"
    );
    assert_eq!(
        after.refusal,
        Some(AudioRefusal::NotAccepted {
            title_ids: vec!["alpha".to_string()]
        })
    );

    rig.select(&[0, 1]).await;
    let selection = audio(&rig).selection.expect("selection audio");
    assert_eq!(selection.title_ids, both);
    assert!(!selection.facts.allowed_sample_rates.contains(&only_lc));
    assert!(selection.facts.allowed_sample_rates.contains(&44_100));
    assert!(selection.mixed.contains(&AudioField::FaacProfile));

    rig.send(set(both.clone(), AudioEdit::Encoder(EncoderType::Faac)))
        .await;
    assert_eq!(
        audio(&rig).refusal,
        None,
        "a value already set is not a refusal"
    );

    rig.lock_order(true);
    rig.send(SessionIntent::ApplyDefaultAudio { title_ids: both })
        .await;
    assert_eq!(audio(&rig).refusal, Some(AudioRefusal::Locked));
}

// ---- Output and plans ----

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

    rig.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Dune".to_string(),
    })
    .await;
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
async fn a_title_plan_resolves_in_the_background_and_reports_why_it_cannot() {
    let rig = rig();
    rig.load(&["alpha"]);
    // The rig's titles have no audio facts, so the plan cannot resolve.
    rig.send(SessionIntent::SetTitleAudio {
        title_ids: vec!["alpha".to_string()],
        edit: AudioEdit::Intent(AudioIntent::Encode),
    })
    .await;

    let plan = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let plan = audio(&rig).titles["alpha"].plan.clone();
            if plan != crate::session::TitlePlan::Pending {
                return plan;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the plan resolves");
    assert!(
        matches!(plan, crate::session::TitlePlan::Failed { .. }),
        "{plan:?}"
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
            state.begin_submission(None)
        })
        .expect("the submission is prepared");
    rig.send(SessionIntent::SetField {
        field: MetadataField::Genre,
        value: "Mystery".to_string(),
    })
    .await;
    rig.send(SessionIntent::Save).await;
    assert_eq!(rig.session.waiting_write_paths(), [path("alpha")]);

    // Let the writer find the file held and wait.
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    // Nothing in WorkRuntime changes; only the submission ending frees the file.
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

/// A one-second silent WAV, as an acquisition would stage it.
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
    // Later changes inside the delay leave the failed download alone.
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
async fn a_download_goes_once_its_title_is_exported_and_nothing_imported_is_refused() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let output = tempfile::TempDir::new().expect("output");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;

    // The same files again add nothing, so that job's download is refused.
    let again = rig.session.import_acquired(acquired("job-2", &audio)).await;
    assert_eq!(
        again,
        crate::remote_source::AcquisitionHandoff::Removed {
            reason: crate::remote_source::HandoffRefusal::NothingAdded
        }
    );
    // The session removes the refused job's download itself.
    assert_eq!(rig.removed_jobs().await, ["job-2"]);
    rig.removed.lock().expect("removed").clear();

    rig.send(SessionIntent::SetOutputDirectory {
        directory: output.path().to_string_lossy().into_owned(),
    })
    .await;
    rig.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Book".to_string(),
    })
    .await;
    rig.send(SessionIntent::Submit).await;
    assert!(
        matches!(
            rig.session.snapshot().output.expect("output").submission,
            Some(SubmissionStatus::Submitted { .. })
        ),
        "{:?}",
        rig.session.snapshot().output.expect("output").submission
    );
    // The title stays listed; its export completed, so the download goes.
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    assert_eq!(
        rig.session.snapshot().titles.expect("titles").files.len(),
        1
    );
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
            state.begin_submission(None)
        })
        .expect("a submission locks the list");

    let audio = staged_wav(staging.path(), "book");
    let session = rig.session.clone();
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
    assert!(rig
        .session
        .snapshot()
        .titles
        .expect("titles")
        .files
        .is_empty());
}

#[tokio::test]
async fn lookup_cover_reply_cannot_pull_the_selection_back_to_an_earlier_title() {
    let (cover, pending) = tokio::sync::oneshot::channel();
    let rig = rig_with_cover(Some(pending));
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    rig.answer_next_search(Ok(found(&["Found Alpha"])));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    let applying = rig.session.begin(SessionIntent::LookupApply { index: 0 });
    rig.select(&[1]).await;
    cover.send(Ok(vec![1, 2, 3])).expect("cover reply");
    assert_eq!(applying.finish().await.outcome, SessionOutcome::Superseded);
    assert_eq!(rig.selected(), vec![1]);
    assert_eq!(rig.title_shown(), "BETA");
    assert!(rig.pending("alpha").is_none());
}

#[tokio::test]
async fn preview_cancel_stops_a_scheduler_wait_and_survives_a_dropped_host_reply() {
    let rig = rig();
    let lane =
        crate::test_cases::integration_media_execution_tests::MediaLane::with_fixtures(&[0.2]);
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
    let cover = rig.session.cover_art().expect("accepted normalized cover");
    let jobs = &rig.session.inner.deps.jobs;
    let (first, first_permit) = jobs.register_job().await.expect("hold first slot");
    let (second, second_permit) = jobs.register_job().await.expect("hold second slot");
    let run = rig.session.begin(SessionIntent::Preview { seconds: 0.1 });
    drop(run);
    let id = output(&rig)
        .preview_run
        .expect("accepted preview")
        .operation
        .operation_id;
    wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Running).await;
    assert_eq!(
        rig.send(SessionIntent::ReadPreviewCover {
            run_id: id.to_string()
        })
        .await,
        SessionOutcome::PreviewCover { bytes: Some(cover) }
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
            if rig.session.inner.deps.remote.ui_snapshot().connection.save
                == RemoteDraftStatus::Succeeded
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("accepted Save finished");
    let saved = rig
        .session
        .inner
        .deps
        .remote
        .get_indexer_connection()
        .await
        .expect("persisted accepted connection");
    assert_eq!(saved.base_url.as_deref(), Some("https://proof.test"));
    assert!(saved.api_key_configured);
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

    // Both clicks were sent before either was answered.
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

#[tokio::test]
async fn accepted_auth_work_survives_a_dropped_host_reply_and_reattachment_keeps_status() {
    use crate::remote_source::{RemoteAuthStatus, RemoteUiIntent};
    let rig = rig();
    drop(rig.session.begin(SessionIntent::Remote {
        intent: RemoteUiIntent::StartAuth,
    }));
    tokio::time::timeout(Duration::from_secs(5), async {
        while rig
            .session
            .snapshot()
            .remote
            .expect("remote attachment")
            .auth
            != RemoteAuthStatus::AwaitingHandoff
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("accepted start completed without host wait");
    drop(
        rig.session.begin(SessionIntent::Remote {
            intent: RemoteUiIntent::CompleteAuth {
                response_url_handoff_path: Some(
                    rig._config
                        .path()
                        .join("missing-auth-handoff")
                        .to_string_lossy()
                        .into_owned(),
                ),
            },
        }),
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(
            rig.session
                .snapshot()
                .remote
                .expect("remote attachment")
                .auth,
            RemoteAuthStatus::Failed { .. }
        ) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("accepted completion reports failure without host wait");
}
