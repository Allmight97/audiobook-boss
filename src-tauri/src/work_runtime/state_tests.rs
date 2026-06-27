use super::snapshot::{new_metadata_save_snapshot, new_processing_snapshot};
use super::state::WorkRuntimeState;
use super::{ChildJobStatus, OperationId, ResourceLane, WorkOperationStatus, WorkProgressStage};
use crate::processing::{
    EventStage, JobType, OperationKind, OperationResultSummary, ProcessCommandResult,
    ProcessResultEntry, ProcessResultStatus, ProgressEvent,
};

fn accepted_state() -> (WorkRuntimeState, OperationId) {
    let mut state = WorkRuntimeState::default();
    let operation_id = OperationId("op-1".to_string());
    state.insert_operation(new_processing_snapshot(
        operation_id.clone(),
        1,
        OperationKind::ProcessingBatch,
        "Batch encode (2 files)".to_string(),
        &["/tmp/first.m4b".to_string(), "/tmp/second.m4b".to_string()],
        Some(&[Some("input-1".to_string()), Some("input-2".to_string())]),
        100,
    ));
    (state, operation_id)
}

#[test]
fn accepted_operation_preserves_immutable_child_identity() {
    let (state, operation_id) = accepted_state();
    let snapshot = state.get(&operation_id).expect("snapshot");

    assert_eq!(snapshot.status, WorkOperationStatus::Accepted);
    assert_eq!(snapshot.source_input_ids, ["input-1", "input-2"]);
    assert_eq!(snapshot.children.len(), 2);
    assert_eq!(snapshot.children[0].input_id.as_deref(), Some("input-1"));
    assert_eq!(snapshot.children[0].label, "first.m4b");
}

#[test]
fn request_cancel_marks_operation_and_children_without_terminalizing() {
    let (mut state, operation_id) = accepted_state();
    let snapshot = state
        .request_cancel(&operation_id, 200)
        .expect("request cancel");

    assert_eq!(snapshot.status, WorkOperationStatus::Cancelling);
    assert!(snapshot.cancel_requested);
    assert!(snapshot.children.iter().all(|child| child.cancel_requested));
    assert!(snapshot.finished_at_ms.is_none());
}

#[test]
fn mark_running_does_not_overwrite_pending_cancellation() {
    let (mut state, operation_id) = accepted_state();
    state
        .request_cancel(&operation_id, 200)
        .expect("request cancel");
    let snapshot = state.mark_running(&operation_id, 250).expect("running");

    assert_eq!(snapshot.status, WorkOperationStatus::Cancelling);
    assert!(snapshot.cancel_requested);
    assert_eq!(snapshot.progress.message, "Cancellation requested.");
    assert_eq!(snapshot.started_at_ms, Some(250));
}

#[test]
fn late_progress_does_not_overwrite_pending_cancellation() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");
    state
        .request_cancel(&operation_id, 200)
        .expect("request cancel");

    let snapshot = state
        .apply_progress_event(
            &operation_id,
            &ProgressEvent {
                operation_kind: OperationKind::ProcessingBatch,
                stage: EventStage::Converting,
                percentage: 60.0,
                message: "Converting after cancel".to_string(),
                current_file: Some("/tmp/first.m4b".to_string()),
                eta_seconds: None,
                job_id: Some("job-1".to_string()),
                input_index: Some(0),
            },
        )
        .expect("progress");

    assert_eq!(snapshot.status, WorkOperationStatus::Cancelling);
    assert_eq!(snapshot.progress.stage, WorkProgressStage::Cleaning);
    assert_eq!(snapshot.progress.message, "Cancellation requested.");
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Running);
    assert_eq!(
        snapshot.children[0].progress.message,
        "Converting after cancel"
    );
}

#[test]
fn terminal_result_updates_summary_and_child_rows_in_input_order() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");
    let result = ProcessCommandResult::new(
        JobType::Batch,
        vec![
            ProcessResultEntry {
                input_index: Some(0),
                status: ProcessResultStatus::Success,
                message: "first ok".to_string(),
                error: None,
                preview_file_path: None,
                preview_actual_seconds: None,
                job_id: Some("job-1".to_string()),
            },
            ProcessResultEntry {
                input_index: Some(1),
                status: ProcessResultStatus::Failed,
                message: "second failed".to_string(),
                error: None,
                preview_file_path: None,
                preview_actual_seconds: None,
                job_id: Some("job-2".to_string()),
            },
        ],
    );

    let snapshot = state
        .complete_from_process_result(&operation_id, &result, 300)
        .expect("complete");

    assert_eq!(snapshot.status, WorkOperationStatus::Mixed);
    assert_eq!(
        snapshot.terminal_summary.as_ref().expect("summary").failed,
        1
    );
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Completed);
    assert_eq!(snapshot.children[1].status, ChildJobStatus::Failed);
    assert_eq!(snapshot.children[1].job_id.as_deref(), Some("job-2"));
}

