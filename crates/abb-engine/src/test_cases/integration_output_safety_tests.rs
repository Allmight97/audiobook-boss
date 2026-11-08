use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::app_settings::{ConcurrencyPreference, SettingsIntent};
use crate::output_artifact::{lock_output_file, CollisionPolicy};
use crate::session::{MetadataField, SelectionModifiers, SessionIntent, SubmissionStatus};
use crate::work_runtime::{ChildJobStatus, OperationId, OperationSnapshot, WorkOperationStatus};
use crate::ShutdownOutcome;

use super::integration_session_desk::{
    collision_review, genre_on_disk, processing_sessions_dir, remote_staging_sessions_dir,
    submission, Desk,
};
use super::support::fs_listing::names_in;
use super::support::poll::until_true;

fn operation(desk: &Desk, id: &OperationId) -> OperationSnapshot {
    desk.engine
        .list_work_operations()
        .expect("operations")
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
