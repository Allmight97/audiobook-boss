use super::snapshot::new_processing_snapshot;
use super::state::WorkRuntimeState;
use super::{ChildJobStatus, OperationId, OperationPhase, WorkOperationStatus, WorkProgressStage};
use crate::processing::{
    EventStage, OperationKind, OperationResultSummary, ProcessCommandResult, ProcessResultEntry,
    ProcessResultStatus, ProgressEvent,
};

fn accepted_state() -> (WorkRuntimeState, OperationId) {
    let mut state = WorkRuntimeState::default();
    let operation_id = OperationId("op-1".to_string());
    state.insert_operation(new_processing_snapshot(
        operation_id.clone(),
        1,
        "Batch encode (2 files)".to_string(),
        &["/tmp/first.m4b".to_string(), "/tmp/second.m4b".to_string()],
        Some(&[Some("input-1".to_string()), Some("input-2".to_string())]),
    ));
    (state, operation_id)
}

fn event(stage: EventStage, message: &str) -> ProgressEvent {
    ProgressEvent {
        operation_kind: OperationKind::ProcessingBatch,
        stage,
        percentage: 0.0,
        message: message.to_string(),
        current_file: None,
        eta_seconds: None,
        job_id: Some("job-1".to_string()),
        input_index: Some(0),
    }
}

fn entry(input_index: usize, status: ProcessResultStatus) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index,
        status,
        message: format!("item {input_index}"),
        error: None,
        output_path: None,
        supplemental_warning: None,
        warnings: Vec::new(),
        job_id: Some(format!("job-{input_index}")),
    }
}

#[test]
fn a_processing_run_keeps_a_title_cancel_request_and_an_inline_run_clears_it() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");
    state
        .request_child_cancel(&operation_id, "input-0")
        .expect("cancel first title");
    let result = ProcessCommandResult::new(vec![
        entry(0, ProcessResultStatus::Cancelled),
        entry(1, ProcessResultStatus::Success),
    ]);
    let processing = state
        .complete_from_process_result(&operation_id, &result, 200)
        .expect("complete");
    assert!(processing.children[0].cancel_requested);

    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");
    state
        .request_child_cancel(&operation_id, "input-0")
        .expect("cancel first title");
    let summary = OperationResultSummary::all_cancelled(2);
    let terminals = [
        (0, ProcessResultStatus::Cancelled, "stopped".to_string()),
        (1, ProcessResultStatus::Cancelled, "stopped".to_string()),
    ];
    let inline = state
        .complete_from_summary(&operation_id, &summary, &terminals, 200)
        .expect("complete");
    assert!(inline.children.iter().all(|child| !child.cancel_requested));
}

#[test]
fn an_aborted_run_keeps_finished_titles_and_stamps_the_rest() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");
    let finished = event(EventStage::Completed, "done");
    state
        .apply_progress_event(&operation_id, &finished, 150)
        .expect("first title completes");

    let failed = state
        .fail(&operation_id, "disk full".to_string(), 300)
        .expect("fail");
    assert_eq!(failed.status, WorkOperationStatus::Failed);
    assert_eq!(failed.children[0].status, ChildJobStatus::Completed);
    assert_eq!(failed.children[0].finished_at_ms, Some(150));
    assert_eq!(failed.children[1].status, ChildJobStatus::Failed);
    assert_eq!(failed.children[1].finished_at_ms, Some(300));
    assert!(!failed.children[1].cancel_requested);
}

#[test]
fn a_single_title_cancel_event_mirrors_the_stage_and_leaves_the_export_running() {
    let mut state = WorkRuntimeState::default();
    let operation_id = OperationId("single-cancelled".to_string());
    state.insert_operation(new_processing_snapshot(
        operation_id.clone(),
        1,
        "Book".to_string(),
        &["/tmp/only.m4b".to_string()],
        None,
    ));
    state.mark_running(&operation_id, 150).expect("running");
    let cancelled = event(EventStage::Cancelled, "Processing was cancelled");

    let snapshot = state
        .apply_progress_event(&operation_id, &cancelled, 1_000)
        .expect("progress");

    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert_eq!(snapshot.progress.stage, WorkProgressStage::Cancelled);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Cancelled);
}

#[test]
fn the_snapshot_phase_follows_every_status_change_an_operation_goes_through() {
    let (mut state, operation_id) = accepted_state();
    let phase = |state: &WorkRuntimeState| {
        state
            .operation(operation_id.as_str())
            .expect("operation is listed")
            .phase
    };
    assert_eq!(phase(&state), OperationPhase::Accepted);

    state.mark_running(&operation_id, 100).expect("running");
    assert_eq!(phase(&state), OperationPhase::Active);

    state
        .request_cancel(&operation_id, 150)
        .expect("cancelling");
    let cancelling = state
        .operation(operation_id.as_str())
        .expect("operation is listed");
    assert_eq!(cancelling.status, WorkOperationStatus::Cancelling);
    assert_eq!(cancelling.phase, OperationPhase::Active);

    let finished = state
        .cancel(&operation_id, "Processing was cancelled".to_string(), 200)
        .expect("cancelled");
    assert_eq!(finished.status, WorkOperationStatus::Cancelled);
    assert_eq!(finished.phase, OperationPhase::Finished);
    assert!(finished.is_finished());
}
