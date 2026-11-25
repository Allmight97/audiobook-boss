use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::app_settings::{ConcurrencyPreference, SettingsIntent};
use crate::session::{
    MetadataField, SelectionModifiers, SessionIntent, SessionOutcome, SubmissionStatus,
};
use crate::work_runtime::{ChildJobStatus, OperationId, OperationSnapshot, WorkOperationStatus};
use crate::ShutdownOutcome;
use abb_media_pipeline::output_artifact::{
    ensure_output_parent_dirs, lock_output_file, CollisionPolicy, OutputFolderRecords, OutputKind,
    PlannedOutputAction, ResolvedOutputPlan,
};
use abb_media_pipeline::processing::OperationKind;

use super::integration_session_desk::{
    collision_review, engine_config, genre_on_disk, processing_sessions_dir,
    remote_staging_sessions_dir, submission, Desk,
};
use abb_test_fixtures::fs_listing::names_in;
use abb_test_fixtures::media::ffmpeg;
use abb_test_fixtures::poll::until_true;
use abb_test_fixtures::wav::write_sine_wav;

fn operation(desk: &Desk, id: &OperationId) -> OperationSnapshot {
    desk.engine
        .list_work_operations()
        .operations
        .into_iter()
        .find(|snapshot| &snapshot.operation_id == id)
        .expect("listed")
}

async fn two_titles(desk: &Desk, seconds: f64) -> (PathBuf, PathBuf) {
    let library = desk.root.path().join("library");
    let alpha = desk.audiobook(&library.join("alpha.m4b"), seconds).await;
    let beta = desk.audiobook(&library.join("beta.m4b"), seconds).await;
    desk.send(SessionIntent::Import {
        paths: vec![
            alpha.to_string_lossy().into_owned(),
            beta.to_string_lossy().into_owned(),
        ],
    })
    .await;
    let beta_id = desk.engine.session_snapshot().titles.expect("titles").files[1]
        .input_id
        .clone();
    desk.send(SessionIntent::SelectFile {
        title_id: beta_id,
        modifiers: SelectionModifiers::default(),
    })
    .await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Title,
        value: "Beta".to_string(),
    })
    .await;
    (alpha, beta)
}

#[tokio::test]
async fn an_accepted_replace_puts_the_new_tags_at_the_same_path_and_touches_nothing_else() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;
    desk.import(&book).await;
    let source_bytes = fs::read(&book).expect("source");
    let first = desk.export_and_finish().await;
    let published = desk.output_of(&first);
    let folder = published.parent().expect("folder").to_path_buf();
    fs::write(folder.join("notes.txt"), b"mine").expect("notes");
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Genre,
        value: "Mystery".to_string(),
    })
    .await;
    desk.send(SessionIntent::Submit).await;
    assert_eq!(submission(&desk), Some(SubmissionStatus::ReviewRequired));
    let review_id = collision_review(&desk).review_id;
    assert_eq!(
        genre_on_disk(&published).as_deref(),
        Some("Fantasy"),
        "review writes nothing"
    );
    desk.send(SessionIntent::ChooseCollisionPolicy {
        review_id,
        policy: CollisionPolicy::ReplaceExisting,
    })
    .await;
    let Some(SubmissionStatus::Submitted { operation_id, .. }) = submission(&desk) else {
        panic!("{:?}", submission(&desk))
    };
    desk.wait_until("replace", |d| d.export_finished(&operation_id))
        .await;
    assert_eq!(
        desk.export_status(&operation_id),
        WorkOperationStatus::Completed
    );
    assert_eq!(desk.output_of(&operation_id), published);
    assert_eq!(names_in(&folder), ["Alpha.m4b", "notes.txt"]);
    assert_eq!(genre_on_disk(&published).as_deref(), Some("Mystery"));
    assert_eq!(fs::read(folder.join("notes.txt")).expect("notes"), b"mine");
    assert!(
        fs::read(&book).expect("source") == source_bytes,
        "source unchanged"
    );
    assert!(desk.residual_workspace_dirs().is_empty());
}

