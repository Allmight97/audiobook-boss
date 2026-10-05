use super::snapshot::{new_metadata_save_snapshot, new_processing_snapshot};
use super::state::WorkRuntimeState;
use super::types::{
    OperationId, OperationSnapshot, SubmitProcessingOperationRequest, WorkOperationStatus,
    WorkSubmissionAccepted,
};
use crate::errors::{AppError, Result};
use crate::host::{EngineEvent, Host};
use crate::processing::run::{
    preflight_title_outputs, process_inspected_with_options, ProcessingRunOptions,
};
use crate::processing::ProgressEventListener;
use crate::processing::TitleOutput;
use crate::processing::{OperationResultSummary, ProcessResultStatus, ProgressEvent};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

#[derive(Clone)]
pub struct WorkRuntime {
    inner: Arc<WorkRuntimeInner>,
}

/// Source paths by operation id, then title index.
type TitleSources = HashMap<String, Vec<Vec<PathBuf>>>;

/// Told an export's terminal snapshot.
pub(crate) type OnFinished = Box<dyn FnOnce(OperationSnapshot) + Send>;

struct WorkRuntimeInner {
    state: Mutex<WorkRuntimeState>,
    operation_cancel_flags: Mutex<HashMap<String, CancelFlags>>,
    /// Source files each accepted processing title reads, by operation id and
    /// title index. An entry lives until its operation finishes.
    title_sources: Mutex<TitleSources>,
    /// Accepted exports run here; closed when the engine shuts down.
    tasks: crate::engine::EngineTasks,
    /// Advances whenever an operation's state changes.
    changes: tokio::sync::watch::Sender<u64>,
    sequence: AtomicU64,
}

impl Default for WorkRuntime {
    fn default() -> Self {
        Self::new(crate::engine::EngineTasks::default())
    }
}

impl WorkRuntime {
    /// A runtime whose exports run as `tasks`, so the engine can wait for them.
    pub(crate) fn new(tasks: crate::engine::EngineTasks) -> Self {
        Self {
            inner: Arc::new(WorkRuntimeInner {
                tasks,
                state: Mutex::new(WorkRuntimeState::default()),
                operation_cancel_flags: Mutex::new(HashMap::new()),
                title_sources: Mutex::new(HashMap::new()),
                changes: tokio::sync::watch::channel(0).0,
                sequence: AtomicU64::new(1),
            }),
        }
    }
}

