use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use abb_engine::audio::{AudioIntent, EncoderType};
use abb_engine::session::{
    AudioEdit, MetadataField, MetadataSnapshot, SessionIntent, SessionOutcome, SubmissionStatus,
    TitlePlan,
};
use abb_engine::work_runtime::WorkOperationStatus;
use abb_engine::{read_metadata, AudiobookMetadata, Engine, EngineConfig};
use tempfile::TempDir;

use super::integration_media_execution_tests::MediaLane;

pub(super) struct Desk {
    pub(super) root: TempDir,
    pub(super) engine: Engine,
}

pub(super) fn engine_config(root: &Path) -> EngineConfig {
    EngineConfig {
        cache_dir: root.join("cache"),
        config_dir: root.join("config"),
        app_identifier: "com.audiobook-boss.test".to_string(),
        events: Arc::new(abb_engine::DiscardEvents),
        aaxclean_helper: None,
    }
}

impl Desk {
    pub(super) fn new() -> Self {
        let root = TempDir::new().expect("engine root");
        let engine = Engine::start(engine_config(root.path())).expect("start headless engine");
        Self { root, engine }
    }

    pub(super) fn relaunch(self) -> Self {
        let Self { root, engine } = self;
        drop(engine);
        let engine = start_once_the_lock_frees(|| engine_config(root.path()));
        Self { root, engine }
    }

    pub(super) fn residual_workspace_dirs(&self) -> Vec<PathBuf> {
        let workspace = crate::audio::processing_workspace_root(&self.root.path().join("cache"));
        if !workspace.exists() {
            return Vec::new();
        }
        fs::read_dir(workspace)
            .expect("read workspace root")
            .map(|entry| entry.expect("workspace entry").path())
            .collect()
    }

    pub(super) async fn audiobook(&self, destination: &Path, seconds: f64) -> PathBuf {
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

    pub(super) async fn send(&self, intent: SessionIntent) -> SessionOutcome {
        self.engine.session_dispatch(intent).await.outcome
    }

    pub(super) async fn import(&self, path: &Path) {
        let outcome = self
            .send(SessionIntent::Import {
                paths: vec![path.to_string_lossy().into_owned()],
            })
            .await;
        assert_eq!(outcome, SessionOutcome::Applied);
    }

    pub(super) async fn edit_genre_and_save(&self) -> MetadataSnapshot {
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

    pub(super) fn metadata(&self) -> MetadataSnapshot {
        self.engine
            .session_snapshot()
            .metadata
            .expect("metadata part")
    }

    pub(super) fn shown(&self, field: MetadataField) -> String {
        self.metadata()
            .form
            .fields
            .into_iter()
            .find(|snapshot| snapshot.field == field)
            .expect("field")
            .value
    }

    pub(super) async fn export(&self) -> abb_engine::work_runtime::OperationId {
        let output = self.root.path().join("exports");
        fs::create_dir_all(&output).expect("create export folder");
        self.send(SessionIntent::SetOutputDirectory {
            directory: output.to_string_lossy().into_owned(),
        })
        .await;
        let title_ids = self
            .engine
            .session_snapshot()
            .titles
            .expect("titles")
            .files
            .into_iter()
            .map(|file| file.input_id)
            .collect::<Vec<_>>();
        for edit in [
            AudioEdit::Intent(AudioIntent::Encode),
            AudioEdit::Encoder(EncoderType::NativeAac),
        ] {
            self.send(SessionIntent::SetTitleAudio {
                title_ids: title_ids.clone(),
                edit,
            })
            .await;
        }
        self.send(SessionIntent::Submit).await;
        match submission(self) {
            Some(SubmissionStatus::Submitted { operation_id, .. }) => operation_id,
            other => panic!("export not accepted: {other:?}"),
        }
    }

    pub(super) fn export_status(
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

    pub(super) fn output_of(&self, operation: &abb_engine::work_runtime::OperationId) -> PathBuf {
        let operations = self.engine.list_work_operations().expect("operations");
        let export = operations
            .operations
            .into_iter()
            .find(|snapshot| &snapshot.operation_id == operation)
            .expect("export is listed");
        PathBuf::from(
            export.children[0]
                .output_path
                .clone()
                .expect("the title published"),
        )
    }

    pub(super) fn plan(&self) -> TitlePlan {
        let audio = self.engine.session_snapshot().audio.expect("audio part");
        audio.titles.values().next().expect("a title").plan.clone()
    }

    pub(super) async fn wait_until(&self, what: &str, done: impl Fn(&Self) -> bool) {
        for _ in 0..600 {
            if done(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("timed out waiting until {what}");
    }
}

/// A child process another test thread spawns copies this process's
/// descriptor table, so it holds the folder's flock until its exec, about a
/// millisecond.
pub(super) fn start_once_the_lock_frees(config: impl Fn() -> EngineConfig) -> Engine {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        match Engine::start(config()) {
            Ok(engine) => return engine,
            Err(abb_engine::AppError::AlreadyRunning(_))
                if std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("ABB starts after the first one quit: {error}"),
        }
    }
}

pub(super) fn genre_on_disk(path: &Path) -> Option<String> {
    read_metadata(path.to_string_lossy().as_ref())
        .expect("read tags back")
        .genre
}

pub(super) fn finished(status: WorkOperationStatus) -> bool {
    matches!(
        status,
        WorkOperationStatus::Completed
            | WorkOperationStatus::Failed
            | WorkOperationStatus::Cancelled
            | WorkOperationStatus::Mixed
    )
}

pub(super) fn submission(desk: &Desk) -> Option<abb_engine::session::SubmissionStatus> {
    desk.engine
        .session_snapshot()
        .output
        .expect("output part")
        .submission
}

pub(super) fn collision_review(desk: &Desk) -> abb_engine::session::CollisionReview {
    desk.engine
        .session_snapshot()
        .output
        .expect("output part")
        .collision_review
        .expect("held collision review")
}
