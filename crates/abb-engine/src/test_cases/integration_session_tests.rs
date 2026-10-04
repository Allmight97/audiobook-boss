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

use abb_engine::audio::{AudioIntent, AudiobookFormat, EncoderType};
use abb_engine::session::{
    AudioEdit, MetadataField, MetadataSnapshot, MetadataStatus, OutputEdits, SessionIntent,
    SessionOutcome, SubmissionStatus, SubmitRefusal,
};
use abb_engine::work_runtime::WorkOperationStatus;
use abb_engine::{read_metadata, AudiobookMetadata, Engine, EngineConfig};
use tempfile::TempDir;

use super::integration_media_execution_tests::MediaLane;

#[tokio::test]
async fn shutdown_finishes_an_accepted_save_without_a_host_waiting_for_its_reply() {
    let desk = Desk::new();
    let source = desk
        .audiobook(&desk.root.path().join("source.m4b"), 0.2)
        .await;
    desk.import(&source).await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Genre,
        value: "Mystery".into(),
    })
    .await;
    let reply = desk.engine.session_begin(SessionIntent::Save);
    drop(reply);
    desk.engine.shutdown().await;
    assert_eq!(
        read_metadata(source.to_str().expect("path"))
            .expect("tags")
            .genre
            .as_deref(),
        Some("Mystery")
    );
    assert!(!desk.metadata().save_in_progress);
}

#[tokio::test]
async fn defaults_are_saved_in_acceptance_order_even_when_replies_are_awaited_backwards() {
    let desk = Desk::new();
    let first = desk
        .engine
        .session_begin(SessionIntent::SetOutputDirectory {
            directory: "/first".into(),
        });
    let second = desk
        .engine
        .session_begin(SessionIntent::SetOutputDirectory {
            directory: "/second".into(),
        });
    second.finish().await;
    first.finish().await;
    assert_eq!(
        desk.engine
            .settings_snapshot()
            .await
            .settings
            .output_defaults
            .output_directory
            .as_deref(),
        Some("/second")
    );
    desk.engine.shutdown().await;
}

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

    /// Submits the session as a native AAC export and returns once the
    /// engine has accepted it.
    async fn export(&self) -> abb_engine::work_runtime::OperationId {
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

    /// The published audiobook of an export's first title.
    fn output_of(&self, operation: &abb_engine::work_runtime::OperationId) -> PathBuf {
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
            held: 0,
            outputs: Default::default(),
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
    let export = desk.export().await;

    let metadata = desk.edit_genre_and_save().await;

    assert_eq!(
        metadata.status,
        Some(MetadataStatus::SaveComplete {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            waiting: 1,
            held: 0,
            outputs: OutputEdits {
                updated: 1,
                ..OutputEdits::default()
            },
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
    assert_eq!(desk.engine.running_work().waiting_writes, 0);
    // The export's output took the edit too.
    assert_eq!(
        genre_on_disk(&desk.output_of(&export)).as_deref(),
        Some("Mystery")
    );
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
    let export = desk.export().await;

    let metadata = desk.edit_genre_and_save().await;

    assert_eq!(
        metadata.status,
        Some(MetadataStatus::SaveComplete {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            waiting: 0,
            held: 1,
            outputs: OutputEdits {
                updated: 1,
                ..OutputEdits::default()
            },
        })
    );
    assert!(metadata.waiting_writes.is_empty());

    desk.wait_until("the export finishes", |desk| {
        finished(desk.export_status(&export))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(fs::read(&book).expect("read download"), before);
    // The output took the edit; it stays pending for a later export.
    assert_eq!(
        genre_on_disk(&desk.output_of(&export)).as_deref(),
        Some("Mystery")
    );
    assert!(desk.metadata().has_pending_edits);
    assert_eq!(desk.shown(MetadataField::Genre), "Mystery");
}

#[tokio::test]
async fn a_naming_edit_retags_a_finished_output_where_it_is() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;
    desk.import(&book).await;
    let export = desk.export().await;
    desk.wait_until("the export finishes", |desk| {
        finished(desk.export_status(&export))
    })
    .await;
    let output = desk.output_of(&export);

    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Beta".to_string(),
    })
    .await;
    desk.send(SessionIntent::Save).await;

    match desk.metadata().status {
        Some(MetadataStatus::SaveComplete { outputs, .. }) => assert_eq!(
            outputs,
            OutputEdits {
                updated: 1,
                elsewhere: 1,
                ..OutputEdits::default()
            }
        ),
        other => panic!("unexpected status {other:?}"),
    }
    desk.wait_until("the output is retagged", |_| {
        read_metadata(output.to_string_lossy().as_ref())
            .expect("read output")
            .title
            .as_deref()
            == Some("Beta")
    })
    .await;
    assert!(output.exists(), "a finished output is never moved");
}

#[tokio::test]
async fn restarting_a_title_moves_its_unfinished_export_to_the_new_location() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 30.0)
        .await;
    desk.import(&book).await;
    let first = desk.export().await;

    desk.send(SessionIntent::SetField {
        field: MetadataField::Author,
        value: "Other Author".to_string(),
    })
    .await;
    desk.send(SessionIntent::Save).await;
    let offer = desk
        .engine
        .session_snapshot()
        .output
        .expect("output part")
        .restart_offers
        .pop()
        .expect("a restart is offered");
    assert!(offer.to.contains("Other Author"), "{offer:?}");
    let old_folder = PathBuf::from(&offer.from)
        .parent()
        .expect("folder")
        .to_path_buf();

    desk.send(SessionIntent::RestartTitle {
        title_id: offer.title_id.clone(),
        revision: offer.revision,
    })
    .await;

    let second = match submission(&desk) {
        Some(SubmissionStatus::Submitted { operation_id, .. }) => operation_id,
        Some(SubmissionStatus::FinishedBeforeRestart { .. }) => {
            panic!("the export finished before the restart could stop it: lengthen the source")
        }
        other => panic!("restart not submitted: {other:?}"),
    };
    assert_ne!(second, first);
    assert_eq!(desk.export_status(&first), WorkOperationStatus::Cancelled);
    assert!(!old_folder.exists(), "the old empty folders are gone");
    desk.wait_until("the restarted export finishes", |desk| {
        finished(desk.export_status(&second))
    })
    .await;
    let output = desk.output_of(&second);
    assert!(output.to_string_lossy().contains("Other Author"));
    assert_eq!(
        read_metadata(output.to_string_lossy().as_ref())
            .expect("read output")
            .artist
            .as_deref(),
        Some("Other Author")
    );
}

#[tokio::test]
async fn audio_and_output_defaults_are_saved_and_return_after_a_settings_reset() {
    let desk = Desk::new();
    desk.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Format(AudiobookFormat::MkaOpus),
    })
    .await;
    desk.send(SessionIntent::SetOutputDirectory {
        directory: "/library".to_string(),
    })
    .await;

    let saved = desk.engine.settings_snapshot().await.settings;
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

