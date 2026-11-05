use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use abb_engine::app_settings::{ConcurrencyPreference, SettingsIntent};
use abb_engine::session::{MetadataField, SelectionModifiers, SessionIntent, SubmissionStatus};
use abb_engine::work_runtime::{ChildJobStatus, OperationId, OperationSnapshot};
use abb_engine::ShutdownOutcome;

use super::integration_session_desk::{finished, submission, Desk};
use crate::output_artifact::lock_output_file;

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
        field: MetadataField::Title,
        value: "Beta".to_string(),
    })
    .await;
    (alpha, beta)
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
        finished(d.export_status(&first)) && finished(d.export_status(&operation_id))
    })
    .await;
    assert!(desk
        .output_of(&operation_id)
        .to_string_lossy()
        .contains("Other Author"));
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
}
