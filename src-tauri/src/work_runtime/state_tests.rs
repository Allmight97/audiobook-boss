use super::snapshot::new_processing_snapshot;
use super::state::WorkRuntimeState;
use super::{ChildJobStatus, OperationId, WorkOperationStatus};
use crate::processing::{
    JobType, OperationKind, ProcessCommandResult, ProcessResultEntry, ProcessResultStatus,
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