#[tokio::test]
async fn shutdown_cancels_running_exports_writes_waiting_saves_and_refuses_new_work() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 30.0)
        .await;
    desk.import(&book).await;
    let export = desk.export().await;
    desk.edit_genre_and_save().await;
    let running = desk.engine.running_work();
    assert_eq!((running.exports, running.waiting_writes), (1, 1));

    tokio::time::timeout(Duration::from_secs(30), desk.engine.shutdown())
        .await
        .expect("shutdown settles");

    assert!(finished(desk.export_status(&export)));
    assert_eq!(genre_on_disk(&book).as_deref(), Some("Mystery"));
    assert!(desk.engine.running_work().is_empty());
    desk.send(SessionIntent::Submit).await;
    assert_eq!(
        submission(&desk),
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::Closing
        }),
        "new exports are refused after shutdown"
    );
}

fn submission(desk: &Desk) -> Option<abb_engine::session::SubmissionStatus> {
    desk.engine
        .session_snapshot()
        .output
        .expect("output part")
        .submission
}

#[tokio::test]
async fn submit_exports_the_session_reviews_a_collision_and_previews() {
    use abb_engine::output_artifact::CollisionPolicy;

    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 2.0)
        .await;
    desk.import(&book).await;
    let library = desk.root.path().join("out");
    fs::create_dir_all(&library).expect("create output folder");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: library.to_string_lossy().into_owned(),
    })
    .await;

    desk.send(SessionIntent::Submit).await;
    let Some(SubmissionStatus::Submitted {
        operation_id,
        title,
    }) = submission(&desk)
    else {
        panic!("submitted: {:?}", submission(&desk));
    };
    assert_eq!(title, "Alpha");
    desk.wait_until("the export finishes", |desk| {
        finished(desk.export_status(&operation_id))
    })
    .await;
    assert_eq!(
        desk.export_status(&operation_id),
        WorkOperationStatus::Completed
    );

    // The same export again collides with the first output.
    desk.send(SessionIntent::Submit).await;
    assert!(
        matches!(
            submission(&desk),
            Some(SubmissionStatus::ReviewRequired { .. })
        ),
        "{:?}",
        submission(&desk)
    );
    let review_id = desk
        .engine
        .session_snapshot()
        .output
        .expect("output")
        .collision_review
        .expect("review")
        .review_id;
    desk.send(SessionIntent::ChooseCollisionPolicy {
        review_id,
        policy: CollisionPolicy::RenameNew,
    })
    .await;
    assert!(
        matches!(submission(&desk), Some(SubmissionStatus::Submitted { .. })),
        "{:?}",
        submission(&desk)
    );

    desk.send(SessionIntent::Preview { seconds: 1.0 }).await;
    let preview = desk
        .engine
        .session_snapshot()
        .output
        .expect("output")
        .preview_run
        .expect("preview snapshot");
    assert_eq!(preview.operation.status, WorkOperationStatus::Completed);
    assert!(preview.open_ready);
    let id = preview.operation.operation_id.to_string();
    assert!(matches!(
        desk.send(SessionIntent::TakePreviewOutput { run_id: id.clone() })
            .await,
        SessionOutcome::PreviewOutput { path: Some(_) }
    ));
    assert_eq!(
        desk.send(SessionIntent::TakePreviewOutput { run_id: id })
            .await,
        SessionOutcome::PreviewOutput { path: None }
    );
    assert!(
        matches!(
            submission(&desk),
            Some(SubmissionStatus::PreviewFinished { .. })
        ),
        "{:?}",
        submission(&desk)
    );
    assert!(
        !desk
            .engine
            .session_snapshot()
            .titles
            .expect("titles")
            .order_locked
    );
}