fn entry(input_index: usize, status: ProcessResultStatus) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index: Some(input_index),
        status,
        message: format!("item {input_index}"),
        error: None,
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id: Some(format!("job-{input_index}")),
    }
}

#[test]
fn apply_batch_progress_updates_matched_child_and_aggregates_operation_progress() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");

    let first_update = state
        .apply_progress_event(
            &operation_id,
            &ProgressEvent {
                operation_kind: OperationKind::ProcessingBatch,
                stage: EventStage::Converting,
                percentage: 25.0,
                message: "Converting file 1".to_string(),
                current_file: Some("/tmp/first.m4b".to_string()),
                eta_seconds: None,
                job_id: Some("job-1".to_string()),
                input_index: Some(0),
            },
        )
        .expect("progress");

    assert_eq!(first_update.status, WorkOperationStatus::Running);
    assert_eq!(first_update.progress.percentage, 12.5);
    assert_eq!(first_update.progress.stage, WorkProgressStage::Converting);
    assert_eq!(
        first_update.children[0].status,
        ChildJobStatus::Running,
        "first child should become running"
    );

    let terminal_update = state
        .apply_progress_event(
            &operation_id,
            &ProgressEvent {
                operation_kind: OperationKind::ProcessingBatch,
                stage: EventStage::Completed,
                percentage: 100.0,
                message: "first complete".to_string(),
                current_file: Some("/tmp/first.m4b".to_string()),
                eta_seconds: None,
                job_id: Some("job-1".to_string()),
                input_index: Some(0),
            },
        )
        .expect("progress");

    assert_eq!(terminal_update.status, WorkOperationStatus::Running);
    assert_eq!(terminal_update.progress.percentage, 50.0);
    assert_eq!(
        terminal_update.progress.stage,
        WorkProgressStage::Converting
    );
    assert_eq!(
        terminal_update.children[0].status,
        ChildJobStatus::Completed
    );
}

#[test]
fn apply_single_child_progress_stages_operation_stage_without_terminaling_operation() {
    let (mut state, operation_id) = accepted_state();
    let single = OperationId("single-op".to_string());
    let mut single_snapshot = state.get(&operation_id).expect("snapshot");
    single_snapshot.children = vec![single_snapshot.children.remove(0)];
    single_snapshot.operation_id = single.clone();
    state.insert_operation(single_snapshot);

    let snapshot = state
        .apply_progress_event(
            &single,
            &ProgressEvent {
                operation_kind: OperationKind::ProcessingMerge,
                stage: EventStage::Completed,
                percentage: 100.0,
                message: "done".to_string(),
                current_file: None,
                eta_seconds: None,
                job_id: Some("single-job".to_string()),
                input_index: None,
            },
        )
        .expect("progress");

    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert_eq!(snapshot.progress.stage, WorkProgressStage::Complete);
    assert_eq!(snapshot.children.len(), 1);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Completed);
}

#[test]
fn apply_batch_progress_failure_keeps_operation_running_for_child_event() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");

    let snapshot = state
        .apply_progress_event(
            &operation_id,
            &ProgressEvent {
                operation_kind: OperationKind::ProcessingBatch,
                stage: EventStage::Failed,
                percentage: 0.0,
                message: "first failed".to_string(),
                current_file: Some("/tmp/first.m4b".to_string()),
                eta_seconds: None,
                job_id: Some("job-1".to_string()),
                input_index: Some(0),
            },
        )
        .expect("progress");

    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Failed);
}

#[test]
fn apply_single_child_progress_failure_terminalizes_operation_immediately() {
    let (mut state, operation_id) = accepted_state();
    let single = OperationId("single-op".to_string());
    let mut single_snapshot = state.get(&operation_id).expect("snapshot");
    single_snapshot.children = vec![single_snapshot.children.remove(0)];
    single_snapshot.operation_id = single.clone();
    state.insert_operation(single_snapshot);

    let snapshot = state
        .apply_progress_event(
            &single,
            &ProgressEvent {
                operation_kind: OperationKind::ProcessingMerge,
                stage: EventStage::Failed,
                percentage: 0.0,
                message: "done".to_string(),
                current_file: None,
                eta_seconds: None,
                job_id: Some("single-job".to_string()),
                input_index: None,
            },
        )
        .expect("progress");

    assert_eq!(snapshot.status, WorkOperationStatus::Failed);
    assert_eq!(snapshot.children.len(), 1);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Failed);
}

