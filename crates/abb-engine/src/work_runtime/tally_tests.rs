use super::snapshot::new_processing_snapshot;
use super::state::WorkRuntimeState;
use super::{ChildCounts, ChildJobStatus, OperationId, OperationSnapshot, WorkOperationStatus};
use crate::processing::{
    EventStage, ProcessCommandResult, ProcessResultEntry, ProcessResultStatus, ProgressEvent,
};

fn three_title_batch() -> (WorkRuntimeState, OperationId) {
    let mut state = WorkRuntimeState::default();
    let operation_id = OperationId("batch".to_string());
    state.insert_operation(new_processing_snapshot(
        operation_id.clone(),
        1,
        "Batch".to_string(),
        &[
            "/a.m4b".to_string(),
            "/b.m4b".to_string(),
            "/c.m4b".to_string(),
        ],
        None,
    ));
    (state, operation_id)
}

fn event(stage: EventStage, input_index: usize) -> ProgressEvent {
    ProgressEvent {
        stage,
        percentage: 0.0,
        message: String::new(),
        current_file: None,
        eta_seconds: None,
        job_id: Some(format!("job-{input_index}")),
        input_index: Some(input_index),
    }
}

fn places(snapshot: &OperationSnapshot) -> Vec<Option<(u32, u32)>> {
    snapshot
        .children
        .iter()
        .map(|child| match child.status {
            ChildJobStatus::Queued { position, of } => Some((position, of)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_queued_title_reads_its_place_among_the_titles_still_waiting() {
    let (mut state, id) = three_title_batch();
    let accepted = state.operation(id.as_str()).expect("accepted").clone();
    assert_eq!(
        places(&accepted),
        [Some((1, 3)), Some((2, 3)), Some((3, 3))]
    );

    state.mark_running(&id, 10).expect("running");
    let started = state
        .apply_progress_event(&id, &event(EventStage::Analyzing, 0), 20)
        .expect("first title starts");
    assert_eq!(places(&started), [None, Some((1, 2)), Some((2, 2))]);

    let (requested, _) = state
        .request_child_cancel(&id, "input-1")
        .expect("cancel the first queued title");
    assert_eq!(
        places(&requested),
        [None, Some((1, 2)), Some((2, 2))],
        "a title keeps its place until the engine settles its cancel"
    );

    let settled = state
        .apply_progress_event(&id, &event(EventStage::Cancelled, 1), 30)
        .expect("cancel settles");
    assert_eq!(places(&settled), [None, None, Some((1, 1))]);
}

fn counts(queued: u32, running: u32, completed: u32, cancelled: u32, failed: u32) -> ChildCounts {
    ChildCounts {
        queued,
        running,
        completed,
        skipped: 0,
        cancelled,
        failed,
    }
}

#[test]
fn the_counts_follow_a_run_with_one_success_one_failure_and_one_cancel() {
    let (mut state, id) = three_title_batch();
    let accepted = state.operation(id.as_str()).expect("accepted").clone();
    assert_eq!(accepted.counts, counts(3, 0, 0, 0, 0));

    state.mark_running(&id, 10).expect("running");
    let first = state
        .apply_progress_event(&id, &event(EventStage::Converting, 0), 20)
        .expect("first title starts");
    assert_eq!(first.counts, counts(2, 1, 0, 0, 0));

    state.request_child_cancel(&id, "input-2").expect("cancel");
    let cancelled = state
        .apply_progress_event(&id, &event(EventStage::Cancelled, 2), 30)
        .expect("last title settles cancelled");
    assert_eq!(cancelled.counts, counts(1, 1, 0, 1, 0));

    state
        .apply_progress_event(&id, &event(EventStage::Completed, 0), 40)
        .expect("first title completes");
    let second = state
        .apply_progress_event(&id, &event(EventStage::Converting, 1), 50)
        .expect("second title starts");
    assert_eq!(second.counts, counts(0, 1, 1, 1, 0));

    let failed = state
        .apply_progress_event(&id, &event(EventStage::Failed, 1), 60)
        .expect("second title fails");
    assert_eq!(failed.counts, counts(0, 0, 1, 1, 1));

    let result = ProcessCommandResult::new(
        [
            (0, ProcessResultStatus::Success),
            (1, ProcessResultStatus::Failed),
            (2, ProcessResultStatus::Cancelled),
        ]
        .map(|(input_index, status)| ProcessResultEntry {
            input_index,
            status,
            message: String::new(),
            error: None,
            output_path: None,
            job_id: None,
            supplemental_warning: None,
            warnings: Vec::new(),
        })
        .to_vec(),
    );
    let finished = state
        .complete_from_process_result(&id, &result, 70)
        .expect("run ends");
    assert_eq!(finished.counts, counts(0, 0, 1, 1, 1));
    assert_eq!(finished.status, WorkOperationStatus::Mixed);
}