#[tokio::test]
async fn shutdown_answers_a_pending_collision_review_so_a_waiting_save_lands() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;
    desk.import(&book).await;
    let first = desk.export().await;
    desk.wait_until("the first export finishes", |desk| {
        finished(desk.export_status(&first))
    })
    .await;
    // The same export again waits for a collision choice, holding its source.
    desk.send(SessionIntent::Submit).await;
    assert!(
        matches!(
            submission(&desk),
            Some(SubmissionStatus::ReviewRequired { .. })
        ),
        "{:?}",
        submission(&desk)
    );
    let metadata = desk.edit_genre_and_save().await;
    assert_eq!(metadata.waiting_writes.len(), 1);

    tokio::time::timeout(Duration::from_secs(30), desk.engine.shutdown())
        .await
        .expect("shutdown does not wait on an unanswered review");

    assert_eq!(genre_on_disk(&book).as_deref(), Some("Mystery"));
}

#[tokio::test]
async fn a_collision_that_appears_during_review_is_reviewed_before_any_policy_applies() {
    use abb_engine::output_artifact::CollisionPolicy;

    let desk = Desk::new();
    let library = desk.root.path().join("library");
    let alpha = desk.audiobook(&library.join("alpha.m4b"), 1.0).await;
    let beta = desk.audiobook(&library.join("beta.m4b"), 1.0).await;
    desk.send(SessionIntent::Import {
        paths: vec![
            alpha.to_string_lossy().into_owned(),
            beta.to_string_lossy().into_owned(),
        ],
    })
    .await;
    // Different titles, so the two exports do not collide with each other.
    desk.send(SessionIntent::SelectFile {
        index: 1,
        modifiers: abb_engine::session::SelectionModifiers::default(),
    })
    .await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Beta".to_string(),
    })
    .await;
    let first = desk.export().await;
    desk.wait_until("the first export finishes", |desk| {
        finished(desk.export_status(&first))
    })
    .await;
    let outputs = |desk: &Desk| -> Vec<PathBuf> { walk(&desk.root.path().join("exports")) };
    let exported = outputs(&desk);
    assert_eq!(exported.len(), 2, "{exported:?}");
    let existing_bytes: Vec<_> = exported
        .iter()
        .map(|path| fs::read(path).expect("read existing audiobook"))
        .collect();

    // Only one output exists when the user reviews.
    let moved = desk.root.path().join("set-aside.m4b");
    fs::rename(&exported[1], &moved).expect("set one output aside");
    desk.send(SessionIntent::Submit).await;
    let Some(SubmissionStatus::ReviewRequired {
        review_id,
        outputs: seen,
        ..
    }) = submission(&desk)
    else {
        panic!("review: {:?}", submission(&desk));
    };
    assert_eq!(seen.len(), 1);

    // The other appears before the user chooses.
    fs::rename(&moved, &exported[1]).expect("put it back");
    desk.send(SessionIntent::ChooseCollisionPolicy {
        review_id,
        policy: CollisionPolicy::ReplaceExisting,
    })
    .await;
    let Some(SubmissionStatus::ReviewRequired {
        review_id: next_review,
        outputs: now,
        ..
    }) = submission(&desk)
    else {
        panic!("back to review: {:?}", submission(&desk));
    };
    assert_eq!(now.len(), 2);
    assert_ne!(next_review, review_id);
    for intent in [
        SessionIntent::ChooseCollisionPolicy {
            review_id,
            policy: CollisionPolicy::ReplaceExisting,
        },
        SessionIntent::CancelCollisionReview { review_id },
    ] {
        assert_eq!(desk.send(intent).await, SessionOutcome::Superseded);
        let output = desk.engine.session_snapshot().output.expect("output");
        assert_eq!(
            output.collision_review.expect("held review").review_id,
            next_review
        );
        assert!(output.submission_in_progress);
    }
    // Path equality alone would miss an in-place overwrite.
    assert_eq!(outputs(&desk), exported);
    for (path, original) in exported.iter().zip(existing_bytes) {
        assert!(
            fs::read(path).expect("read preserved audiobook") == original,
            "held collision review changed {path:?}"
        );
    }
}

