use super::operations::{CancelTokens, Live, Operations};
use super::snapshot::{new_metadata_save_snapshot, new_processing_snapshot};
use super::state::WorkRuntimeState;
use super::types::{
    OperationId, OperationSnapshot, SubmitProcessingOperationRequest, WorkOperationStatus,
    WorkSubmissionAccepted,
};
use crate::errors::{AppError, Result};
use crate::host::{EngineEvent, Host};
use crate::processing::run::{
    preflight_title_outputs, process_inspected_with_options, ExportTitle, ProcessingRun, RunMode,
};
use crate::processing::ProgressEventListener;
use crate::processing::TitleOutput;
use crate::processing::{OperationResultSummary, ProcessResultStatus, ProgressEvent};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct WorkRuntime {
    inner: Arc<WorkRuntimeInner>,
}

pub(crate) type OnFinished = Box<dyn FnOnce(OperationSnapshot) + Send>;

struct WorkRuntimeInner {
    operations: Mutex<Operations>,
    tasks: crate::engine_tasks::EngineTasks,
    changes: tokio::sync::watch::Sender<u64>,
    sequence: AtomicU64,
}

impl WorkRuntime {
    pub(crate) fn new(tasks: crate::engine_tasks::EngineTasks) -> Self {
        Self {
            inner: Arc::new(WorkRuntimeInner {
                tasks,
                operations: Mutex::new(Operations::new()),
                changes: tokio::sync::watch::channel(0).0,
                sequence: AtomicU64::new(1),
            }),
        }
    }
}

impl WorkRuntime {
    pub(crate) fn follow_concurrency_lock(
        &self,
        host: &Host,
        registry: &crate::ManagedJobRegistry,
    ) {
        let runtime = Arc::downgrade(&self.inner);
        let observed = Arc::downgrade(registry);
        let host = host.clone();
        registry.observe_concurrency_lock(Arc::new(move || {
            if let (Some(inner), Some(registry)) = (runtime.upgrade(), observed.upgrade()) {
                WorkRuntime { inner }.publish_concurrency_lock(&host, &registry);
            }
        }));
        self.operations()
            .state
            .set_concurrency_locked(registry.concurrency_locked());
    }

    fn publish_concurrency_lock(&self, host: &Host, registry: &crate::ManagedJobRegistry) {
        let update = self
            .operations()
            .state
            .set_concurrency_locked(registry.concurrency_locked());
        if let Some(update) = update {
            host.emit(EngineEvent::WorkOperations(update));
        }
    }

    pub(crate) async fn submit_processing_operation(
        &self,
        host: Host,
        registry: crate::ManagedJobRegistry,
        workspace_root: PathBuf,
        request: SubmitProcessingOperationRequest,
        on_finished: Option<OnFinished>,
        inspected: crate::processing::plan::InspectedProcessingPlan,
    ) -> Result<WorkSubmissionAccepted> {
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
        );
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
        let cancel_tokens = CancelTokens::per_title(request.payload.input_files.len());
        let export_titles: Vec<ExportTitle> = cancel_tokens
            .titles()
            .into_iter()
            .zip(&titles)
            .map(|(cancel, output)| ExportTitle {
                cancel,
                output: Arc::clone(output),
            })
            .collect();
        let title_cancels = cancel_tokens.titles();
        let live = Live {
            titles: cancel_tokens,
            sources: title_source_paths(&request.payload),
        };
        let snapshot = self.operations().accept(snapshot, live);

        for (index, title) in titles.iter().enumerate() {
            title.on_change(self.output_listener(&host, &operation_id, index));
        }

