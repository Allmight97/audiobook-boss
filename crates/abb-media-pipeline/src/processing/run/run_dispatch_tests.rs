use super::report_stopped_titles;
use crate::errors::AppError;
use crate::processing::{
    EventStage, ProcessResultEntry, ProcessResultStatus, ProgressEvent, ProgressEventListener,
};
use std::sync::{Arc, Mutex};

#[test]
fn a_title_task_that_stopped_is_reported_failed_once_and_reported_titles_are_not_reported_again() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let listener: ProgressEventListener = Arc::new(move |event: &ProgressEvent| {
        sink.lock()
            .expect("events")
            .push((event.stage, event.input_index));
    });
    let reported = ProcessResultEntry {
        input_index: 0,
        status: ProcessResultStatus::Failed,
        message: "decoder unavailable".to_string(),
        error: None,
        output_path: None,
        supplemental_warning: None,
        warnings: Vec::new(),
        job_id: Some("job-1".to_string()),
    };

    let results = report_stopped_titles(
        vec![
            Ok(reported),
            Err(AppError::General(
                "Batch task join error: panicked".to_string(),
            )),
        ],
        "operation",
        &listener,
    );

    assert_eq!(results[0].status, ProcessResultStatus::Failed);
    assert_eq!(results[1].status, ProcessResultStatus::Failed);
    assert_eq!(results[1].input_index, 1);
    assert_eq!(
        *events.lock().expect("events"),
        [(EventStage::Failed, Some(1))]
    );
}