/// Every file under `dir`, sorted.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).expect("read folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[tokio::test]
async fn a_choice_made_while_reset_is_waiting_stays_on_screen_and_on_disk() {
    use std::future::Future;
    let desk = Desk::new();
    let typing = desk.engine.session_begin(SessionIntent::SetNamingTemplate {
        template: "{title}".into(),
    });
    typing.finish().await;
    let mut reset = Box::pin(
        desk.engine
            .settings_dispatch(abb_engine::app_settings::SettingsIntent::Reset),
    );
    std::future::poll_fn(|cx| {
        assert!(
            reset.as_mut().poll(cx).is_pending(),
            "reset waits behind earlier typing"
        );
        std::task::Poll::Ready(())
    })
    .await;
    desk.send(SessionIntent::SetOutputDirectory {
        directory: "/after-reset".into(),
    })
    .await;
    reset.await;
    assert_eq!(
        desk.engine
            .session_snapshot()
            .output
            .expect("output")
            .directory
            .as_deref(),
        Some("/after-reset")
    );
    assert_eq!(
        desk.engine
            .settings_snapshot()
            .await
            .settings
            .output_defaults
            .output_directory
            .as_deref(),
        Some("/after-reset")
    );
    desk.engine.shutdown().await;
}

/// Every supported final container must accept a later Save on its published output.
#[tokio::test]
async fn save_updates_finished_outputs_in_every_supported_container() {
    for format in [
        AudiobookFormat::M4b,
        AudiobookFormat::Mp3,
        AudiobookFormat::M4aOpus,
        AudiobookFormat::MkaOpus,
    ] {
        let desk = Desk::new();
        let mut source = desk
            .audiobook(&desk.root.path().join("source.m4b"), 0.2)
            .await;
        if format == AudiobookFormat::Mp3 {
            let mp3 = desk.root.path().join("source.mp3");
            let ffmpeg = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
            let result = std::process::Command::new(ffmpeg)
                .args(["-v", "error", "-i"])
                .arg(&source)
                .args(["-c:a", "libmp3lame"])
                .arg(&mp3)
                .output()
                .expect("MP3 source");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            source = mp3;
        }
        desk.import(&source).await;
        let id = desk.engine.session_snapshot().titles.expect("titles").files[0]
            .input_id
            .clone();
        desk.send(SessionIntent::SetTitleAudio {
            title_ids: vec![id],
            edit: AudioEdit::Format(format),
        })
        .await;
        let output = desk.root.path().join("exports");
        fs::create_dir_all(&output).expect("output folder");
        desk.send(SessionIntent::SetOutputDirectory {
            directory: output.to_string_lossy().into_owned(),
        })
        .await;
        desk.send(SessionIntent::Submit).await;
        let Some(SubmissionStatus::Submitted { operation_id, .. }) = submission(&desk) else {
            panic!("{format:?}: {:?}", submission(&desk));
        };
        desk.wait_until("container export", |desk| {
            finished(desk.export_status(&operation_id))
        })
        .await;
        assert_eq!(
            desk.export_status(&operation_id),
            WorkOperationStatus::Completed,
            "{format:?}"
        );
        let published = desk.output_of(&operation_id);
        let before = genre_on_disk(&source);
        let metadata = desk.edit_genre_and_save().await;
        assert!(
            matches!(
                metadata.status,
                Some(MetadataStatus::SaveComplete {
                    outputs: OutputEdits {
                        updated: 1,
                        failed: 0,
                        ..
                    },
                    ..
                })
            ),
            "{format:?}: {:?}",
            metadata.status
        );
        assert_eq!(
            genre_on_disk(&published).as_deref(),
            Some("Mystery"),
            "{format:?}"
        );
        assert_ne!(before.as_deref(), Some("Mystery"));
        desk.engine.shutdown().await;
    }
}

