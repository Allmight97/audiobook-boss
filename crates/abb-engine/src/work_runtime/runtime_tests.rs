use super::*;
use std::path::Path;

fn accepted_runtime(source: &Path) -> (WorkRuntime, OperationId) {
    let runtime = WorkRuntime::new(crate::engine_tasks::EngineTasks::default());
    let operation_id = OperationId("operation-1".to_string());
    let snapshot = new_processing_snapshot(
        operation_id.clone(),
        1,
        "Book".to_string(),
        &[source.display().to_string()],
        None,
    );
    runtime.lock_operations().expect("operations lock").accept(
        snapshot,
        Live {
            titles: CancelTokens::per_title(1),
            sources: vec![vec![source.to_path_buf()]],
        },
    );
    (runtime, operation_id)
}

fn poison(runtime: &WorkRuntime) {
    let poisoned = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _guard = runtime.inner.operations.lock().expect("operations lock");
                panic!("poison the operations lock");
            })
            .join()
    });
    assert!(poisoned.is_err());
    assert!(runtime.inner.operations.is_poisoned());
}

#[test]
fn a_source_is_in_use_from_registration_before_the_first_progress_event() {
    let source = PathBuf::from("/library/book.m4b");
    let (runtime, operation_id) = accepted_runtime(&source);

    let snapshot = runtime.list_operations().expect("operations").operations;
    assert_eq!(snapshot[0].operation_id, operation_id);
    assert_eq!(snapshot[0].status, WorkOperationStatus::Accepted);
    assert_eq!(runtime.sources_in_use(), HashSet::from([source.clone()]));
    assert_eq!(runtime.sources_held(), HashSet::from([source]));
}

#[test]
fn sources_in_use_still_reports_queued_sources_after_the_lock_is_poisoned() {
    let source = PathBuf::from("/library/book.m4b");
    let (runtime, _) = accepted_runtime(&source);

    poison(&runtime);

    assert_eq!(runtime.sources_in_use(), HashSet::from([source]));
}

#[test]
fn sources_held_still_reports_every_source_after_the_lock_is_poisoned() {
    let source = PathBuf::from("/library/book.m4b");
    let (runtime, _) = accepted_runtime(&source);

    poison(&runtime);

    assert_eq!(runtime.sources_held(), HashSet::from([source]));
}

#[test]
fn a_cancel_is_visible_as_snapshot_and_token_together() {
    let source = PathBuf::from("/library/book.m4b");
    let (runtime, operation_id) = accepted_runtime(&source);
    let tokens = runtime
        .lock_operations()
        .expect("operations lock")
        .live_tokens(&operation_id);
    assert!(tokens.iter().all(|token| !token.is_cancelled()));

    runtime
        .cancel_whole_operation(&operation_id)
        .expect("cancel the operation");

    let operations = runtime.lock_operations().expect("operations lock");
    let status = operations
        .state
        .operation(operation_id.as_str())
        .map(|operation| operation.status);
    assert_eq!(status, Some(WorkOperationStatus::Cancelling));
    assert!(tokens.iter().all(|token| token.is_cancelled()));
}

struct LockProbe {
    runtime: std::sync::OnceLock<WorkRuntime>,
    seen: Mutex<Vec<(bool, usize)>>,
}

impl crate::EventSink for LockProbe {
    fn emit(&self, _event: crate::EngineEvent) {
        let runtime = self.runtime.get().expect("probe is wired to the runtime");
        let lock_is_free = runtime.inner.operations.try_lock().is_ok();
        let held = runtime.sources_held().len();
        self.seen
            .lock()
            .expect("probe lock")
            .push((lock_is_free, held));
    }
}

#[test]
fn finishing_an_export_frees_its_sources_before_anyone_hears_of_it_and_emits_outside_the_lock() {
    let source = PathBuf::from("/library/book.m4b");
    let (runtime, operation_id) = accepted_runtime(&source);
    let probe = Arc::new(LockProbe {
        runtime: std::sync::OnceLock::new(),
        seen: Mutex::new(Vec::new()),
    });
    assert!(probe.runtime.set(runtime.clone()).is_ok());
    let host = Host::new(probe.clone(), crate::power::PowerManager::default());
    let result = crate::processing::ProcessCommandResult::new(Vec::new());

    let finished = runtime
        .finish_processing_and_emit(&host, &operation_id, Ok(result))
        .expect("terminal snapshot");

    assert!(finished.is_finished());
    assert!(runtime.sources_held().is_empty());
    assert!(runtime.sources_in_use().is_empty());
    assert_eq!(
        *probe.seen.lock().expect("probe lock"),
        [(true, 0)],
        "one event, sent with the lock free and the sources already released"
    );
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
    );

    let accepted = format_work_operation_record(WorkOperationLogEvent::Accepted, &snapshot);
    assert_eq!(
        accepted,
        "work_operation event=accepted operation_id=operation-123 kind=processing_batch status=accepted total=2 succeeded=0 skipped=0 cancelled=0 failed=0"
    );
    assert!(!accepted.contains("/private/library"));

    snapshot.set_status(WorkOperationStatus::Mixed);
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

#[derive(Default)]
struct PublishedLocks(Mutex<Vec<(u64, bool)>>);

impl crate::EventSink for PublishedLocks {
    fn emit(&self, event: EngineEvent) {
        if let EngineEvent::WorkOperations(update) = event {
            assert!(update.changed.is_none(), "only the lock changed");
            self.0
                .lock()
                .expect("published")
                .push((update.revision, update.concurrency_locked));
        }
    }
}

#[test]
fn the_concurrency_lock_is_published_with_every_job_and_run_change_and_with_the_snapshot() {
    let registry: crate::ManagedJobRegistry = Arc::new(crate::processing::JobRegistry::new(2));
    let published = Arc::new(PublishedLocks::default());
    let host = Host::new(published.clone(), crate::power::PowerManager::default());
    let runtime = WorkRuntime::new(crate::engine_tasks::EngineTasks::default());
    runtime.follow_concurrency_lock(&host, &registry);
    assert!(
        !runtime
            .list_operations()
            .expect("snapshot")
            .concurrency_locked
    );

    let tagging = registry.register_job_without_permit();
    let second = registry.register_job_without_permit();
    assert!(
        runtime
            .list_operations()
            .expect("snapshot")
            .concurrency_locked
    );
    let run = registry.hold_run();
    registry.complete_job(tagging);
    registry.complete_job(second);
    assert!(
        runtime
            .list_operations()
            .expect("snapshot")
            .concurrency_locked,
        "a held run keeps the lock"
    );
    drop(run);

    let snapshot = runtime.list_operations().expect("snapshot");
    assert!(!snapshot.concurrency_locked);
    let published = published.0.lock().expect("published").clone();
    assert_eq!(
        published
            .iter()
            .map(|(_, locked)| *locked)
            .collect::<Vec<_>>(),
        [true, false]
    );
    assert_eq!(published[1].0, snapshot.revision);
}