impl WorkRuntime {
    /// Accepts an export and runs it. `on_finished` receives the operation's
    /// terminal snapshot once it ends.
    pub(crate) async fn submit_processing_operation(
        &self,
        host: Host,
        registry: crate::ManagedJobRegistry,
        workspace_root: PathBuf,
        request: SubmitProcessingOperationRequest,
        on_finished: Option<OnFinished>,
        inspected: crate::processing::plan::InspectedProcessingPlan,
    ) -> Result<WorkSubmissionAccepted> {
        if self.inner.tasks.is_closed() {
            return Err(AppError::General("ABB is closing.".to_string()));
        }
        if request.title.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Processing operations need a title naming their books.".into(),
            ));
        }
        let titles =
            preflight_title_outputs(&request.payload, request.metadata.as_ref(), &inspected)?;
        let accepted = self.inner.tasks.admit(|| {
            self.register_processing_operation(
                host.clone(),
                registry,
                workspace_root,
                request,
                (titles, inspected),
                on_finished,
            )
        })??;
        log_work_operation(WorkOperationLogEvent::Accepted, &accepted.snapshot);
        self.emit_snapshot(&host, &accepted.snapshot);
        Ok(accepted)
    }

    fn register_processing_operation(
        &self,
        host: Host,
        registry: crate::ManagedJobRegistry,
        workspace_root: PathBuf,
        request: SubmitProcessingOperationRequest,
        planned: (
            Vec<Arc<TitleOutput>>,
            crate::processing::plan::InspectedProcessingPlan,
        ),
        on_finished: Option<OnFinished>,
    ) -> Result<WorkSubmissionAccepted> {
        let (titles, inspected) = planned;
        // Concurrency stays fixed from acceptance until the export ends,
        // including between titles when no job is registered.
        let active_run = registry.hold_run();
        let operation_id = request.operation_id.clone();
        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let title = request.title.trim().to_string();
        let input_ids = request.payload.input_ids.as_deref();
        let mut snapshot = new_processing_snapshot(
            operation_id.clone(),
            sequence,
            title,
            &request.payload.input_files,
            input_ids,
            now_ms(),
        );
        snapshot.source_input_ids = (0..request.payload.input_files.len())
            .flat_map(|index| request.payload.sources_for(index))
            .filter_map(|source| source.input_id)
            .collect();
        for child in &mut snapshot.children {
            if let Some(index) = child.input_index {
                child.source_input_ids = request
                    .payload
                    .sources_for(index)
                    .into_iter()
                    .filter_map(|source| source.input_id)
                    .collect();
            }
        }
        let cancel_flags = CancelFlags::per_title(request.payload.input_files.len());
        let title_cancels = cancel_flags.titles();
        let title_sources = title_source_paths(&request.payload);

        // Recorded before the operation becomes visible, so a file is never
        // reported free between acceptance and the first read.
        self.title_sources()
            .insert(operation_id.0.clone(), title_sources);
        let snapshot = lock_state(&self.inner.state)?.insert_operation(snapshot);
        {
            let mut flags = lock_cancel_flags(&self.inner.operation_cancel_flags)?;
            flags.insert(operation_id.0.clone(), cancel_flags);
        }

        for (index, title) in titles.iter().enumerate() {
            title.on_change(self.output_listener(&host, &operation_id, index));
        }

        let runtime = self.clone();
        let operation_id_for_task = operation_id.clone();
        let progress_runtime = runtime.clone();
        let progress_operation_id = operation_id_for_task.clone();
        let progress_host = host.clone();
        let run_titles = titles.clone();
        let progress_listener: Option<ProgressEventListener> =
            Some(std::sync::Arc::new(move |event: &ProgressEvent| {
                progress_runtime.apply_progress_and_emit(
                    &progress_host,
                    &progress_operation_id,
                    event,
                );
            }));
        self.inner.tasks.spawn(async move {
            let _active_run = active_run;
            runtime.mark_running_and_emit(&host, &operation_id_for_task);
            let result = process_inspected_with_options(
                &_active_run,
                host.clone(),
                registry,
                workspace_root,
                request.payload,
                inspected,
                ProcessingRunOptions {
                    operation_id: Some(operation_id_for_task.to_string()),
                    title_cancels,
                    progress_listener,
                    title_outputs: run_titles.clone(),
                },
            )
            .await;
            // A title the run never reached, or one skipped, has no output.
            for title in &run_titles {
                title.end();
            }
            runtime.release_title_sources(&operation_id_for_task);
            let finished =
                runtime.finish_processing_and_emit(&host, &operation_id_for_task, result);
            runtime.remove_cancel_flag(&operation_id_for_task);
            if let (Some(on_finished), Some(snapshot)) = (on_finished, finished) {
                on_finished(snapshot);
            }
        });

        Ok(WorkSubmissionAccepted {
            operation_id,
            snapshot,
            titles,
        })
    }

    /// Publishes a title's update state in its operation's snapshot.
    fn output_listener(
        &self,
        host: &Host,
        operation_id: &OperationId,
        index: usize,
    ) -> Box<dyn Fn(Option<crate::processing::OutputUpdate>) + Send + Sync> {
        let runtime: Weak<WorkRuntimeInner> = Arc::downgrade(&self.inner);
        let host = host.clone();
        let operation_id = operation_id.clone();
        Box::new(move |update| {
            let Some(inner) = runtime.upgrade() else {
                return;
            };
            let runtime = WorkRuntime { inner };
            let snapshot = lock_state(&runtime.inner.state)
                .ok()
                .and_then(|mut state| state.set_output_update(&operation_id, index, update));
            if let Some(snapshot) = snapshot {
                runtime.emit_snapshot(&host, &snapshot);
            }
        })
    }

    /// Cancels the title at `index` and waits until it has published or
    /// ended without an output, its own empty folders removed. A title still
    /// queued ends at once: it cannot publish with its cancel flag set, so
    /// a restart need not wait for the titles ahead of it. Returns whether
    /// it published.
    pub(crate) async fn stop_title(
        &self,
        host: &Host,
        operation_id: &OperationId,
        index: usize,
        title: &TitleOutput,
    ) -> bool {
        let child = lock_state(&self.inner.state)
            .ok()
            .and_then(|state| state.child_job_id(operation_id, index));
        if let Some(child) = child {
            match self.cancel_operation(host, operation_id.clone(), Some(child.clone())) {
                Ok(snapshot) => {
                    let queued = snapshot.children.iter().any(|candidate| {
                        candidate.child_job_id == child
                            && candidate.status == super::ChildJobStatus::Queued
                    });
                    if queued {
                        title.end();
                    }
                }
                Err(error) => log::warn!("Failed to cancel a title for restart: {error}"),
            }
        }
        title.settled().await
    }

    /// Begin a command-driven inline metadata-save operation: register the
    /// snapshot + operation cancel flag, emit it, and mark it running. The
    /// command then drives the save loop and terminalizes via
    /// `record_metadata_save_progress` + `finish_metadata_save_operation`.
    /// Returns the operation id and its cancel flag (wire into the registry
    /// cancellation checker so `cancel_work_operation` reaches the save loop).
    pub(crate) fn begin_metadata_save_operation(
        &self,
        host: &Host,
        input_files: &[String],
    ) -> Result<(OperationId, Arc<AtomicBool>)> {
        let operation_id = OperationId::new();
        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let count = input_files.len();
        let title = format!("Metadata save ({count} file{})", plural_suffix(count));
        let snapshot = new_metadata_save_snapshot(
            operation_id.clone(),
            sequence,
            title,
            input_files,
            now_ms(),
        );
        let cancel_flags = CancelFlags::per_title(1);
        let cancel_flag = Arc::clone(&cancel_flags.titles()[0]);

        let snapshot = lock_state(&self.inner.state)?.insert_operation(snapshot);
        {
            let mut flags = lock_cancel_flags(&self.inner.operation_cancel_flags)?;
            flags.insert(operation_id.0.clone(), cancel_flags);
        }

        log_work_operation(WorkOperationLogEvent::Accepted, &snapshot);
        self.emit_snapshot(host, &snapshot);
        self.mark_running_and_emit(host, &operation_id);

        Ok((operation_id, cancel_flag))
    }

    /// Apply a metadata-save progress event to its operation (Work Center
    /// renders the live snapshot). Events are piped by `input_index`.
    pub(crate) fn record_metadata_save_progress(
        &self,
        host: &Host,
        operation_id: &OperationId,
        event: &ProgressEvent,
    ) {
        self.apply_progress_and_emit(host, operation_id, event);
    }

    /// Terminalize a metadata-save operation from its run outcome, then emit
    /// and drop its cancel flag. A finished run maps its summary through the
    /// canonical classifier and terminalizes children by `input_index`; an
    /// aborted run resolves to Cancelled for a cancellation and Failed
    /// otherwise, exactly as a processing run does.
    pub(crate) fn finish_metadata_save_operation(
        &self,
        host: &Host,
        operation_id: &OperationId,
        outcome: std::result::Result<InlineRunTerminal<'_>, &AppError>,
    ) -> Result<OperationSnapshot> {
        let snapshot = {
            let mut state = lock_state(&self.inner.state)?;
            match outcome {
                Ok(run) => {
                    state.complete_from_summary(operation_id, run.summary, run.children, now_ms())
                }
                Err(error) => terminalize_aborted_run(&mut state, operation_id, error),
            }?
        };
        log_work_operation(WorkOperationLogEvent::Terminal, &snapshot);
        self.emit_snapshot(host, &snapshot);
        self.remove_cancel_flag(operation_id);
        Ok(snapshot)
    }

    fn apply_progress_and_emit(
        &self,
        host: &Host,
        operation_id: &OperationId,
        event: &ProgressEvent,
    ) {
        match lock_state(&self.inner.state)
            .and_then(|mut state| state.apply_progress_event(operation_id, event, now_ms()))
        {
            Ok(snapshot) => self.emit_snapshot(host, &snapshot),
            Err(error) => log::warn!(
                "Failed to apply progress event for operation {}: {}",
                operation_id,
                error
            ),
        }
    }

    pub fn list_operations(&self) -> Result<super::WorkOperationsSnapshot> {
        Ok(lock_state(&self.inner.state)?.list())
    }

    /// Accepted exports that have not finished. Metadata Saves are left out:
    /// shutdown lets them finish rather than cancelling them.
    pub(crate) fn unfinished_exports(&self) -> Vec<OperationId> {
        let Ok(state) = lock_state(&self.inner.state) else {
            return Vec::new();
        };
        state
            .list()
            .operations
            .into_iter()
            .filter(|operation| {
                operation.kind == crate::processing::OperationKind::ProcessingBatch
                    && !super::terminal::is_terminal(operation.status)
            })
            .map(|operation| operation.operation_id)
            .collect()
    }

    /// Cancels a whole operation, or one of its titles when `child_job_id`
    /// names a child. Repeating a cancel returns the current snapshot.
    pub(crate) fn cancel_operation(
        &self,
        host: &Host,
        operation_id: OperationId,
        child_job_id: Option<String>,
    ) -> Result<OperationSnapshot> {
        let snapshot = match child_job_id {
            None => self.cancel_whole_operation(&operation_id)?,
            Some(child_job_id) => self.cancel_title(&operation_id, &child_job_id)?,
        };
        self.emit_snapshot(host, &snapshot);
        Ok(snapshot)
    }

    fn cancel_whole_operation(&self, operation_id: &OperationId) -> Result<OperationSnapshot> {
        if let Some(flags) =
            lock_cancel_flags(&self.inner.operation_cancel_flags)?.get(operation_id.as_str())
        {
            flags.cancel_all();
        }
        let snapshot = lock_state(&self.inner.state)?.request_cancel(operation_id, now_ms())?;
        if snapshot.status == WorkOperationStatus::Cancelling {
            log_work_operation(WorkOperationLogEvent::CancelRequested, &snapshot);
        }
        Ok(snapshot)
    }

    fn cancel_title(
        &self,
        operation_id: &OperationId,
        child_job_id: &str,
    ) -> Result<OperationSnapshot> {
        let (snapshot, title_index) = lock_state(&self.inner.state)?.request_child_cancel(
            operation_id,
            child_job_id,
            now_ms(),
        )?;
        if let Some(index) = title_index {
            let flags = lock_cancel_flags(&self.inner.operation_cancel_flags)?;
            if flags
                .get(operation_id.as_str())
                .is_some_and(|flags| flags.cancel_title(index))
            {
                log::info!(
                    // Not a `work_operation` lifecycle record: dev-log analysis validates
                    // those events; the job's own terminal record carries the outcome.
                    "title_cancel_requested operation_id={operation_id} child_job_id={child_job_id}"
                );
            }
        }
        Ok(snapshot)
    }

    fn mark_running_and_emit(&self, host: &Host, operation_id: &OperationId) {
        match lock_state(&self.inner.state)
            .and_then(|mut state| state.mark_running(operation_id, now_ms()))
        {
            Ok(snapshot) => {
                if snapshot.status == WorkOperationStatus::Running {
                    log_work_operation(WorkOperationLogEvent::Running, &snapshot);
                }
                self.emit_snapshot(host, &snapshot);
            }
            Err(error) => log::warn!("Failed to mark work operation running: {}", error),
        }
    }

    fn finish_processing_and_emit(
        &self,
        host: &Host,
        operation_id: &OperationId,
        result: Result<crate::processing::ProcessCommandResult>,
    ) -> Option<OperationSnapshot> {
        let snapshot_result = lock_state(&self.inner.state).and_then(|mut state| match &result {
            Ok(result) => state.complete_from_process_result(operation_id, result, now_ms()),
            Err(error) => terminalize_aborted_run(&mut state, operation_id, error),
        });

        match snapshot_result {
            Ok(snapshot) => {
                log_work_operation(WorkOperationLogEvent::Terminal, &snapshot);
                self.emit_snapshot(host, &snapshot);
                Some(snapshot)
            }
            Err(error) => {
                log::warn!("Failed to terminalize work operation: {}", error);
                None
            }
        }
    }

    /// Source files accepted exports have yet to finish reading: every source
    /// of every title that is queued or running.
    pub(crate) fn sources_in_use(&self) -> HashSet<PathBuf> {
        let sources = self.title_sources();
        // Reporting nothing in use would let Save write a file an export is
        // reading, so a poisoned lock still answers from the data it holds.
        let state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut in_use = HashSet::new();
        for (operation_id, titles) in sources.iter() {
            let Some(operation) = state.operation(operation_id) else {
                continue;
            };
            for child in &operation.children {
                let reading = matches!(
                    child.status,
                    super::ChildJobStatus::Queued | super::ChildJobStatus::Running
                );
                if let Some(paths) = child.input_index.and_then(|index| titles.get(index)) {
                    if reading {
                        in_use.extend(paths.iter().cloned());
                    }
                }
            }
        }
        in_use
    }

    /// Every source of every export that has not finished, including titles
    /// already encoded: a title still copies its companion files after its
    /// audio completes.
    pub(crate) fn sources_held(&self) -> HashSet<PathBuf> {
        self.title_sources()
            .values()
            .flatten()
            .flatten()
            .cloned()
            .collect()
    }

    /// A receiver that wakes whenever any operation's state changes.
    pub(crate) fn subscribe_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.inner.changes.subscribe()
    }

    fn release_title_sources(&self, operation_id: &OperationId) {
        self.title_sources().remove(operation_id.as_str());
    }

    /// The map is replaced or edited in one step, so it stays usable after a
    /// panic elsewhere; skipping it would hide sources from Save.
    fn title_sources(&self) -> MutexGuard<'_, TitleSources> {
        self.inner
            .title_sources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn remove_cancel_flag(&self, operation_id: &OperationId) {
        if let Ok(mut flags) = lock_cancel_flags(&self.inner.operation_cancel_flags) {
            flags.remove(operation_id.as_str());
        }
    }

    /// Publishes a change with the display order as of now. The revision is
    /// taken under the state lock, so hosts can order updates however they
    /// arrive.
    fn emit_snapshot(&self, host: &Host, snapshot: &OperationSnapshot) {
        self.inner.changes.send_modify(|change| *change += 1);
        match lock_state(&self.inner.state) {
            Ok(mut state) => {
                let update = state.publish(snapshot.clone());
                drop(state);
                host.emit(EngineEvent::WorkOperations(update));
            }
            Err(error) => log::warn!("Failed to publish a work operation update: {error}"),
        }
    }
}

