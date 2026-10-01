//! The working session driven the way a host drives it: intents in, snapshots
//! out, real files on disk, no UI.
//!
//! These prove what the session's unit tests cannot: that import fills the
//! form from real tags, that Save reaches the file, and that Save respects an
//! export that is still reading its source.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use abb_engine::audio::{AudioIntent, AudiobookFormat, SampleRateConfig, TitleAudioRequest};
use abb_engine::processing::ProcessPayload;
use abb_engine::session::{
    MetadataField, MetadataSnapshot, MetadataStatus, SessionIntent, SessionOutcome,
};
use abb_engine::work_runtime::{SubmitProcessingOperationRequest, WorkOperationStatus};
use abb_engine::{read_metadata, AudiobookMetadata, Engine, EngineConfig, PatchOp};
use tempfile::TempDir;

use super::integration_media_execution_tests::{native_encoder_settings, MediaLane};

/// One engine over its own throwaway roots, with one tagged audiobook.
struct Desk {
    root: TempDir,
    engine: Engine,
}

impl Desk {
    fn new() -> Self {
        let root = TempDir::new().expect("engine root");
        let engine = Engine::start(EngineConfig {
            cache_dir: root.path().join("cache"),
            config_dir: root.path().join("config"),
            app_identifier: "com.audiobook-boss.test".to_string(),
            events: Arc::new(abb_engine::DiscardEvents),
            aaxclean_helper: None,
        })
        .expect("start headless engine");
        Self { root, engine }
    }

    /// Builds an M4B tagged "Alpha" by "Source Author" at `destination`.
    async fn audiobook(&self, destination: &Path, seconds: f64) -> PathBuf {
        let lane = MediaLane::with_fixtures(&[seconds]);
        let built = lane
            .process(Some(AudiobookMetadata {
                title: Some("Alpha".to_string()),
                album: Some("Alpha".to_string()),
                artist: Some("Source Author".to_string()),
                genre: Some("Fantasy".to_string()),
                ..Default::default()
            }))
            .await;
        fs::create_dir_all(destination.parent().expect("parent")).expect("create folder");
        fs::copy(built, destination).expect("place audiobook");
        destination.to_path_buf()
    }

    async fn send(&self, intent: SessionIntent) -> SessionOutcome {
        self.engine.session_dispatch(intent).await.outcome
    }

    async fn import(&self, path: &Path) {
        let outcome = self
            .send(SessionIntent::Import {
                paths: vec![path.to_string_lossy().into_owned()],
            })
            .await;
        assert_eq!(outcome, SessionOutcome::Applied);
    }

    async fn edit_genre_and_save(&self) -> MetadataSnapshot {
        self.send(SessionIntent::SetField {
            field: MetadataField::Genre,
            value: "Mystery".to_string(),
        })
        .await;
        assert_eq!(
            self.send(SessionIntent::Save).await,
            SessionOutcome::Applied
        );
        self.metadata()
    }

    fn metadata(&self) -> MetadataSnapshot {
        self.engine
            .session_snapshot()
            .metadata
            .expect("metadata part")
    }

    fn shown(&self, field: MetadataField) -> String {
        self.metadata()
            .form
            .fields
            .into_iter()
            .find(|snapshot| snapshot.field == field)
            .expect("field")
            .value
    }

    /// Accepts an export of `source` and returns once the engine has it.
    async fn export(&self, source: &Path) -> abb_engine::work_runtime::OperationId {
        let output = self.root.path().join("exports");
        fs::create_dir_all(&output).expect("create export folder");
        let accepted = self
            .engine
            .submit_processing_operation(SubmitProcessingOperationRequest {
                payload: ProcessPayload {
                    input_files: vec![source.to_string_lossy().into_owned()],
                    title_sources: None,
                    chapter_plans: None,
                    input_ids: None,
                    output_dir: output.to_string_lossy().into_owned(),
                    audio_requests: vec![audio_request()],
                    output_naming: None,
                    collision_policy: None,
                    preflight_signature: None,
                    supplemental_assets_by_input_id: None,
                },
                metadata: None,
                preview_seconds: None,
                title: "Alpha".to_string(),
            })
            .await
            .expect("export accepted");
        accepted.operation_id
    }

