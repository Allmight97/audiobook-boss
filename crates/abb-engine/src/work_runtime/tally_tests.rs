use super::snapshot::new_processing_snapshot;
use super::state::WorkRuntimeState;
use super::{ChildJobStatus, OperationId, OperationSnapshot};
use crate::processing::{EventStage, OperationKind, ProgressEvent};

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
        operation_kind: OperationKind::ProcessingBatch,
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