/// The source files each title reads, in the spelling import gives a file,
/// so a session can compare its sources against them.
fn title_source_paths(payload: &crate::processing::ProcessPayload) -> Vec<Vec<PathBuf>> {
    (0..payload.input_files.len())
        .map(|index| {
            payload
                .sources_for(index)
                .into_iter()
                .map(|source| {
                    let path = PathBuf::from(source.path);
                    std::fs::canonicalize(&path).unwrap_or(path)
                })
                .collect()
        })
        .collect()
}

#[derive(Clone, Copy)]
enum WorkOperationLogEvent {
    Accepted,
    Running,
    CancelRequested,
    Terminal,
}

#[derive(Default)]
struct WorkOperationLogCounts {
    total: usize,
    succeeded: usize,
    skipped: usize,
    cancelled: usize,
    failed: usize,
}

fn log_work_operation(event: WorkOperationLogEvent, snapshot: &OperationSnapshot) {
    log::info!("{}", format_work_operation_record(event, snapshot));
}

fn format_work_operation_record(
    event: WorkOperationLogEvent,
    snapshot: &OperationSnapshot,
) -> String {
    let counts = work_operation_log_counts(snapshot);
    format!(
        "work_operation event={} operation_id={} kind={} status={} total={} succeeded={} skipped={} cancelled={} failed={}",
        work_operation_event_label(event),
        snapshot.operation_id,
        crate::processing::operation_kind_log_label(snapshot.kind),
        work_operation_status_label(snapshot.status),
        counts.total,
        counts.succeeded,
        counts.skipped,
        counts.cancelled,
        counts.failed,
    )
}