#[tokio::test]
async fn save_updates_a_grouped_output_without_writing_its_individual_sources() {
    let desk = Desk::new();
    let first = desk
        .audiobook(&desk.root.path().join("first.m4b"), 0.2)
        .await;
    let second = desk
        .audiobook(&desk.root.path().join("second.m4b"), 0.2)
        .await;
    desk.import(&first).await;
    desk.import(&second).await;
    desk.send(SessionIntent::SelectAll).await;
    desk.send(SessionIntent::GroupSelected).await;
    let operation = desk.export().await;
    desk.wait_until("grouped export", |desk| {
        finished(desk.export_status(&operation))
    })
    .await;
    let metadata = desk.edit_genre_and_save().await;
    assert!(
        matches!(
            metadata.status,
            Some(MetadataStatus::SaveComplete {
                outputs: OutputEdits {
                    updated: 1,
                    failed: 0,
                    ..
                },
                ..
            })
        ),
        "{:?}",
        metadata.status
    );
    assert_eq!(
        genre_on_disk(&desk.output_of(&operation)).as_deref(),
        Some("Mystery")
    );
    for source in [&first, &second] {
        assert_eq!(genre_on_disk(source).as_deref(), Some("Fantasy"));
    }
    desk.engine.shutdown().await;
}

#[tokio::test]
async fn shutdown_closes_admission_before_an_accepted_submission_finishes_preflight() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("source.m4b"), 0.2)
        .await;
    desk.import(&book).await;
    let out = desk.root.path().join("out");
    fs::create_dir_all(&out).expect("output folder");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: out.to_string_lossy().into_owned(),
    })
    .await;
    let accepted = desk.engine.session_begin(SessionIntent::Submit);
    // No yield between admission and shutdown: preflight has not registered
    // its export when shutdown closes and enumerates running operations.
    desk.engine.shutdown().await;
    accepted.finish().await;
    assert!(desk
        .engine
        .list_work_operations()
        .expect("operations")
        .operations
        .is_empty());
    assert_eq!(
        submission(&desk),
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::Closing
        })
    );
    assert_eq!(fs::read_dir(&out).expect("outputs").count(), 0);
}

#[tokio::test]
async fn keep_location_writes_the_latest_offered_tags_to_the_original_export_path() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("source.m4b"), 20.0)
        .await;
    desk.import(&book).await;
    let operation = desk.export().await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Author,
        value: "First choice".into(),
    })
    .await;
    desk.send(SessionIntent::Save).await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Author,
        value: "Latest choice".into(),
    })
    .await;
    desk.send(SessionIntent::Save).await;
    let offer = desk
        .engine
        .session_snapshot()
        .output
        .expect("output")
        .restart_offers
        .into_iter()
        .next()
        .expect("an unfinished export offers a move");
    assert_eq!(
        desk.send(SessionIntent::KeepTitleLocation {
            title_id: offer.title_id,
            revision: offer.revision
        })
        .await,
        SessionOutcome::Applied
    );
    desk.wait_until("kept export", |desk| {
        finished(desk.export_status(&operation))
    })
    .await;
    let output = desk.output_of(&operation);
    assert_eq!(output, offer.from);
    assert_eq!(
        read_metadata(output.to_str().expect("path"))
            .expect("real tags")
            .artist
            .as_deref(),
        Some("Latest choice")
    );
    assert!(
        !Path::new(&offer.to).exists(),
        "Keep does not move the output"
    );
    desk.engine.shutdown().await;
}
