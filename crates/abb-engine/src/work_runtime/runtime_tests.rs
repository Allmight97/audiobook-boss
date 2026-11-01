use super::*;

#[test]
fn sources_in_use_still_reports_queued_sources_after_the_state_lock_is_poisoned() {
    let runtime = WorkRuntime::default();
    let source = PathBuf::from("/library/book.m4b");
    let operation_id = OperationId("operation-1".to_string());
    let snapshot = new_processing_snapshot(
        operation_id.clone(),
        1,
        "Book".to_string(),
        &[source.display().to_string()],
        None,
        0,
    );
    lock_state(&runtime.inner.state)
        .expect("state lock")
        .insert_operation(snapshot);
    runtime
        .title_sources()
        .insert(operation_id.0.clone(), vec![vec![source.clone()]]);

    let poisoned = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _guard = runtime.inner.state.lock().expect("state lock");
                panic!("poison the state lock");
            })
            .join()
    });
    assert!(poisoned.is_err());
    assert!(runtime.inner.state.is_poisoned());

    assert_eq!(runtime.sources_in_use(), HashSet::from([source]));
}

#[test]
fn cancelling_one_title_sets_only_its_flag_and_whole_cancel_sets_all() {
    let flags = CancelFlags::per_title(3);
    let titles = flags.titles();

    assert!(flags.cancel_title(1));
    assert!(!flags.cancel_title(3), "unknown titles are not cancelled");
    let cancelled: Vec<_> = titles
        .iter()
        .map(|flag| flag.load(Ordering::Acquire))
        .collect();
    assert_eq!(cancelled, [false, true, false]);

    flags.cancel_all();
    assert!(titles.iter().all(|flag| flag.load(Ordering::Acquire)));
}

#[test]
fn work_operation_record_format_is_stable_and_path_free() {
    let mut snapshot = new_processing_snapshot(
        OperationId("operation-123".to_string()),
        1,
        "Batch encode".to_string(),
        &[
            "/private/library/first.m4b".to_string(),
            "/private/library/second.m4b".to_string(),
        ],
        None,
        100,
    );

    let accepted = format_work_operation_record(WorkOperationLogEvent::Accepted, &snapshot);
    assert_eq!(
        accepted,
        "work_operation event=accepted operation_id=operation-123 kind=processing_batch status=accepted total=2 succeeded=0 skipped=0 cancelled=0 failed=0"
    );
    assert!(!accepted.contains("/private/library"));

    snapshot.status = WorkOperationStatus::Mixed;
    snapshot.terminal_summary = Some(super::super::OperationTerminalSummary {
        total: 2,
        succeeded: 1,
        skipped: 0,
        cancelled: 0,
        failed: 1,
        message: "Mixed result".to_string(),
    });
    assert_eq!(
        format_work_operation_record(WorkOperationLogEvent::Terminal, &snapshot),
        "work_operation event=terminal operation_id=operation-123 kind=processing_batch status=mixed total=2 succeeded=1 skipped=0 cancelled=0 failed=1"
    );
}

#[test]
fn work_operation_record_labels_pin_all_contract_variants() {
    assert_eq!(
        [
            WorkOperationStatus::Accepted,
            WorkOperationStatus::Running,
            WorkOperationStatus::Cancelling,
            WorkOperationStatus::Completed,
            WorkOperationStatus::Cancelled,
            WorkOperationStatus::Failed,
            WorkOperationStatus::Mixed,
        ]
        .map(work_operation_status_label),
        [
            "accepted",
            "running",
            "cancelling",
            "completed",
            "cancelled",
            "failed",
            "mixed",
        ]
    );
    assert_eq!(
        [
            WorkOperationLogEvent::Accepted,
            WorkOperationLogEvent::Running,
            WorkOperationLogEvent::CancelRequested,
            WorkOperationLogEvent::Terminal,
        ]
        .map(work_operation_event_label),
        ["accepted", "running", "cancel_requested", "terminal"]
    );
}