fn work_operation_log_counts(snapshot: &OperationSnapshot) -> WorkOperationLogCounts {
    if let Some(summary) = &snapshot.terminal_summary {
        return WorkOperationLogCounts {
            total: summary.total,
            succeeded: summary.succeeded,
            skipped: summary.skipped,
            cancelled: summary.cancelled,
            failed: summary.failed,
        };
    }

    let mut counts = WorkOperationLogCounts {
        total: snapshot.children.len(),
        ..WorkOperationLogCounts::default()
    };
    for child in &snapshot.children {
        match child.status {
            super::ChildJobStatus::Completed => counts.succeeded += 1,
            super::ChildJobStatus::Skipped => counts.skipped += 1,
            super::ChildJobStatus::Cancelled => counts.cancelled += 1,
            super::ChildJobStatus::Failed => counts.failed += 1,
            super::ChildJobStatus::Queued | super::ChildJobStatus::Running => {}
        }
    }
    counts
}

fn work_operation_event_label(event: WorkOperationLogEvent) -> &'static str {
    match event {
        WorkOperationLogEvent::Accepted => "accepted",
        WorkOperationLogEvent::Running => "running",
        WorkOperationLogEvent::CancelRequested => "cancel_requested",
        WorkOperationLogEvent::Terminal => "terminal",
    }
}