    fn export_status(
        &self,
        operation: &abb_engine::work_runtime::OperationId,
    ) -> WorkOperationStatus {
        self.engine
            .list_work_operations()
            .expect("operations")
            .operations
            .into_iter()
            .find(|snapshot| &snapshot.operation_id == operation)
            .expect("export is listed")
            .status
    }

    async fn wait_until(&self, what: &str, done: impl Fn(&Self) -> bool) {
        for _ in 0..600 {
            if done(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("timed out waiting until {what}");
    }
}

fn audio_request() -> TitleAudioRequest {
    TitleAudioRequest {
        format: AudiobookFormat::M4b,
        intent: AudioIntent::Encode,
        settings: Some(native_encoder_settings()),
        sample_rate: SampleRateConfig::Auto,
    }
}

fn genre_on_disk(path: &Path) -> Option<String> {
    read_metadata(path.to_string_lossy().as_ref())
        .expect("read tags back")
        .genre
}

fn finished(status: WorkOperationStatus) -> bool {
    matches!(
        status,
        WorkOperationStatus::Completed
            | WorkOperationStatus::Failed
            | WorkOperationStatus::Cancelled
            | WorkOperationStatus::Mixed
    )
}

#[tokio::test]
async fn importing_one_audiobook_selects_it_and_shows_its_tags() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;

    desk.import(&book).await;

    let snapshot = desk.engine.session_snapshot();
    let titles = snapshot.titles.expect("titles part");
    assert_eq!(titles.files.len(), 1);
    assert!(titles.files[0].is_valid);
    let audio = snapshot.audio.expect("audio part");
    assert!(audio.titles.contains_key(&titles.files[0].input_id));
    assert_eq!(
        snapshot.selection.expect("selection part").selected_indices,
        [0]
    );
    assert_eq!(desk.shown(MetadataField::Title), "Alpha");
    assert_eq!(desk.shown(MetadataField::Author), "Source Author");

    // Importing the same file again adds nothing.
    desk.import(&book).await;
    assert_eq!(
        desk.engine
            .session_snapshot()
            .titles
            .expect("titles part")
            .files
            .len(),
        1
    );
}

#[tokio::test]
async fn save_with_no_export_running_writes_the_file_in_place() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;
    desk.import(&book).await;

    let metadata = desk.edit_genre_and_save().await;

    assert_eq!(
        metadata.status,
        Some(MetadataStatus::SaveComplete {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            waiting: 0,
            held: 0
        })
    );
    assert!(!metadata.save_in_progress && !metadata.has_pending_edits);
    assert!(book.exists(), "the file stays where it was");
    assert_eq!(genre_on_disk(&book).as_deref(), Some("Mystery"));
    let tags = read_metadata(book.to_string_lossy().as_ref()).expect("read tags back");
    assert_eq!(
        tags.artist.as_deref(),
        Some("Source Author"),
        "untouched tags stay"
    );

    // The save ran as one accepted operation with one outcome per file.
    let operations = desk.engine.list_work_operations().expect("operations");
    let save = operations.operations.first().expect("the save is listed");
    assert_eq!(save.status, WorkOperationStatus::Completed);
    assert_eq!(save.children.len(), 1);
}

