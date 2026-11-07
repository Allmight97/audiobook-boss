use super::snapshot::{new_metadata_save_snapshot, new_processing_snapshot};
use super::state::WorkRuntimeState;
use super::{ChildJobStatus, OperationId, WorkOperationStatus, WorkProgressStage};
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

#[test]
fn snapshots_revision_each_operation_independently() {
    let (mut state, operation_id) = accepted_state();
    let accepted = state
        .operation(operation_id.as_str())
        .cloned()
        .expect("accepted snapshot");
    assert_eq!(accepted.revision, 1);

    let running = state.mark_running(&operation_id, 150).expect("start");
    let progress = state
        .apply_progress_event(&operation_id, &converting_event(50.0, "Encoding", 0), 155)
        .expect("progress");
    let cancelling = state
        .request_cancel(&operation_id, 160)
        .expect("request cancel");
    let terminal = state
        .cancel(&operation_id, "Cancelled".to_string(), 170)
        .expect("cancel");
    assert!(accepted.revision < running.revision);
    assert!(running.revision < progress.revision);
    assert!(progress.revision < cancelling.revision);
    assert!(cancelling.revision < terminal.revision);
    assert_eq!(terminal.sequence, accepted.sequence);
    assert_eq!(state.list().operations[0].revision, terminal.revision);

    let next = state.insert_operation(new_metadata_save_snapshot(
        OperationId("op-2".to_string()),
        2,
        "Tags".to_string(),
        &["/tmp/new.m4b".to_string()],
    ));
    assert_eq!(next.revision, 1);
    assert_eq!(
        state
            .operation(operation_id.as_str())
            .expect("terminal snapshot")
            .revision,
        terminal.revision
    );

    let metadata_id = OperationId("op-2".to_string());
    let metadata_progress = state
        .apply_progress_event(
            &metadata_id,
            &ProgressEvent {
                operation_kind: OperationKind::MetadataSave,
                stage: EventStage::Writing,
                job_id: None,
                ..converting_event(40.0, "Saving tags", 0)
            },
            190,
        )
        .expect("metadata progress");
    let metadata_terminal = state
        .complete_from_summary(
            &metadata_id,
            &OperationResultSummary {
                total: 1,
                succeeded: 1,
                skipped: 0,
                cancelled: 0,
                failed: 0,
            },
            &[(0, ProcessResultStatus::Success, "Saved".to_string())],
            200,
        )
        .expect("metadata complete");
    assert!(next.revision < metadata_progress.revision);
    assert!(metadata_progress.revision < metadata_terminal.revision);
}

#[test]
fn accepted_operation_preserves_immutable_child_identity() {
    let (state, operation_id) = accepted_state();
    let snapshot = state
        .operation(operation_id.as_str())
        .cloned()
        .expect("snapshot");

    assert_eq!(snapshot.status, WorkOperationStatus::Accepted);
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
            1_000,
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
    let result = ProcessCommandResult::new(vec![
        ProcessResultEntry {
            input_index: 0,
            status: ProcessResultStatus::Success,
            message: "first ok".to_string(),
            error: None,
            output_path: Some("/library/first.m4b".to_string()),
            supplemental_warning: None,
            warnings: Vec::new(),
            job_id: Some("job-1".to_string()),
        },
        ProcessResultEntry {
            input_index: 1,
            status: ProcessResultStatus::Failed,
            message: "second failed".to_string(),
            error: None,
            output_path: None,
            supplemental_warning: None,
            warnings: Vec::new(),
            job_id: Some("job-2".to_string()),
        },
    ]);

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
    assert_eq!(
        snapshot.children[0].output_path.as_deref(),
        Some("/library/first.m4b")
    );
}

#[test]
fn partial_publication_completes_child_and_carries_its_warning() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");
    let mut published_without_pdf = entry(0, ProcessResultStatus::Success);
    published_without_pdf.supplemental_warning = Some("PDF was not saved".to_string());
    let result = ProcessCommandResult::new(vec![
        published_without_pdf,
        entry(1, ProcessResultStatus::Success),
    ]);

    let snapshot = state
        .complete_from_process_result(&operation_id, &result, 300)
        .expect("complete");

    assert_eq!(snapshot.status, WorkOperationStatus::Completed);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Completed);
    assert_eq!(
        snapshot.children[0].supplemental_warning.as_deref(),
        Some("PDF was not saved")
    );
    assert_eq!(snapshot.children[1].supplemental_warning, None);
    assert!(snapshot.log_tail.iter().any(|entry| {
        entry.message == "PDF was not saved" && entry.child_job_id.as_deref() == Some("input-0")
    }));
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
            1_000,
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
            1_000,
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
            1_000,
        )
        .expect("progress");

    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Failed);
}

