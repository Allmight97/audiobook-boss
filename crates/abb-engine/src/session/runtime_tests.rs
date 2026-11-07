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
use crate::session::submission::SubmissionStatus;
use crate::session::working_set::InputNotice;

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
async fn clear_all_empties_the_session() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    rig.type_into(MetadataField::Genre, "Mystery").await;

    rig.send(SessionIntent::ClearAll).await;

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

fn audio(rig: &Rig) -> crate::session::AudioSnapshot {
    rig.session.snapshot().audio.expect("audio part")
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
#[path = "runtime/cover_flow_tests.rs"]
mod cover_flow;
#[path = "runtime/defaults_flow_tests.rs"]
mod defaults_flow;
#[path = "runtime/download_flow_tests.rs"]
mod download_flow;
#[path = "runtime_field_edit_tests.rs"]
mod field_edit;
#[path = "runtime/import_flow_tests.rs"]
mod import_flow;
#[path = "runtime/lookup_flow_tests.rs"]
mod lookup_flow;
#[path = "runtime/preview_flow_tests.rs"]
mod preview_flow;
#[path = "runtime/save_flow_tests.rs"]
mod save_flow;
#[path = "runtime/submission_flow_tests.rs"]
mod submission_flow;
