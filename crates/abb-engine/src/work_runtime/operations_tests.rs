use super::operations::{CancelFlags, Live, Operations};
use crate::errors::AppError;
use crate::processing::{
    EventStage, OperationKind, ProcessCommandResult, ProcessResultEntry, ProcessResultStatus,
    ProgressEvent,
};
use crate::work_runtime::snapshot::new_processing_snapshot;
use crate::work_runtime::{OperationId, WorkOperationStatus};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

fn first() -> PathBuf {
    PathBuf::from("/library/first.m4b")
}

fn second() -> PathBuf {
    PathBuf::from("/library/second.m4b")
}

fn accepted() -> (Operations, OperationId) {
    let mut operations = Operations::new();
    let id = OperationId("op-1".to_string());
    let snapshot = new_processing_snapshot(
        id.clone(),
        1,
        "Two books".to_string(),
        &[
            first().display().to_string(),
            second().display().to_string(),
        ],
        None,
    );
    operations.accept(
        snapshot,
        Live {
            titles: CancelFlags::per_title(2),
            sources: vec![vec![first()], vec![second()]],
        },
    );
    (operations, id)
}

fn completed(index: usize) -> ProgressEvent {
    ProgressEvent {
        operation_kind: OperationKind::ProcessingBatch,
        stage: EventStage::Completed,
        percentage: 100.0,
        message: "done".to_string(),
        current_file: None,
        eta_seconds: None,
        job_id: Some(format!("job-{index}")),
        input_index: Some(index),
    }
}

fn result_of(statuses: &[ProcessResultStatus]) -> ProcessCommandResult {
    ProcessCommandResult::new(
        statuses
            .iter()
            .enumerate()
            .map(|(input_index, status)| ProcessResultEntry {
                input_index,
                status: *status,
                message: String::new(),
                error: None,
                output_path: None,
                supplemental_warning: None,
                warnings: Vec::new(),
                job_id: None,
            })
            .collect(),
    )
}

#[test]
fn every_source_is_in_use_from_acceptance_before_any_progress_event() {
    let (operations, id) = accepted();

    assert_eq!(
        operations.sources_in_use(),
        HashSet::from([first(), second()])
    );
    assert_eq!(
        operations.sources_held(),
        HashSet::from([first(), second()])
    );
    assert_eq!(
        operations.state.operation(id.as_str()).map(|o| o.status),
        Some(WorkOperationStatus::Accepted)
    );
}

#[test]
fn a_finished_title_stops_being_read_but_stays_held_until_the_operation_finishes() {
    let (mut operations, id) = accepted();
    operations
        .state
        .apply_progress_event(&id, &completed(0), 10)
        .expect("progress");

    assert_eq!(operations.sources_in_use(), HashSet::from([second()]));
    assert_eq!(
        operations.sources_held(),
        HashSet::from([first(), second()])
    );

    let result = result_of(&[ProcessResultStatus::Success, ProcessResultStatus::Success]);
    let finished = operations
        .finish(&id, |state| {
            state.complete_from_process_result(&id, &result, 20)
        })
        .expect("finish");

    assert!(finished.is_finished());
    assert!(operations.sources_in_use().is_empty());
    assert!(operations.sources_held().is_empty());
    assert!(!operations.is_live(&id));
}

#[test]
fn finishing_drops_the_live_record_even_when_the_terminal_snapshot_fails() {
    let (mut operations, id) = accepted();

    let outcome = operations.finish(&id, |_| Err(AppError::General("no snapshot".to_string())));

    assert!(outcome.is_err());
    assert!(!operations.is_live(&id));
    assert!(operations.sources_held().is_empty());
}

#[test]
fn cancelling_one_title_sets_only_its_flag_and_whole_cancel_sets_all() {
    let (mut operations, id) = accepted();
    let flags = operations.live_flags(&id);

    let (snapshot, cancelled) = operations.cancel_title(&id, "input-1").expect("cancel");
    assert!(cancelled);
    assert!(snapshot.children[1].cancel_requested);
    let state: Vec<_> = flags.iter().map(|f| f.load(Ordering::Acquire)).collect();
    assert_eq!(state, [false, true]);

    let (_, again) = operations.cancel_title(&id, "input-1").expect("repeat");
    assert!(!again, "a repeated title cancel changes nothing");

    let snapshot = operations.cancel_all(&id, 5).expect("cancel all");
    assert_eq!(snapshot.status, WorkOperationStatus::Cancelling);
    assert!(flags.iter().all(|f| f.load(Ordering::Acquire)));
}

#[test]
fn cancelling_a_finished_operation_sets_no_flag_and_returns_its_snapshot() {
    let (mut operations, id) = accepted();
    let flags = operations.live_flags(&id);
    let result = result_of(&[ProcessResultStatus::Success, ProcessResultStatus::Success]);
    operations
        .finish(&id, |state| {
            state.complete_from_process_result(&id, &result, 20)
        })
        .expect("finish");

    let snapshot = operations.cancel_all(&id, 30).expect("cancel finished");

    assert_eq!(snapshot.status, WorkOperationStatus::Completed);
    assert!(flags.iter().all(|f| !f.load(Ordering::Acquire)));
}
