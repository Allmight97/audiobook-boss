use std::fs;

use crate::app_settings::{ConcurrencyPreference, SettingsIntent};
use crate::output_artifact::lock_output_file;
use crate::session::{MetadataField, SelectionModifiers, SessionIntent};
use crate::work_runtime::{ChildJobStatus, OperationId, OperationSnapshot};
use crate::ShutdownOutcome;

use super::integration_session_desk::Desk;

fn operation(desk: &Desk, id: &OperationId) -> OperationSnapshot {
    desk.engine
        .list_work_operations()
        .operations
        .into_iter()
        .find(|snapshot| &snapshot.operation_id == id)
        .expect("listed")
}

fn statuses(desk: &Desk, id: &OperationId) -> Vec<ChildJobStatus> {
    operation(desk, id)
        .children
        .into_iter()
        .map(|child| child.status)
        .collect()
}

async fn three_titles_one_at_a_time(desk: &Desk) {
    desk.engine
        .settings_dispatch(SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(1),
        })
        .await;
    let library = desk.root.path().join("library");
    let mut paths = Vec::new();
    for name in ["alpha", "beta", "gamma"] {
        let path = desk
            .audiobook(&library.join(format!("{name}.m4b")), 1.0)
            .await;
        paths.push(path.to_string_lossy().into_owned());
    }
    desk.send(SessionIntent::Import { paths }).await;
    for (index, title) in [(1, "Beta"), (2, "Gamma")] {
        let title_id = desk.title_ids()[index].clone();
        desk.send(SessionIntent::SelectFile {
            title_id,
            modifiers: SelectionModifiers::default(),
        })
        .await;
        desk.send(SessionIntent::SetField {
            binding: desk.metadata().binding,
            field: MetadataField::Title,
            value: title.to_string(),
        })
        .await;
    }
}

#[tokio::test]
async fn titles_waiting_behind_a_running_one_read_their_place_and_close_up_after_a_cancel() {
    let desk = Desk::new();
    three_titles_one_at_a_time(&desk).await;
    let alpha_folder = desk.root.path().join("exports/Source Author/Alpha");
    fs::create_dir_all(&alpha_folder).expect("alpha folder");
    let hold = lock_output_file(&alpha_folder.join("Alpha.m4b"));
    let export = desk.export().await;
    desk.wait_until("alpha runs", |d| {
        statuses(d, &export)[0] == ChildJobStatus::Running
    })
    .await;
    assert_eq!(
        statuses(&desk, &export)[1..],
        [
            ChildJobStatus::Queued { position: 1, of: 2 },
            ChildJobStatus::Queued { position: 2, of: 2 },
        ]
    );

    let beta = operation(&desk, &export).children[1].child_job_id.clone();
    desk.engine
        .cancel_work_operation(export.clone(), Some(beta))
        .expect("cancel beta");
    desk.wait_until("beta settles", |d| {
        statuses(d, &export)[1] == ChildJobStatus::Cancelled
    })
    .await;
    assert_eq!(
        statuses(&desk, &export)[2],
        ChildJobStatus::Queued { position: 1, of: 1 }
    );

    drop(hold);
    desk.wait_until("done", |d| d.export_finished(&export))
        .await;
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
}
