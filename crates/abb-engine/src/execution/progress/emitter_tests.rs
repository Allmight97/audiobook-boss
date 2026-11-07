use super::*;
use std::sync::Mutex;

fn emitter_with_events(job_id: Option<&str>) -> (ProgressEmitter, Arc<Mutex<Vec<ProgressEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let listener: ProgressEventListener = Arc::new(move |event| {
        sink.lock()
            .expect("capture progress event")
            .push(event.clone());
    });
    let emitter = ProgressEmitter::with_context(EmitContext {
        operation_kind: OperationKind::ProcessingBatch,
        identity: JobIdentity {
            operation_id: None,
            job_id: job_id.map(str::to_string),
            input_index: 7,
        },
    })
    .with_progress_listener(Some(listener));
    (emitter, events)
}

#[test]
fn terminal_events_reach_the_listener_with_the_job_identity_and_reset_progress() {
    let (emitter, events) = emitter_with_events(Some("job-123"));

    emitter.emit_terminal_failed("failed");
    emitter.emit_terminal_cancelled("cancelled");
    emitter.emit_terminal_skipped("skipped");

    let events = events.lock().expect("read progress events");
    let stages: Vec<_> = events
        .iter()
        .map(|event| (event.stage, event.percentage))
        .collect();
    assert_eq!(
        stages,
        [
            (EventStage::Failed, 0.0),
            (EventStage::Cancelled, 0.0),
            (EventStage::Skipped, 100.0),
        ]
    );
    for event in events.iter() {
        assert_eq!(event.operation_kind, OperationKind::ProcessingBatch);
        assert_eq!(event.job_id.as_deref(), Some("job-123"));
        assert_eq!(event.input_index, Some(7));
        assert_eq!(event.current_file, None);
        assert_eq!(event.eta_seconds, None);
    }
}

#[test]
fn emitted_progress_normalizes_non_finite_percentage() {
    let (emitter, events) = emitter_with_events(None);

    emitter.emit_converting_progress(f32::NAN, "Encoding", None, None);

    let events = events.lock().expect("read progress events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].percentage, 0.0);
}