fn single_title_state(id: &str) -> (WorkRuntimeState, OperationId) {
    let mut state = WorkRuntimeState::default();
    let operation_id = OperationId(id.to_string());
    state.insert_operation(new_processing_snapshot(
        operation_id.clone(),
        1,
        "Book".to_string(),
        &["/tmp/only.m4b".to_string()],
        None,
    ));
    state.mark_running(&operation_id, 150).expect("running");
    (state, operation_id)
}

#[test]
fn a_single_title_export_takes_its_titles_stage_and_fails_with_it() {
    let (mut state, operation_id) = single_title_state("single-complete");
    let snapshot = state
        .apply_progress_event(
            &operation_id,
            &ProgressEvent {
                stage: EventStage::Completed,
                ..converting_event(100.0, "done", 0)
            },
            1_000,
        )
        .expect("progress");
    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert_eq!(snapshot.progress.stage, WorkProgressStage::Complete);
    assert_eq!(snapshot.progress.percentage, 100.0);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Completed);

    let (mut state, operation_id) = single_title_state("single-failed");
    let snapshot = state
        .apply_progress_event(
            &operation_id,
            &ProgressEvent {
                stage: EventStage::Failed,
                ..converting_event(0.0, "done", 0)
            },
            1_000,
        )
        .expect("progress");
    assert_eq!(snapshot.status, WorkOperationStatus::Failed);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Failed);
}

#[test]
fn success_plus_skipped_resolves_to_mixed_matching_canonical_classifier() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 150).expect("running");
    let result = ProcessCommandResult::new(vec![
        entry(0, ProcessResultStatus::Success),
        entry(1, ProcessResultStatus::Skipped),
    ]);

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
    let result = ProcessCommandResult::new(vec![
        entry(0, ProcessResultStatus::Skipped),
        entry(1, ProcessResultStatus::Cancelled),
    ]);

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
    let result = ProcessCommandResult::new(vec![
        entry(0, ProcessResultStatus::Skipped),
        entry(1, ProcessResultStatus::Skipped),
    ]);

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
    ));
    (state, operation_id)
}

#[test]
fn metadata_save_progress_updates_only_the_title_it_names() {
    let (mut state, operation_id) = metadata_save_state();
    state.mark_running(&operation_id, 150).expect("running");
    let event = |stage, percentage, message: &str, input_index| ProgressEvent {
        operation_kind: OperationKind::MetadataSave,
        stage,
        percentage,
        message: message.to_string(),
        current_file: None,
        eta_seconds: None,
        job_id: None,
        input_index: Some(input_index),
    };

    let snapshot = state
        .apply_progress_event(
            &operation_id,
            &event(EventStage::Writing, 50.0, "Writing b", 1),
            200,
        )
        .expect("progress");
    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert_eq!(snapshot.children[0].label, "a.m4b");
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Queued);
    assert_eq!(snapshot.children[1].status, ChildJobStatus::Running);
    assert_eq!(snapshot.children[1].progress.percentage, 50.0);

    let snapshot = state
        .apply_progress_event(
            &operation_id,
            &event(EventStage::Completed, 100.0, "Saved a", 0),
            300,
        )
        .expect("progress");
    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert_eq!(snapshot.children[0].status, ChildJobStatus::Completed);
    assert_eq!(snapshot.children[1].status, ChildJobStatus::Running);
    assert_eq!(snapshot.children[1].progress.percentage, 50.0);
    assert_eq!(snapshot.children[1].message.as_deref(), Some("Writing b"));
}

