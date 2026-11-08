use super::*;
use abb_engine::work_runtime::{
    ChildJobSnapshot, ProgressSnapshot, WorkOperationStatus, WorkProgressStage,
};

#[test]
fn export_verdict_requires_the_requested_title_to_be_cancelled() {
    use ChildJobStatus::{Cancelled, Completed};
    for (statuses, succeeds) in [
        ([Completed, Completed], false),
        ([Cancelled, Completed], true),
        ([Completed, Cancelled], false),
    ] {
        let status = if statuses.contains(&Cancelled) {
            WorkOperationStatus::Mixed
        } else {
            WorkOperationStatus::Completed
        };
        let mut operation = operation(1, status, OperationPhase::Finished, &statuses);
        operation.cancel_requested = true;
        assert_eq!(
            export_verdict(&operation, Some(1)).is_ok(),
            succeeds,
            "{statuses:?}"
        );
    }
}

fn landing(status: Option<MetadataStatus>, save_expected: bool) -> Result<(), String> {
    match during_export::landed_verdict(status.as_ref(), save_expected) {
        Ok(()) => Ok(()),
        Err(AbbDevStatus::SaveFailed(message)) => Err(message),
        Err(_) => Err("not a save failure".into()),
    }
}

#[test]
fn a_landed_export_save_reports_every_save_failure() {
    let finished = |failed| MetadataStatus::DeferredWritesFinished { written: 1, failed };
    let error = abb_engine::AppErrorEnvelope::from(abb_engine::AppError::General("disk".into()));
    let save_failed = format!("Save failed: {}", error.message);
    let failed_save = MetadataStatus::SaveFailed { error };
    assert_eq!(landing(Some(finished(0)), true), Ok(()));
    assert_eq!(landing(None, false), Ok(()));
    assert_eq!(
        landing(Some(finished(2)), true),
        Err("Save could not write 2 file(s)".into())
    );
    assert_eq!(landing(Some(failed_save), true), Err(save_failed));
    assert_eq!(
        landing(Some(MetadataStatus::SaveInvalid), true),
        Err("Save was refused: the edits are invalid".into())
    );
    assert!(landing(None, true).is_err());
}

#[test]
fn during_export_steps_keep_the_order_given() {
    let args = [
        "book.m4b",
        "--during-export-set",
        "genre=Mystery",
        "--during-export-reimport",
        "--during-export-set",
        "author=A=B",
    ];
    let options = parse(args.iter().map(|arg| arg.to_string())).expect("options");
    let steps: Vec<_> = options
        .during_export
        .iter()
        .map(|step| match step {
            DuringExport::Set(field, value) => format!("{field:?}={value}"),
            DuringExport::Reimport => "reimport".to_string(),
        })
        .collect();
    assert_eq!(steps, ["Genre=Mystery", "reimport", "Author=A=B"]);
}

#[test]
fn a_during_export_edit_needs_a_known_field_and_a_value() {
    for args in [
        ["book.m4b", "--during-export-set", "genre"],
        ["book.m4b", "--during-export-set", "mood=Calm"],
    ] {
        assert!(parse(args.iter().map(|arg| arg.to_string())).is_err());
    }
}

fn operation(
    revision: u64,
    status: WorkOperationStatus,
    phase: OperationPhase,
    children: &[ChildJobStatus],
) -> OperationSnapshot {
    let operation_id = OperationId("export".into());
    let progress = ProgressSnapshot {
        stage: WorkProgressStage::Converting,
        percentage: 10.0,
        message: "working".into(),
        eta_seconds: None,
    };
    OperationSnapshot {
        operation_id: operation_id.clone(),
        sequence: 1,
        revision,
        kind: abb_processing_core::OperationKind::ProcessingBatch,
        status,
        phase,
        title: "export".into(),
        started_at_ms: None,
        finished_at_ms: None,
        cancellable: false,
        cancel_requested: false,
        progress: progress.clone(),
        children: children
            .iter()
            .enumerate()
            .map(|(index, status)| ChildJobSnapshot {
                child_job_id: index.to_string(),
                operation_id: operation_id.clone(),
                label: index.to_string(),
                status: *status,
                started_at_ms: None,
                finished_at_ms: None,
                progress: progress.clone(),
                source_path: None,
                input_index: None,
                input_id: None,
                source_input_ids: Vec::new(),
                job_id: None,
                cancellable: false,
                cancel_requested: false,
                message: None,
                output_path: None,
                supplemental_warning: None,
                warnings: Vec::new(),
                output_update: None,
            })
            .collect(),
        terminal_summary: None,
        log_tail: Vec::new(),
    }
}

fn snapshot(
    revision: u64,
    status: WorkOperationStatus,
    phase: OperationPhase,
    child: ChildJobStatus,
) -> OperationSnapshot {
    operation(revision, status, phase, &[child])
}

fn report(progress: &PrintProgress, operation: OperationSnapshot) {
    progress.emit(EngineEvent::WorkOperations(
        abb_engine::work_runtime::WorkOperationsUpdate {
            revision: operation.revision,
            order: vec![operation.operation_id.clone()],
            changed: Some(operation),
            concurrency_locked: false,
        },
    ));
}

#[tokio::test]
async fn the_wait_ends_on_the_finished_snapshot_reported_while_it_waits() {
    let progress = Arc::new(PrintProgress::default());
    let mut feed = progress.feed();
    let id = OperationId("export".into());
    let reporter = async {
        for (revision, status, phase, child) in [
            (
                1,
                WorkOperationStatus::Accepted,
                OperationPhase::Accepted,
                ChildJobStatus::Queued,
            ),
            (
                2,
                WorkOperationStatus::Running,
                OperationPhase::Active,
                ChildJobStatus::Running,
            ),
            (
                3,
                WorkOperationStatus::Completed,
                OperationPhase::Finished,
                ChildJobStatus::Completed,
            ),
        ] {
            report(&progress, snapshot(revision, status, phase, child));
            tokio::task::yield_now().await;
        }
    };
    let mut phases = Vec::new();
    let (finished, ()) = tokio::join!(
        await_finished(&mut feed, &id, |operation| {
            phases.push(operation.phase);
            Ok(())
        }),
        reporter
    );
    let finished = finished.expect("finished");
    assert_eq!(finished.revision, 3);
    assert_eq!(phases.last(), Some(&OperationPhase::Finished));
}

#[tokio::test]
async fn a_snapshot_reported_before_the_wait_began_still_ends_it() {
    let progress = PrintProgress::default();
    report(
        &progress,
        snapshot(
            2,
            WorkOperationStatus::Completed,
            OperationPhase::Finished,
            ChildJobStatus::Completed,
        ),
    );
    let mut feed = progress.feed();
    let finished = await_finished(&mut feed, &OperationId("export".into()), |_| Ok(()))
        .await
        .expect("finished");
    assert_eq!(finished.revision, 2);
}

#[test]
fn an_older_snapshot_never_replaces_a_newer_one() {
    let progress = PrintProgress::default();
    let feed = progress.feed();
    report(
        &progress,
        snapshot(
            3,
            WorkOperationStatus::Completed,
            OperationPhase::Finished,
            ChildJobStatus::Completed,
        ),
    );
    report(
        &progress,
        snapshot(
            2,
            WorkOperationStatus::Running,
            OperationPhase::Active,
            ChildJobStatus::Running,
        ),
    );
    let operations = feed.borrow();
    let known = operations
        .get(&OperationId("export".into()))
        .expect("known");
    assert_eq!(known.revision, 3);
}
