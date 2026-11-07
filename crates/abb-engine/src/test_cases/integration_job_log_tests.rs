//! The `processing_job` records a real export writes, byte for byte.
//! `scripts/dev-log-analysis.ts` parses them to count a session's jobs and
//! failures, so a wording or field-order change breaks that tool without
//! failing any engine test that builds the line from the same code.

use std::fs;
use std::path::{Path, PathBuf};

use abb_engine::work_runtime::{ChildJobStatus, OperationId, WorkOperationStatus};

use super::integration_session_desk::Desk;
use super::log_capture;
use crate::output_artifact::lock_output_file;

fn job_records(operation: &OperationId) -> Vec<String> {
    let marker = format!(" operation_id={} ", operation.as_str());
    log_capture::records()
        .into_iter()
        .filter(|record| record.starts_with("processing_job ") && record.contains(&marker))
        .collect()
}

fn value_of<'a>(record: &'a str, key: &str) -> &'a str {
    record
        .split(' ')
        .find_map(|field| field.strip_prefix(key))
        .unwrap_or_else(|| panic!("{key} in {record}"))
}

fn assert_job_records(operation: &OperationId, status_and_tail: impl Fn(&str) -> String) {
    let records = job_records(operation);
    assert_eq!(records.len(), 2, "{records:?}");
    let job = value_of(&records[0], "job_id=");
    uuid::Uuid::parse_str(job).expect("the job id is a uuid");
    let operation = operation.as_str();
    assert_eq!(
        records[0],
        format!("processing_job event=started operation_id={operation} job_id={job} input_index=0 kind=processing_batch status=running")
    );
    let elapsed = value_of(&records[1], "elapsed_ms=");
    elapsed.parse::<u128>().expect("elapsed milliseconds");
    assert_eq!(
        records[1],
        format!(
            "processing_job event=terminal operation_id={operation} job_id={job} input_index=0 kind=processing_batch {}",
            status_and_tail(elapsed)
        )
    );
}

fn title_is_running(desk: &Desk, operation: &OperationId) -> bool {
    desk.engine
        .list_work_operations()
        .expect("operations")
        .operations
        .into_iter()
        .find(|snapshot| &snapshot.operation_id == operation)
        .is_some_and(|snapshot| snapshot.children[0].status == ChildJobStatus::Running)
}

async fn imported_book(desk: &Desk, seconds: f64) {
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), seconds)
        .await;
    desk.import(&book).await;
}

async fn export_held_at_publication(desk: &Desk) -> (OperationId, PathBuf, impl Sized) {
    imported_book(desk, 1.0).await;
    let folder = desk.root.path().join("exports/Source Author/Alpha");
    fs::create_dir_all(&folder).expect("export folder");
    let hold = lock_output_file(&folder.join("Alpha.m4b"));
    let operation = desk.export().await;
    desk.wait_until("the title runs", |d| title_is_running(d, &operation))
        .await;
    (operation, folder, hold)
}

#[tokio::test]
async fn a_published_title_writes_a_started_and_a_success_record() {
    log_capture::install();
    let desk = Desk::new();
    imported_book(&desk, 1.0).await;
    let operation = desk.export().await;
    desk.wait_until("the export finishes", |d| d.export_finished(&operation))
        .await;
    assert_eq!(
        desk.export_status(&operation),
        WorkOperationStatus::Completed
    );
    assert_job_records(&operation, |elapsed| {
        format!("status=success elapsed_ms={elapsed}")
    });
}

#[tokio::test]
async fn a_cancelled_title_writes_a_started_and_a_cancelled_record() {
    log_capture::install();
    let desk = Desk::new();
    let (operation, _, hold) = export_held_at_publication(&desk).await;
    desk.engine
        .cancel_work_operation(operation.clone(), None)
        .expect("cancel");
    drop(hold);
    desk.wait_until("the export finishes", |d| d.export_finished(&operation))
        .await;
    assert_eq!(
        desk.export_status(&operation),
        WorkOperationStatus::Cancelled
    );
    assert_job_records(&operation, |elapsed| {
        format!("status=cancelled elapsed_ms={elapsed}")
    });
}

#[tokio::test]
async fn a_title_that_cannot_publish_writes_a_started_and_a_failed_record_with_its_code() {
    log_capture::install();
    let desk = Desk::new();
    let (operation, folder, hold) = export_held_at_publication(&desk).await;
    let users_file = folder.join("Alpha.m4b");
    fs::write(&users_file, b"a book that arrived while the export ran").expect("arriving file");
    drop(hold);
    desk.wait_until("the export finishes", |d| d.export_finished(&operation))
        .await;
    assert_eq!(desk.export_status(&operation), WorkOperationStatus::Failed);
    assert_eq!(
        fs::read(&users_file).expect("the arrived file"),
        b"a book that arrived while the export ran"
    );
    assert_job_records(&operation, |elapsed| {
        format!(
            "status=failed elapsed_ms={elapsed} code=file_validation_failed category=validation"
        )
    });
    assert_no_workspace_or_temp_files(&desk, &folder);
    let job = value_of(&job_records(&operation)[0], "job_id=").to_string();
    let errors: Vec<String> = log_capture::error_records()
        .into_iter()
        .filter(|record| record.contains(&job))
        .collect();
    assert_eq!(
        errors.len(),
        1,
        "one failed job, one ERROR line: {errors:?}"
    );
}

fn assert_no_workspace_or_temp_files(desk: &Desk, folder: &Path) {
    assert!(desk.residual_workspace_dirs().is_empty());
    let names: Vec<_> = fs::read_dir(folder)
        .expect("read export folder")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(names, ["Alpha.m4b"]);
}