        let runtime = self.clone();
        let operation_id_for_task = operation_id.clone();
        let progress_runtime = runtime.clone();
        let progress_operation_id = operation_id_for_task.clone();
        let progress_host = host.clone();
        let run_titles = titles.clone();
        let listener: ProgressEventListener = Arc::new(move |event: &ProgressEvent| {
            progress_runtime.apply_progress_and_emit(&progress_host, &progress_operation_id, event);
        });
        let active_run = Arc::new(active_run);
        #[expect(clippy::disallowed_methods, reason = "joined by end_export")]
        let work = tokio::task::spawn({
            let runtime = runtime.clone();
            let host = host.clone();
            let operation_id = operation_id_for_task.clone();
            let active_run = Arc::clone(&active_run);
            async move {
                runtime.mark_running_and_emit(&host, &operation_id);
                process_inspected_with_options(
                    &active_run,
                    host,
                    registry,
                    workspace_root,
                    request.payload,
                    inspected,
                    ProcessingRun {
                        operation_id: operation_id.to_string(),
                        listener,
                        mode: RunMode::Export(export_titles),
                    },
                )
                .await
            }
        });
        let end = ExportEnd {
            host,
            operation_id: operation_id_for_task,
            titles: run_titles,
            title_cancels,
            on_finished,
        };
        self.inner.tasks.spawn(async move {
            let _active_run = active_run;
            runtime.end_export(work, end).await;
        });