#[test]
fn complete_from_summary_terminalizes_metadata_save_children_by_index() {
    let (mut state, operation_id) = metadata_save_state();
    state.mark_running(&operation_id, 150).expect("running");

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

#[test]
fn cancel_terminalizes_metadata_save_operation_as_cancelled_not_failed() {
    let (mut state, operation_id) = metadata_save_state();
    state.mark_running(&operation_id, 150).expect("running");

    let snapshot = state
        .cancel(&operation_id, "Metadata save cancelled.".to_string(), 300)
        .expect("cancel");

    assert_eq!(snapshot.status, WorkOperationStatus::Cancelled);
    assert!(snapshot
        .children
        .iter()
        .all(|child| child.status == ChildJobStatus::Cancelled));
    assert!(!snapshot.cancellable);
    assert_eq!(snapshot.finished_at_ms, Some(300));
}

fn converting_event(percentage: f32, message: &str, input_index: usize) -> ProgressEvent {
    ProgressEvent {
        operation_kind: OperationKind::ProcessingBatch,
        stage: EventStage::Converting,
        percentage,
        message: message.to_string(),
        current_file: Some("/tmp/first.m4b".to_string()),
        eta_seconds: None,
        job_id: Some("job-1".to_string()),
        input_index: Some(input_index),
    }
}

#[test]
fn log_tail_records_lifecycle_and_progress_with_dedupe() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");

    state
        .apply_progress_event(
            &operation_id,
            &converting_event(10.0, "encoding chunk 1/12", 0),
            200,
        )
        .expect("progress");
    state
        .apply_progress_event(
            &operation_id,
            &converting_event(12.0, "encoding chunk 1/12", 0),
            300,
        )
        .expect("progress");
    let snapshot = state
        .apply_progress_event(
            &operation_id,
            &converting_event(20.0, "encoding chunk 2/12", 0),
            400,
        )
        .expect("progress");

    let messages: Vec<&str> = snapshot
        .log_tail
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "Processing started.",
            "encoding chunk 1/12",
            "encoding chunk 2/12"
        ]
    );
    assert_eq!(
        snapshot.log_tail[1].child_job_id.as_deref(),
        Some("input-0")
    );
}

#[test]
fn log_tail_is_bounded_drop_oldest() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");

    for step in 0..40 {
        let message = format!("encoding chunk {step}/40");
        state
            .apply_progress_event(
                &operation_id,
                &converting_event(step as f32, &message, 0),
                200 + i64::from(step),
            )
            .expect("progress");
    }

    let snapshot = state
        .operation(operation_id.as_str())
        .cloned()
        .expect("snapshot");
    assert_eq!(snapshot.log_tail.len(), 20);
    assert_eq!(snapshot.log_tail[0].message, "encoding chunk 20/40");
    assert_eq!(snapshot.log_tail[19].message, "encoding chunk 39/40");
}

#[test]
fn multi_child_batch_events_suppress_operation_eta() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");

    let mut event = converting_event(40.0, "encoding chunk 3/12", 0);
    event.eta_seconds = Some(42.0);
    let snapshot = state
        .apply_progress_event(&operation_id, &event, 200)
        .expect("progress");

    assert_eq!(snapshot.progress.eta_seconds, None);
    let child = snapshot
        .children
        .iter()
        .find(|child| child.input_index == Some(0))
        .expect("child");
    assert_eq!(child.progress.eta_seconds, Some(42.0));
}

#[test]
fn log_tail_keeps_identical_messages_from_distinct_children() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");

    let mut first = converting_event(10.0, "Encoding audio...", 0);
    first.job_id = Some("job-1".to_string());
    state
        .apply_progress_event(&operation_id, &first, 200)
        .expect("progress");

    let mut second = converting_event(10.0, "Encoding audio...", 1);
    second.job_id = Some("job-2".to_string());
    let snapshot = state
        .apply_progress_event(&operation_id, &second, 300)
        .expect("progress");

    let matching: Vec<_> = snapshot
        .log_tail
        .iter()
        .filter(|entry| entry.message == "Encoding audio...")
        .collect();
    assert_eq!(matching.len(), 2, "distinct children are real history");
    assert_ne!(matching[0].child_job_id, matching[1].child_job_id);
}