#[tokio::test]
async fn save_on_a_local_source_in_flight_is_written_after_its_export_finishes() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 12.0)
        .await;
    desk.import(&book).await;
    let export = desk.export(&book).await;

    let metadata = desk.edit_genre_and_save().await;

    assert_eq!(
        metadata.status,
        Some(MetadataStatus::SaveComplete {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            waiting: 1,
            held: 0
        })
    );
    assert_eq!(metadata.waiting_writes.len(), 1);

    desk.wait_until("the waiting write is applied", |desk| {
        desk.metadata().status
            == Some(MetadataStatus::DeferredWritesFinished {
                written: 1,
                failed: 0,
            })
    })
    .await;

    assert_eq!(
        desk.export_status(&export),
        WorkOperationStatus::Completed,
        "the write waited for the export"
    );
    assert_eq!(genre_on_disk(&book).as_deref(), Some("Mystery"));
    assert!(!desk.metadata().has_pending_edits);
    assert!(desk.engine.waiting_metadata_writes().is_empty());
}

#[tokio::test]
async fn save_on_a_temporary_source_in_flight_never_writes_the_download() {
    let desk = Desk::new();
    // Where the engine stages downloads.
    let staged = desk
        .root
        .path()
        .join("cache/remote-source/sessions/job-1/alpha.m4b");
    let book = desk.audiobook(&staged, 12.0).await;
    desk.import(&book).await;
    let before = fs::read(&book).expect("read download");
    let export = desk.export(&book).await;

    let metadata = desk.edit_genre_and_save().await;

    assert_eq!(
        metadata.status,
        Some(MetadataStatus::SaveComplete {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            waiting: 0,
            held: 1
        })
    );
    assert!(metadata.waiting_writes.is_empty());

    desk.wait_until("the export finishes", |desk| {
        finished(desk.export_status(&export))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(fs::read(&book).expect("read download"), before);
    // The edit is kept for the title's output.
    let loaded = desk.engine.session_snapshot().titles.expect("titles part");
    let pending = desk
        .engine
        .session_metadata_intents(&[loaded.files[0].path.to_string_lossy().into_owned()]);
    assert_eq!(
        pending
            .values()
            .next()
            .and_then(|patch| patch.genre.clone()),
        Some(PatchOp::Set("Mystery".to_string()))
    );
}

#[tokio::test]
async fn the_developer_tool_imports_edits_and_saves_a_real_file() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;

    let run = std::process::Command::new(env!("CARGO_BIN_EXE_abb-dev"))
        .arg(&book)
        .args(["--set", "genre=Mystery", "--save", "--json"])
        .arg("--state-dir")
        .arg(desk.root.path().join("tool-state"))
        .output()
        .expect("run abb-dev");

    assert!(
        run.status.success(),
        "abb-dev failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let session: serde_json::Value =
        serde_json::from_slice(&run.stdout).expect("abb-dev prints the session as JSON");
    assert_eq!(session["titles"]["files"].as_array().map(Vec::len), Some(1));
    assert_eq!(session["metadata"]["status"]["kind"], "saveComplete");
    assert_eq!(session["metadata"]["status"]["succeeded"], 1);
    assert_eq!(genre_on_disk(&book).as_deref(), Some("Mystery"));
}

#[tokio::test]
async fn audio_and_output_defaults_are_saved_and_return_after_a_settings_reset() {
    let desk = Desk::new();
    desk.send(SessionIntent::SetDefaultAudio {
        edit: abb_engine::session::AudioEdit::Format(AudiobookFormat::MkaOpus),
    })
    .await;
    desk.send(SessionIntent::SetOutputDirectory {
        directory: "/library".to_string(),
    })
    .await;

    let saved = desk
        .engine
        .settings_snapshot()
        .await
        .settings
        .expect("settings");
    assert_eq!(saved.encoder_defaults.format, AudiobookFormat::MkaOpus);
    assert_eq!(
        saved.output_defaults.output_directory.as_deref(),
        Some("/library")
    );

    desk.engine
        .settings_dispatch(abb_engine::app_settings::SettingsIntent::Reset)
        .await;

    let snapshot = desk.engine.session_snapshot();
    let audio = snapshot.audio.expect("audio part");
    assert_eq!(audio.defaults.choice.format, AudiobookFormat::M4b);
    assert_eq!(snapshot.output.expect("output part").directory, None);
}