#[test]
fn success_plus_skipped_resolves_to_mixed_matching_canonical_classifier() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");
    let result = ProcessCommandResult::new(
        JobType::Batch,
        vec![
            entry(0, ProcessResultStatus::Success),
            entry(1, ProcessResultStatus::Skipped),
        ],
    );

    let snapshot = state
        .complete_from_process_result(&operation_id, &result, 300)
        .expect("complete");

    assert_eq!(snapshot.status, WorkOperationStatus::Mixed);
    assert_eq!(
        snapshot.terminal_summary.as_ref().expect("summary").message,
        "Finished with 1 succeeded and 1 skipped."
    );
}

#[test]
fn skipped_plus_cancelled_reports_mixed_message_counts() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");
    let result = ProcessCommandResult::new(
        JobType::Batch,
        vec![
            entry(0, ProcessResultStatus::Skipped),
            entry(1, ProcessResultStatus::Cancelled),
        ],
    );

    let snapshot = state
        .complete_from_process_result(&operation_id, &result, 300)
        .expect("complete");

    assert_eq!(snapshot.status, WorkOperationStatus::Mixed);
    assert_eq!(
        snapshot.terminal_summary.as_ref().expect("summary").message,
        "Finished with 1 skipped and 1 cancelled."
    );
}

#[test]
fn skipped_only_resolves_to_completed() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");
    let result = ProcessCommandResult::new(
        JobType::Batch,
        vec![
            entry(0, ProcessResultStatus::Skipped),
            entry(1, ProcessResultStatus::Skipped),
        ],
    );

    let snapshot = state
        .complete_from_process_result(&operation_id, &result, 300)
        .expect("complete");

    assert_eq!(snapshot.status, WorkOperationStatus::Completed);
}

fn metadata_save_state() -> (WorkRuntimeState, OperationId) {
    let mut state = WorkRuntimeState::default();
    let operation_id = OperationId("op-meta".to_string());
    state.insert_operation(new_metadata_save_snapshot(
        operation_id.clone(),
        1,
        "Metadata save (2 files)".to_string(),
        &["/tmp/a.m4b".to_string(), "/tmp/b.m4b".to_string()],
        100,
    ));
    (state, operation_id)
}

#[test]
fn metadata_save_snapshot_uses_metadata_write_lane() {
    let (state, operation_id) = metadata_save_state();
    let snapshot = state.get(&operation_id).expect("snapshot");

    assert_eq!(snapshot.kind, OperationKind::MetadataSave);
    assert_eq!(snapshot.lanes, [ResourceLane::MetadataWrite]);
    assert_eq!(snapshot.children.len(), 2);
    assert!(snapshot
        .children
        .iter()
        .all(|child| child.lane == ResourceLane::MetadataWrite));
    assert_eq!(snapshot.children[0].input_index, Some(0));
    assert_eq!(snapshot.children[0].label, "a.m4b");
    // Inline operations carry no per-child job_id; progress matches by index.
    assert!(snapshot.children.iter().all(|child| child.job_id.is_none()));
}

#[test]
fn complete_from_summary_terminalizes_metadata_save_children_by_index() {
    let (mut state, operation_id) = metadata_save_state();
    state.mark_running(&operation_id, 150).expect("running");

    // One success, one failure → canonical classifier resolves the run to Mixed,
    // and each child terminalizes from its (input_index, status, message) fact.
    let summary = OperationResultSummary {
        total: 2,
        succeeded: 1,
        skipped: 0,
        cancelled: 0,
        failed: 1,
    };
    let child_terminals = vec![
        (
            0usize,
            ProcessResultStatus::Success,
            "Saved metadata".to_string(),
        ),
        (
            1usize,
            ProcessResultStatus::Failed,
            "Failed metadata save".to_string(),
        ),
    ];

    let snapshot = state
        .complete_from_summary(&operation_id, &summary, &child_terminals, 300)
        .expect("complete");

    assert_eq!(snapshot.status, WorkOperationStatus::Mixed);
    let terminal = snapshot.terminal_summary.as_ref().expect("summary");
    assert_eq!(terminal.succeeded, 1);
    assert_eq!(terminal.failed, 1);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Completed);
    assert_eq!(
        snapshot.children[0].message.as_deref(),
        Some("Saved metadata")
    );
    assert_eq!(snapshot.children[1].status, ChildJobStatus::Failed);
    assert!(!snapshot.cancellable);
    assert_eq!(snapshot.finished_at_ms, Some(300));
}