#[tokio::test]
async fn restarting_a_title_queued_behind_another_removes_its_folders_before_returning() {
    let desk = Desk::new();
    desk.engine
        .settings_dispatch(SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(1),
        })
        .await;
    two_titles(&desk, 1.0).await;
    let alpha_folder = desk.root.path().join("exports/Source Author/Alpha");
    fs::create_dir_all(&alpha_folder).expect("alpha folder");
    let hold = lock_output_file(&alpha_folder.join("Alpha.m4b"));
    let first = desk.export().await;
    desk.wait_until("alpha runs", |d| {
        operation(d, &first).children[0].status == ChildJobStatus::Running
    })
    .await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Author,
        value: "Other Author".to_string(),
    })
    .await;
    desk.send(SessionIntent::Save).await;
    let offer = desk
        .engine
        .session_snapshot()
        .output
        .expect("output")
        .restart_offers
        .pop()
        .expect("offer");
    let beta_folder = PathBuf::from(&offer.from)
        .parent()
        .expect("folder")
        .to_path_buf();
    assert!(beta_folder.exists());
    tokio::time::timeout(
        Duration::from_secs(5),
        desk.send(SessionIntent::RestartTitle {
            title_id: offer.title_id,
            revision: offer.revision,
        }),
    )
    .await
    .expect("the restart does not wait for the title ahead");
    assert!(
        !beta_folder.exists(),
        "folders gone when the restart returns"
    );
    assert_eq!(
        operation(&desk, &first).children[0].status,
        ChildJobStatus::Running,
        "alpha still held"
    );
    drop(hold);
    let Some(SubmissionStatus::Submitted { operation_id, .. }) = submission(&desk) else {
        panic!("{:?}", submission(&desk))
    };
    desk.wait_until("both", |d| {
        d.export_finished(&first) && d.export_finished(&operation_id)
    })
    .await;
    assert!(desk
        .output_of(&operation_id)
        .to_string_lossy()
        .contains("Other Author"));
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
}

#[tokio::test]
async fn cancelling_one_title_keeps_its_sibling_and_leaves_no_residue() {
    let desk = Desk::new();
    two_titles(&desk, 30.0).await;
    let first = desk.export().await;
    let folder = desk.root.path().join("exports/Source Author/Beta");
    until_true("the second title's folder exists", || folder.exists()).await;
    let hold = lock_output_file(&folder.join("Beta.m4b"));
    let beta = operation(&desk, &first).children[1].child_job_id.clone();
    desk.engine
        .cancel_work_operation(first.clone(), Some(beta))
        .expect("cancel");
    drop(hold);
    desk.wait_until("done", |d| d.export_finished(&first)).await;
    let op = operation(&desk, &first);
    assert_eq!(op.status, WorkOperationStatus::Mixed);
    assert_eq!(op.children[1].status, ChildJobStatus::Cancelled);
    assert!(PathBuf::from(op.children[0].output_path.clone().expect("alpha")).exists());
    assert!(!folder.exists());
    assert!(desk.residual_workspace_dirs().is_empty());
}

#[tokio::test]
async fn quitting_during_an_export_leaves_no_output_folder_or_workspace() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 30.0)
        .await;
    desk.import(&book).await;
    let first = desk.export().await;
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
    assert_eq!(desk.export_status(&first), WorkOperationStatus::Cancelled);
    assert_eq!(
        names_in(&desk.root.path().join("exports")),
        Vec::<String>::new()
    );
    assert!(desk.residual_workspace_dirs().is_empty());
}

#[tokio::test]
async fn a_relaunch_sweeps_what_a_crash_left_in_the_cache() {
    let desk = Desk::new();
    let session = processing_sessions_dir(desk.root.path()).join("crashed");
    let download = remote_staging_sessions_dir(desk.root.path()).join("crashed");
    for folder in [&session, &download] {
        fs::create_dir_all(folder).expect("folder");
        fs::write(folder.join("part.m4b"), b"audio").expect("file");
    }
    let desk = desk.relaunch();
    assert!(!session.exists() && !download.exists());
    assert!(session.parent().expect("root").exists());
    drop(desk);
}

fn written_at(path: PathBuf) -> ResolvedOutputPlan {
    ResolvedOutputPlan {
        kind: OutputKind::Final,
        requested_path: path.clone(),
        resolved_path: path,
        rename: abb_output_artifact_core::RenameOption::Unneeded,
        choices: Default::default(),
        collision: None,
        action: PlannedOutputAction::Write,
    }
}