        Ok(WorkSubmissionAccepted {
            operation_id,
            snapshot,
            titles,
        })
    }

    async fn end_export(
        self,
        work: tokio::task::JoinHandle<Result<crate::processing::ProcessCommandResult>>,
        end: ExportEnd,
    ) {
        let result = work.await.unwrap_or_else(|stopped| {
            log::error!("export {} stopped: {stopped}", end.operation_id);
            for cancel in &end.title_cancels {
                cancel.cancel();
            }
            Err(AppError::General(
                "The export stopped on an internal error.".to_string(),
            ))
        });
        for title in &end.titles {
            title.end();
        }
        let finished = self.finish_processing_and_emit(&end.host, &end.operation_id, result);
        if let (Some(on_finished), Some(snapshot)) = (end.on_finished, finished) {
            on_finished(snapshot);
        }
    }

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
            let snapshot =
                runtime
                    .operations()
                    .state
                    .set_output_update(&operation_id, index, update);
            if let Some(snapshot) = snapshot {
                runtime.emit_snapshot(&host, &snapshot);
            }
        })
    }

    /// Cancels the title at `index` and waits until it has published or
    /// ended without an output, its own empty folders removed. Returns whether
    /// it published.
    pub(crate) async fn stop_title(
        &self,
        host: &Host,
        operation_id: &OperationId,
        index: usize,
        title: &TitleOutput,
    ) -> bool {
        let child = self.operations().state.child_job_id(operation_id, index);
        if let Some(child) = child {
            if let Err(error) = self.cancel_operation(host, operation_id.clone(), Some(child)) {
                log::warn!("Failed to cancel a title for restart: {error}");
            }
        }
        title.settled().await
    }

    /// Registers a metadata-save operation, emits it, and marks it running.
    /// Returns the operation id and the cancel token `cancel_operation` cancels
    /// for the save loop.
    pub(crate) fn begin_metadata_save_operation(
        &self,
        host: &Host,
        input_files: &[String],
    ) -> Result<(OperationId, CancellationToken)> {
        let operation_id = OperationId::new();
        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let count = input_files.len();
        let title = format!("Metadata save ({count} file{})", plural_suffix(count));
        let snapshot =
            new_metadata_save_snapshot(operation_id.clone(), sequence, title, input_files);
        let cancel_tokens = CancelTokens::per_title(1);
        let cancel_token = cancel_tokens.titles().remove(0);
        let live = Live {
            titles: cancel_tokens,
            sources: Vec::new(),
        };
        let snapshot = self.operations().accept(snapshot, live);

        log_work_operation(WorkOperationLogEvent::Accepted, &snapshot);
        self.emit_snapshot(host, &snapshot);
        self.mark_running_and_emit(host, &operation_id);

        Ok((operation_id, cancel_token))
    }

    /// Applies a metadata-save progress event to its operation; events match
    /// children by `input_index`.
    pub(crate) fn record_metadata_save_progress(
        &self,
        host: &Host,
        operation_id: &OperationId,
        event: &ProgressEvent,
    ) {
        self.apply_progress_and_emit(host, operation_id, event);
    }

    /// Terminalize a metadata-save operation from its run outcome, then emit
    /// and drop its cancel token. A finished run maps its summary through the
    /// canonical classifier and terminalizes children by `input_index`; an
    /// aborted run resolves to Cancelled for a cancellation and Failed
    /// otherwise, exactly as a processing run does.
    pub(crate) fn finish_metadata_save_operation(
        &self,
        host: &Host,
        operation_id: &OperationId,
        outcome: std::result::Result<InlineRunTerminal<'_>, &AppError>,
    ) -> Result<OperationSnapshot> {
        let snapshot = self
            .operations()
            .finish(operation_id, |state| match outcome {
                Ok(run) => {
                    state.complete_from_summary(operation_id, run.summary, run.children, now_ms())
                }
                Err(error) => terminalize_aborted_run(state, operation_id, error),
            })?;
        log_work_operation(WorkOperationLogEvent::Terminal, &snapshot);
        self.emit_snapshot(host, &snapshot);
        Ok(snapshot)
    }

    fn apply_progress_and_emit(
        &self,
        host: &Host,
        operation_id: &OperationId,
        event: &ProgressEvent,
    ) {
        let applied = self
            .operations()
            .state
            .apply_progress_event(operation_id, event, now_ms());
        match applied {
            Ok(snapshot) => self.emit_snapshot(host, &snapshot),
            Err(error) => log::warn!(
                "Failed to apply progress event for operation {}: {}",
                operation_id,
                error
            ),
        }
    }

    pub fn list_operations(&self) -> super::WorkOperationsSnapshot {
        self.operations().state.list()
    }

    /// Accepted exports that have not finished. Metadata Saves are left out:
    /// shutdown lets them finish rather than cancelling them.
    pub(crate) fn unfinished_exports(&self) -> Vec<OperationId> {
        self.operations()
            .state
            .list()
            .operations
            .into_iter()
            .filter(|operation| {
                operation.kind == crate::processing::OperationKind::ProcessingBatch
                    && !operation.is_finished()
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
        let snapshot = self.operations().cancel_all(operation_id, now_ms())?;
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
        let (snapshot, cancelled) = self.operations().cancel_title(operation_id, child_job_id)?;
        if cancelled {
            log::info!(
                // Not a `work_operation` lifecycle record: dev-log analysis validates
                // those events; the job's own terminal record carries the outcome.
                "title_cancel_requested operation_id={operation_id} child_job_id={child_job_id}"
            );
        }
        Ok(snapshot)
    }

    fn mark_running_and_emit(&self, host: &Host, operation_id: &OperationId) {
        let marked = self.operations().state.mark_running(operation_id, now_ms());
        match marked {
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
        let snapshot_result = self
            .operations()
            .finish(operation_id, |state| match &result {
                Ok(result) => state.complete_from_process_result(operation_id, result, now_ms()),
                Err(error) => terminalize_aborted_run(state, operation_id, error),
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

    pub(crate) fn sources_in_use(&self) -> HashSet<PathBuf> {
        self.operations().sources_in_use()
    }

    /// Every source of every export that has not finished, including titles
    /// already encoded: a title still copies its companion files after its
    /// audio completes.
    pub(crate) fn sources_held(&self) -> HashSet<PathBuf> {
        self.operations().sources_held()
    }

    pub(crate) fn subscribe_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.inner.changes.subscribe()
    }

    /// A panic under this lock ends only its own operation, as Failed; every
    /// later call keeps working on the state it left.
    fn operations(&self) -> MutexGuard<'_, Operations> {
        self.inner
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Publishes a change with the display order as of now. The revision is
    /// taken under the state lock, so hosts can order updates however they
    /// arrive.
    fn emit_snapshot(&self, host: &Host, snapshot: &OperationSnapshot) {
        self.inner.changes.send_modify(|change| *change += 1);
        let update = self.operations().state.publish(snapshot.clone());
        host.emit(EngineEvent::WorkOperations(update));
    }
}

struct ExportEnd {
    host: Host,
    operation_id: OperationId,
    titles: Vec<Arc<TitleOutput>>,
    title_cancels: Vec<CancellationToken>,
    on_finished: Option<OnFinished>,
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
        snapshot.kind.log_label(),
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

pub struct InlineRunTerminal<'a> {
    pub summary: &'a OperationResultSummary,
    pub children: &'a [(usize, ProcessResultStatus, String)],
}

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

pub(crate) fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