#[test]
fn prune_terminal_operations_keeps_cap_most_recent_and_never_prunes_active() {
    let mut state = WorkRuntimeState::default();

    for sequence in 1..=25u64 {
        let id = OperationId(format!("term-{sequence}"));
        state.insert_operation(new_processing_snapshot(
            id.clone(),
            sequence,
            format!("Batch {sequence}"),
            &["/tmp/f.m4b".to_string()],
            None,
        ));
        state
            .fail(&id, "boom".to_string(), 100 + sequence as i64)
            .expect("fail terminalizes and prunes");
    }

    let running_id = OperationId("running-op".to_string());
    state.insert_operation(new_processing_snapshot(
        running_id.clone(),
        1,
        "Running".to_string(),
        &["/tmp/r.m4b".to_string()],
        None,
    ));
    state.mark_running(&running_id, 500).expect("running");

    let list = state.list();
    assert_eq!(list.operations.len(), 21, "20 terminal + 1 running");
    assert!(state.operation(running_id.as_str()).is_some());

    for sequence in 1..=5u64 {
        assert!(
            state.operation(&format!("term-{sequence}")).is_none(),
            "sequence {sequence} should have been pruned"
        );
    }
    for sequence in 6..=25u64 {
        assert!(
            state.operation(&format!("term-{sequence}")).is_some(),
            "sequence {sequence} should be retained"
        );
    }
}

#[test]
fn prune_orders_by_terminalization_so_a_just_finished_long_runner_survives() {
    let mut state = WorkRuntimeState::default();

    let long_runner = OperationId("long-runner".to_string());
    state.insert_operation(new_processing_snapshot(
        long_runner.clone(),
        1,
        "Long batch".to_string(),
        &["/tmp/long.m4b".to_string()],
        None,
    ));
    state.mark_running(&long_runner, 100).expect("running");

    for sequence in 2..=26u64 {
        let id = OperationId(format!("term-{sequence}"));
        state.insert_operation(new_processing_snapshot(
            id.clone(),
            sequence,
            format!("Batch {sequence}"),
            &["/tmp/f.m4b".to_string()],
            None,
        ));
        state
            .fail(&id, "boom".to_string(), 100 + sequence as i64)
            .expect("fail terminalizes and prunes");
    }

    state
        .fail(&long_runner, "done late".to_string(), 10_000)
        .expect("long runner terminalizes");

    assert!(
        state.operation(long_runner.as_str()).is_some(),
        "the just-finished long-running operation must survive its own prune"
    );
    let list = state.list();
    assert_eq!(list.operations.len(), 20, "terminal cap holds");
    assert!(state.operation("term-2").is_none());
}

#[test]
fn log_tail_records_the_cancellation_request_transition() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");
    state.request_cancel(&operation_id, 400).expect("request");
    state
        .cancel(&operation_id, "Processing was cancelled.".to_string(), 500)
        .expect("cancel");

    let snapshot = state
        .operation(operation_id.as_str())
        .cloned()
        .expect("snapshot");
    let messages: Vec<&str> = snapshot
        .log_tail
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert!(messages.contains(&"Cancellation requested."));
    assert_eq!(messages.last(), Some(&"Processing was cancelled."));
    let last = snapshot.log_tail.last().expect("tail entry");
    assert_eq!(last.stage, Some(WorkProgressStage::Cancelled));
}

#[test]
fn child_completion_timing_excludes_queue_and_survives_later_batch_settlement() {
    let (mut state, operation_id) = accepted_state();
    state
        .mark_running(&operation_id, 150)
        .expect("start operation");
    assert_eq!(
        state
            .operation(operation_id.as_str())
            .expect("operation timing snapshot")
            .children[0]
            .started_at_ms,
        None
    );
    state
        .apply_progress_event(&operation_id, &converting_event(0.0, "Start", 0), 1_000)
        .expect("operation timing snapshot");
    let complete = ProgressEvent {
        stage: EventStage::Completed,
        ..converting_event(100.0, "Complete", 0)
    };
    let snapshot = state
        .apply_progress_event(&operation_id, &complete, 14_150)
        .expect("operation timing snapshot");
    assert_eq!(snapshot.children[0].started_at_ms, Some(1_000));
    assert_eq!(snapshot.children[0].finished_at_ms, Some(14_150));
    assert_eq!(snapshot.children[1].started_at_ms, None);
    state
        .apply_progress_event(&operation_id, &complete, 15_000)
        .expect("operation timing snapshot");
    let result = ProcessCommandResult::new(vec![
        entry(0, ProcessResultStatus::Success),
        entry(1, ProcessResultStatus::Skipped),
    ]);
    let snapshot = state
        .complete_from_process_result(&operation_id, &result, 20_000)
        .expect("operation timing snapshot");
    assert_eq!(snapshot.finished_at_ms, Some(20_000));
    assert_eq!(snapshot.children[0].finished_at_ms, Some(14_150));
    assert_eq!(snapshot.children[1].started_at_ms, None);
    assert_eq!(snapshot.children[1].finished_at_ms, None);
}

