use super::*;

#[test]
fn terminal_events_share_context_and_reset_progress() {
    let emitter = ProgressEmitter {
        operation_kind: OperationKind::ProcessingBatch,
        job_id: Some("job-123".to_string()),
        input_index: Some(7),
        progress_listener: None,
    };

    let failed = emitter.terminal_event(EventStage::Failed, "failed");
    let cancelled = emitter.terminal_event(EventStage::Cancelled, "cancelled");
    let skipped = emitter.terminal_event(EventStage::Skipped, "skipped");

    assert_eq!(failed.stage, EventStage::Failed);
    assert_eq!(cancelled.stage, EventStage::Cancelled);
    assert_eq!(skipped.stage, EventStage::Skipped);
    assert_eq!(failed.operation_kind, OperationKind::ProcessingBatch);
    assert_eq!(cancelled.operation_kind, OperationKind::ProcessingBatch);
    assert_eq!(skipped.operation_kind, OperationKind::ProcessingBatch);
    assert_eq!(failed.percentage, 0.0);
    assert_eq!(cancelled.percentage, 0.0);
    assert_eq!(skipped.percentage, 100.0);
    assert_eq!(failed.job_id, Some("job-123".to_string()));
    assert_eq!(cancelled.job_id, Some("job-123".to_string()));
    assert_eq!(skipped.job_id, Some("job-123".to_string()));
    assert_eq!(failed.input_index, Some(7));
    assert_eq!(cancelled.input_index, Some(7));
    assert_eq!(skipped.input_index, Some(7));
    assert_eq!(failed.current_file, None);
    assert_eq!(cancelled.current_file, None);
    assert_eq!(skipped.current_file, None);
    assert_eq!(failed.eta_seconds, None);
    assert_eq!(cancelled.eta_seconds, None);
    assert_eq!(skipped.eta_seconds, None);
}

#[test]
fn emitted_progress_normalizes_non_finite_percentage() {
    let captured = Arc::new(std::sync::Mutex::new(None));
    let listener_capture = Arc::clone(&captured);
    let listener: ProgressListener = Arc::new(move |event| {
        *listener_capture.lock().expect("capture progress event") = Some(event.clone());
    });
    let emitter = ProgressEmitter::with_context(EmitContext {
        operation_kind: OperationKind::ProcessingBatch,
        job_id: None,
        input_index: None,
    })
    .with_progress_listener(Some(listener));

    emitter.emit_custom(
        ProcessingStage::Converting,
        f32::NAN,
        "Encoding",
        None,
        None,
    );

    let event = captured
        .lock()
        .expect("read progress event")
        .clone()
        .expect("progress event was emitted");
    assert_eq!(event.percentage, 0.0);
    assert!(event.percentage.is_finite());
}