#[tokio::test]
async fn a_relaunch_removes_only_the_empty_output_folders_a_crash_left() {
    let desk = Desk::new();
    let exports = desk.root.path().join("exports");
    let unfinished = exports.join("Crash Author/Crashed");
    let published = exports.join("Kept Author/Kept");
    let hand_made = exports.join("Hand Made");
    let records = OutputFolderRecords::for_cache(&engine_config(desk.root.path()).cache_dir);
    let killed_run = ensure_output_parent_dirs(
        &records,
        &exports,
        [
            &written_at(unfinished.join("Crashed.m4b")),
            &written_at(published.join("Kept.m4b")),
        ],
    )
    .expect("the run creates its folders");
    std::mem::forget(killed_run);
    fs::write(published.join("Kept.m4b"), b"audio").expect("published file");
    fs::create_dir(&hand_made).expect("the user's empty folder");

    let desk = desk.relaunch();

    assert_eq!(
        names_in(&exports),
        vec!["Hand Made".to_string(), "Kept Author".to_string()]
    );
    assert!(published.join("Kept.m4b").exists());
    drop(desk);
}

fn tone_hz(path: &Path) -> f64 {
    let samples: Vec<f32> = ffmpeg([
        "-v",
        "error",
        "-i",
        path.to_str().expect("utf8 path"),
        "-map",
        "0:a:0",
        "-ac",
        "1",
        "-ar",
        "44100",
        "-f",
        "f32le",
        "-c:a",
        "pcm_f32le",
        "pipe:1",
    ])
    .as_chunks::<4>()
    .0
    .iter()
    .map(|bytes| f32::from_le_bytes(*bytes))
    .collect();
    let crossings = samples
        .windows(2)
        .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
        .count();
    crossings as f64 * 44_100.0 / (2.0 * samples.len() as f64)
}

fn metadata_save(desk: &Desk) -> Option<OperationSnapshot> {
    desk.engine
        .list_work_operations()
        .operations
        .into_iter()
        .find(|snapshot| snapshot.kind == OperationKind::MetadataSave)
}

#[tokio::test]
async fn a_save_on_a_replacement_exports_destination_never_restores_the_old_audio() {
    let desk = Desk::new();
    let existing = desk
        .audiobook(
            &desk
                .root
                .path()
                .join("exports/Source Author/Alpha/Alpha.m4b"),
            2.0,
        )
        .await;
    let old_bytes = fs::read(&existing).expect("existing output");
    assert!((tone_hz(&existing) - 440.0).abs() < 20.0);
    let replacement = desk.root.path().join("library/replacement.wav");
    fs::create_dir_all(replacement.parent().expect("parent")).expect("library");
    write_sine_wav(&replacement, 2.0, 880.0);
    desk.import(&replacement).await;
    for (field, value) in [
        (MetadataField::Title, "Alpha"),
        (MetadataField::Author, "Source Author"),
    ] {
        desk.send(SessionIntent::SetField {
            binding: desk.metadata().binding,
            field,
            value: value.to_string(),
        })
        .await;
    }
    desk.prepare_export().await;
    desk.send(SessionIntent::Submit).await;
    let review_id = collision_review(&desk).review_id;
    let publication = lock_output_file(&existing);
    desk.send(SessionIntent::ChooseCollisionPolicy {
        review_id,
        policy: CollisionPolicy::ReplaceExisting,
    })
    .await;
    let export = desk.submitted_operation();
    assert_eq!(
        desk.send(SessionIntent::ClearAll).await,
        SessionOutcome::Applied
    );
    desk.import(&existing).await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Genre,
        value: "Mystery".to_string(),
    })
    .await;

    let desk = &desk;
    let held = async {
        desk.wait_until("the save starts", |desk| metadata_save(desk).is_some())
            .await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let untouched = fs::read(&existing).expect("existing output") == old_bytes;
        drop(publication);
        untouched
    };
    let (saved, untouched) = tokio::join!(desk.send(SessionIntent::Save), held);
    assert!(untouched, "the save waited while publication held the file");
    assert_eq!(saved, SessionOutcome::Applied);
    desk.wait_until("the export finishes", |desk| desk.export_finished(&export))
        .await;

    assert_eq!(desk.export_status(&export), WorkOperationStatus::Completed);
    assert_eq!(desk.output_of(&export), existing);
    assert_eq!(
        metadata_save(desk).map(|save| save.status),
        Some(WorkOperationStatus::Completed)
    );
    let hz = tone_hz(&existing);
    assert!(
        (hz - 880.0).abs() < 20.0,
        "the replacement's audio: {hz} Hz"
    );
    assert!(
        matches!(genre_on_disk(&existing).as_deref(), None | Some("Mystery")),
        "{:?}",
        genre_on_disk(&existing)
    );
}