#[test]
fn cancelling_one_title_leaves_its_siblings_running_and_stays_cancelled() {
    let (mut state, operation_id) = accepted_state();
    state.mark_running(&operation_id, 100).expect("running");

    let (snapshot, title_index) = state
        .request_child_cancel(&operation_id, "input-0")
        .expect("cancel first title");
    let (_, repeated) = state
        .request_child_cancel(&operation_id, "input-0")
        .expect("repeat is harmless");
    let after_progress = state
        .apply_progress_event(&operation_id, &converting_event(40.0, "Encoding", 0), 120)
        .expect("late progress");

    assert_eq!(title_index, Some(0));
    assert_eq!(repeated, None);
    assert_eq!(snapshot.status, WorkOperationStatus::Running);
    assert!(snapshot.children[0].cancel_requested);
    assert!(!snapshot.children[1].cancel_requested && snapshot.children[1].cancellable);
    assert!(
        !after_progress.children[0].cancellable,
        "progress must not re-offer cancel for a title already cancelling"
    );

    let cancelled = ProgressEvent {
        stage: EventStage::Cancelled,
        job_id: None,
        message: "Processing was cancelled".to_string(),
        ..converting_event(0.0, "", 0)
    };
    let settled = state
        .apply_progress_event(&operation_id, &cancelled, 130)
        .expect("title cancelled");
    assert_eq!(settled.children[0].status, ChildJobStatus::Cancelled);
    assert_eq!(settled.children[1].status, ChildJobStatus::Queued);
    assert_eq!(settled.status, WorkOperationStatus::Running);
}

#[test]
fn metadata_save_items_cannot_be_cancelled_one_at_a_time() {
    let mut state = WorkRuntimeState::default();
    let operation_id = OperationId("save".to_string());
    state.insert_operation(new_metadata_save_snapshot(
        operation_id.clone(),
        1,
        "Tags".to_string(),
        &["/tmp/a.m4b".to_string(), "/tmp/b.m4b".to_string()],
    ));
    let running = state.mark_running(&operation_id, 110).expect("running");

    assert!(running.children.iter().all(|child| !child.cancellable));
    assert!(state
        .request_child_cancel(&operation_id, "metadata-0")
        .is_err());
}

#[test]
fn cancellation_and_failure_terminalize_queued_child_progress_for_reattachment() {
    for cancelled in [true, false] {
        let (mut state, id) = accepted_state();
        let terminal = if cancelled {
            state.cancel(&id, "Stopped".into(), 200)
        } else {
            state.fail(&id, "Failed".into(), 200)
        }
        .expect("terminalize queued operation");
        for child in terminal.children {
            assert_eq!(
                child.progress.stage,
                if cancelled {
                    WorkProgressStage::Cancelled
                } else {
                    WorkProgressStage::Failed
                }
            );
            assert_eq!(child.progress.percentage, 100.0);
            assert_eq!(child.finished_at_ms, Some(200));
            assert!(!child.cancellable);
        }
    }
}

#[test]
fn each_published_change_advances_the_revision_and_orders_active_work_first() {
    let (mut state, finished) = accepted_state();
    let done = state
        .fail(&finished, "boom".to_string(), 120)
        .expect("terminal");
    let first = state.publish(done);

    let running = OperationId("op-2".to_string());
    state.insert_operation(new_metadata_save_snapshot(
        running.clone(),
        2,
        "Tags".to_string(),
        &["/tmp/new.m4b".to_string()],
    ));
    let queued = OperationId("op-3".to_string());
    state.insert_operation(new_metadata_save_snapshot(
        queued.clone(),
        3,
        "Tags".to_string(),
        &["/tmp/other.m4b".to_string()],
    ));
    let started = state.mark_running(&running, 150).expect("running");
    let second = state.publish(started);

    assert!(second.revision > first.revision);
    assert_eq!(second.order, vec![running, queued, finished]);
    assert_eq!(state.list().order, second.order);
    assert_eq!(state.list().revision, second.revision);
}