fn work_operation_status_label(status: WorkOperationStatus) -> &'static str {
    match status {
        WorkOperationStatus::Accepted => "accepted",
        WorkOperationStatus::Running => "running",
        WorkOperationStatus::Cancelling => "cancelling",
        WorkOperationStatus::Completed => "completed",
        WorkOperationStatus::Cancelled => "cancelled",
        WorkOperationStatus::Failed => "failed",
        WorkOperationStatus::Mixed => "mixed",
    }
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

/// Result facts for a command-driven inline run: its summary and each child's
/// `(input_index, status, reason)` terminal.
pub struct InlineRunTerminal<'a> {
    pub summary: &'a OperationResultSummary,
    pub children: &'a [(usize, ProcessResultStatus, String)],
}

/// A run that aborted before producing results: cancellation → Cancelled,
/// anything else → Failed.
fn terminalize_aborted_run(
    state: &mut WorkRuntimeState,
    operation_id: &OperationId,
    error: &AppError,
) -> Result<OperationSnapshot> {
    match error {
        AppError::Cancellation(message) => state.cancel(operation_id, message.clone(), now_ms()),
        error => state.fail(operation_id, error.to_string(), now_ms()),
    }
}

/// Wall-clock milliseconds for operation snapshots.
pub(crate) fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn lock_state(state: &Mutex<WorkRuntimeState>) -> Result<MutexGuard<'_, WorkRuntimeState>> {
    state
        .lock()
        .map_err(|_| AppError::General("Work runtime state lock failed".to_string()))
}

/// One operation's cancel flags: one independent flag per output title for
/// processing; an inline metadata save uses a single flag for all its files.
struct CancelFlags(Vec<Arc<AtomicBool>>);

impl CancelFlags {
    fn per_title(count: usize) -> Self {
        Self(
            (0..count)
                .map(|_| Arc::new(AtomicBool::new(false)))
                .collect(),
        )
    }

    fn titles(&self) -> Vec<Arc<AtomicBool>> {
        self.0.clone()
    }

    fn cancel_all(&self) {
        self.0
            .iter()
            .for_each(|flag| flag.store(true, Ordering::Release));
    }

    fn cancel_title(&self, index: usize) -> bool {
        self.0
            .get(index)
            .map(|flag| flag.store(true, Ordering::Release))
            .is_some()
    }
}

fn lock_cancel_flags(
    flags: &Mutex<HashMap<String, CancelFlags>>,
) -> Result<MutexGuard<'_, HashMap<String, CancelFlags>>> {
    flags
        .lock()
        .map_err(|_| AppError::General("Work runtime cancellation lock failed".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Operation-kind labels remain owned by the processing lifecycle
        // utility; this test pins the work-runtime status vocabulary here.
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
}
