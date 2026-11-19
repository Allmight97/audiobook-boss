use super::state_tests::{accepted_state, converting_event};
use super::WorkProgressStage;

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
