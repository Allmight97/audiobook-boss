//! The session runtime's own behavior: ordering between an intent and its
//! file or network work, and what a host is told along the way. Rules with
//! no I/O are proven in `state_tests.rs`; real files in the engine's
//! integration tests.

use std::collections::VecDeque;
use std::sync::Mutex as StdMutex;

use super::*;
use crate::audio::{AudioFile, AudioIntent, AudiobookFormat, SampleRateConfig};
use crate::host::EventSink;
use crate::metadata::PatchOp;
use crate::metadata_lookup::{
    MetadataLookupDiagnostic, MetadataLookupDiagnosticKind, OnlineMetadataResult,
};
use crate::power::PowerManager;
use crate::processing::JobRegistry;
use crate::session::lookup::LookupSnapshot;
use crate::session::state::MetadataSnapshot;

type SearchReply = Result<MetadataLookupResponse>;
/// A search as the provider saw it: the query and the sources asked.
type Search = (String, Vec<MetadataSource>);

#[derive(Default)]
struct Events(StdMutex<Vec<SessionUpdate>>);

impl EventSink for Events {
    fn emit(&self, event: EngineEvent) {
        if let EngineEvent::Session(update) = event {
            self.0.lock().expect("events").push(update);
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
}

fn rig() -> Rig {
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
            move |_url| {
                let result = match &*cover.lock().expect("cover") {
                    Ok(bytes) => Ok(bytes.clone()),
                    Err(error) => Err(AppError::General(error.to_string())),
                };
                Box::pin(async move { result })
            }
        }),
    };
    let session = Session::with_network(
        SessionDeps {
            host: Host::new(
                Arc::clone(&events) as Arc<dyn EventSink>,
                PowerManager::default(),
            ),
            work: WorkRuntime::default(),
            jobs: Arc::new(JobRegistry::new(2)),
            temporary_root: PathBuf::from("/staged"),
            opened_audio: Arc::default(),
            previews: Arc::default(),
        },
        network,
    );
    Rig {
        session,
        events,
        searches,
        replies,
        cover,
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
            state.working_set.append_analyzed(
                files,
                &TitleAudioRequest {
                    format: AudiobookFormat::M4b,
                    intent: AudioIntent::Auto,
                    settings: None,
                    sample_rate: SampleRateConfig::Auto,
                },
            );
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
            .pending_intents(&[path(name)])
            .into_values()
            .next()
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
        default_audio: TitleAudioRequest {
            format: AudiobookFormat::M4b,
            intent: AudioIntent::Auto,
            settings: None,
            sample_rate: SampleRateConfig::Auto,
        },
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
    let save = rig.session.begin(SessionIntent::StageSelection);
    assert_eq!(save.finish().await.outcome, SessionOutcome::Applied);
    edit.finish().await;
    select.finish().await;

    assert_eq!(rig.selected(), [1]);
    assert!(rig.pending("alpha").is_none());
    assert_eq!(
        rig.pending("beta").and_then(|patch| patch.genre),
        Some(PatchOp::Set("Mystery".to_string()))
    );
}
