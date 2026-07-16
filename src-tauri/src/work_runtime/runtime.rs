use super::snapshot::{new_metadata_save_snapshot, new_processing_snapshot};
use super::state::WorkRuntimeState;
use super::types::{
    OperationId, OperationListSnapshot, OperationSnapshot, SubmitProcessingOperationRequest,
    WorkOperationListSnapshotEvent, WorkOperationSnapshotEvent, WorkOperationStatus,
    WorkSubmissionAccepted,
};
use crate::errors::{AppError, Result};
use crate::processing::context::processing::ProgressEventListener;
use crate::processing::run::{
    preflight_payload, process_payload_with_options, ProcessingRunOptions,
};
use crate::processing::JobType;
use crate::processing::{OperationResultSummary, ProcessResultStatus, ProgressEvent};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::Emitter;

#[derive(Clone)]
pub struct WorkRuntime {
    inner: Arc<WorkRuntimeInner>,
}

struct WorkRuntimeInner {
    state: Mutex<WorkRuntimeState>,
    operation_cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    sequence: AtomicU64,
}

impl Default for WorkRuntime {
    fn default() -> Self {
        Self {
            inner: Arc::new(WorkRuntimeInner {
                state: Mutex::new(WorkRuntimeState::default()),
                operation_cancel_flags: Mutex::new(HashMap::new()),
                sequence: AtomicU64::new(1),
            }),
        }
    }
}

impl WorkRuntime {
    pub async fn submit_processing_operation(
        &self,
        window: tauri::Window,
        registry: crate::ManagedJobRegistry,
        workspace_root: PathBuf,
        request: SubmitProcessingOperationRequest,
    ) -> Result<WorkSubmissionAccepted> {
        preflight_payload(
            request.payload.clone(),
            request.metadata.clone(),
            request.preview_seconds,
        )?;

        let operation_id = OperationId::new();
        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let kind = request.payload.job_type.unwrap_or(JobType::Batch).into();
        let title = request
            .title
            .clone()
            .unwrap_or_else(|| processing_operation_title(kind, request.payload.input_files.len()));
        let input_ids = request.payload.input_ids.as_deref();
        let snapshot = new_processing_snapshot(
            operation_id.clone(),
            sequence,
            kind,
            title,
            &request.payload.input_files,
            input_ids,
            now_ms(),
        );
        let cancel_flag = Arc::new(AtomicBool::new(false));

        {
            let mut state = lock_state(&self.inner.state)?;
            state.insert_operation(snapshot.clone());
        }
        {
            let mut flags = lock_cancel_flags(&self.inner.operation_cancel_flags)?;
            flags.insert(operation_id.0.clone(), cancel_flag.clone());
        }

        log_work_operation(WorkOperationLogEvent::Accepted, &snapshot);
        self.emit_snapshot(&window, &snapshot);
        self.emit_list(&window);

        let runtime = self.clone();
        let operation_id_for_task = operation_id.clone();
        let progress_runtime = runtime.clone();
        let progress_operation_id = operation_id_for_task.clone();
        let progress_window = window.clone();
        let progress_listener: Option<ProgressEventListener> =
            Some(std::sync::Arc::new(move |event: &ProgressEvent| {
                progress_runtime.apply_progress_and_emit(
                    &progress_window,
                    &progress_operation_id,
                    event,
                );
            }));
        tokio::spawn(async move {
            runtime.mark_running_and_emit(&window, &operation_id_for_task);
            let result = process_payload_with_options(
                window.clone(),
                registry,
                workspace_root,
                request.payload,
                request.metadata,
                request.preview_seconds,
                ProcessingRunOptions {
                    operation_id: Some(operation_id_for_task.to_string()),
                    operation_cancel: Some(cancel_flag),
                    progress_listener,
                },
            )
            .await;
            runtime.finish_processing_and_emit(&window, &operation_id_for_task, result);
            runtime.remove_cancel_flag(&operation_id_for_task);
        });

        Ok(WorkSubmissionAccepted {
            operation_id,
            snapshot,
        })
    }

    /// Begin a command-driven inline metadata-save operation: register the
    /// snapshot + operation cancel flag, emit it, and mark it running. The
    /// command then drives the save loop and terminalizes via
    /// `record_metadata_save_progress` + `finish_metadata_save_operation`.
    /// Returns the operation id and its cancel flag (wire into the registry
    /// cancellation checker so `cancel_work_operation` reaches the save loop).
    pub fn begin_metadata_save_operation(
        &self,
        window: &tauri::Window,
        input_files: &[String],
    ) -> Result<(OperationId, Arc<AtomicBool>)> {
        let operation_id = OperationId::new();
        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let title = processing_operation_title(
            crate::processing::OperationKind::MetadataSave,
            input_files.len(),
        );
        let snapshot = new_metadata_save_snapshot(
            operation_id.clone(),
            sequence,
            title,
            input_files,
            now_ms(),
        );
        let cancel_flag = Arc::new(AtomicBool::new(false));

        {
            let mut state = lock_state(&self.inner.state)?;
            state.insert_operation(snapshot.clone());
        }
        {
            let mut flags = lock_cancel_flags(&self.inner.operation_cancel_flags)?;
            flags.insert(operation_id.0.clone(), cancel_flag.clone());
        }

        log_work_operation(WorkOperationLogEvent::Accepted, &snapshot);
        self.emit_snapshot(window, &snapshot);
        self.emit_list(window);
        self.mark_running_and_emit(window, &operation_id);

        Ok((operation_id, cancel_flag))
    }

    /// Apply a metadata-save progress event to its operation (Work Center
    /// renders the live snapshot). Events are piped by `input_index`.
    pub fn record_metadata_save_progress(
        &self,
        window: &tauri::Window,
        operation_id: &OperationId,
        event: &ProgressEvent,
    ) {
        self.apply_progress_and_emit(window, operation_id, event);
    }

    /// Terminalize a metadata-save operation from its result summary plus
    /// per-child terminal facts, then emit and drop its cancel flag.
    pub fn finish_metadata_save_operation(
        &self,
        window: &tauri::Window,
        operation_id: &OperationId,
        summary: &OperationResultSummary,
        child_terminals: &[(usize, ProcessResultStatus, String)],
    ) -> Result<OperationSnapshot> {
        let snapshot = {
            let mut state = lock_state(&self.inner.state)?;
            state.complete_from_summary(operation_id, summary, child_terminals, now_ms())?
        };
        log_work_operation(WorkOperationLogEvent::Terminal, &snapshot);
        self.emit_snapshot(window, &snapshot);
        self.emit_list(window);
        self.remove_cancel_flag(operation_id);
        Ok(snapshot)
    }

    /// Fail a metadata-save operation when its run aborts before producing a
    /// result (e.g. an infrastructure error), then emit and drop its cancel flag.
    pub fn fail_metadata_save_operation(
        &self,
        window: &tauri::Window,
        operation_id: &OperationId,
        message: String,
    ) -> Result<OperationSnapshot> {
        let snapshot = {
            let mut state = lock_state(&self.inner.state)?;
            state.fail(operation_id, message, now_ms())?
        };
        log_work_operation(WorkOperationLogEvent::Terminal, &snapshot);
        self.emit_snapshot(window, &snapshot);
        self.emit_list(window);
        self.remove_cancel_flag(operation_id);
        Ok(snapshot)
    }

    /// Cancel-terminalize a metadata-save operation that aborted via cancellation
    /// (e.g. the operation cancel flag flipped during the permit wait, before the
    /// save loop). Mirrors the processing path's `AppError::Cancellation =>
    /// state.cancel` so a cancelled metadata save resolves to `Cancelled`, not
    /// `Failed`.
    pub fn cancel_metadata_save_operation(
        &self,
        window: &tauri::Window,
        operation_id: &OperationId,
        message: String,
    ) -> Result<OperationSnapshot> {
        let snapshot = {
            let mut state = lock_state(&self.inner.state)?;
            state.cancel(operation_id, message, now_ms())?
        };
        log_work_operation(WorkOperationLogEvent::Terminal, &snapshot);
        self.emit_snapshot(window, &snapshot);
        self.emit_list(window);
        self.remove_cancel_flag(operation_id);
        Ok(snapshot)
    }

    fn apply_progress_and_emit(
        &self,
        window: &tauri::Window,
        operation_id: &OperationId,
        event: &ProgressEvent,
    ) {
        match lock_state(&self.inner.state)
            .and_then(|mut state| state.apply_progress_event(operation_id, event, now_ms()))
        {
            Ok(snapshot) => self.emit_snapshot(window, &snapshot),
            Err(error) => log::warn!(
                "Failed to apply progress event for operation {}: {}",
                operation_id,
                error
            ),
        }
    }

    pub fn list_operations(&self) -> Result<OperationListSnapshot> {
        Ok(lock_state(&self.inner.state)?.list())
    }

    pub fn get_operation(&self, operation_id: OperationId) -> Result<OperationSnapshot> {
        lock_state(&self.inner.state)?.get(&operation_id)
    }

    pub fn cancel_operation(
        &self,
        window: &tauri::Window,
        operation_id: OperationId,
    ) -> Result<OperationSnapshot> {
        if let Some(flag) = lock_cancel_flags(&self.inner.operation_cancel_flags)?
            .get(operation_id.as_str())
            .cloned()
        {
            flag.store(true, Ordering::Release);
        }
        let snapshot = lock_state(&self.inner.state)?.request_cancel(&operation_id, now_ms())?;
        if snapshot.status == WorkOperationStatus::Cancelling {
            log_work_operation(WorkOperationLogEvent::CancelRequested, &snapshot);
        }
        self.emit_snapshot(window, &snapshot);
        self.emit_list(window);
        Ok(snapshot)
    }

    fn mark_running_and_emit(&self, window: &tauri::Window, operation_id: &OperationId) {
        match lock_state(&self.inner.state)
            .and_then(|mut state| state.mark_running(operation_id, now_ms()))
        {
            Ok(snapshot) => {
                if snapshot.status == WorkOperationStatus::Running {
                    log_work_operation(WorkOperationLogEvent::Running, &snapshot);
                }
                self.emit_snapshot(window, &snapshot);
                self.emit_list(window);
            }
            Err(error) => log::warn!("Failed to mark work operation running: {}", error),
        }
    }

    fn finish_processing_and_emit(
        &self,
        window: &tauri::Window,
        operation_id: &OperationId,
        result: Result<crate::processing::ProcessCommandResult>,
    ) {
        let snapshot_result = match result {
            Ok(result) => lock_state(&self.inner.state).and_then(|mut state| {
                state.complete_from_process_result(operation_id, &result, now_ms())
            }),
            Err(AppError::Cancellation(message)) => lock_state(&self.inner.state)
                .and_then(|mut state| state.cancel(operation_id, message, now_ms())),
            Err(error) => {
                let message = error.to_string();
                lock_state(&self.inner.state)
                    .and_then(|mut state| state.fail(operation_id, message, now_ms()))
            }
        };

        match snapshot_result {
            Ok(snapshot) => {
                log_work_operation(WorkOperationLogEvent::Terminal, &snapshot);
                self.emit_snapshot(window, &snapshot);
                self.emit_list(window);
            }
            Err(error) => log::warn!("Failed to terminalize work operation: {}", error),
        }
    }

    fn remove_cancel_flag(&self, operation_id: &OperationId) {
        if let Ok(mut flags) = lock_cancel_flags(&self.inner.operation_cancel_flags) {
            flags.remove(operation_id.as_str());
        }
    }

    fn emit_snapshot(&self, window: &tauri::Window, snapshot: &OperationSnapshot) {
        let event = WorkOperationSnapshotEvent {
            snapshot: snapshot.clone(),
        };
        if let Err(error) = window.emit(super::WORK_OPERATION_SNAPSHOT_EVENT_NAME, event) {
            log::warn!("Failed to emit work operation snapshot: {}", error);
        }
    }

    fn emit_list(&self, window: &tauri::Window) {
        match self.list_operations() {
            Ok(list) => {
                let event = WorkOperationListSnapshotEvent {
                    operations: list.operations,
                };
                if let Err(error) =
                    window.emit(super::WORK_OPERATION_LIST_SNAPSHOT_EVENT_NAME, event)
                {
                    log::warn!("Failed to emit work operation list snapshot: {}", error);
                }
            }
            Err(error) => log::warn!("Failed to build work operation list snapshot: {}", error),
        }
    }
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

fn processing_operation_title(kind: crate::processing::OperationKind, count: usize) -> String {
    match kind {
        crate::processing::OperationKind::ProcessingMerge => {
            format!("Merge encode ({count} file{})", plural_suffix(count))
        }
        crate::processing::OperationKind::ProcessingBatch => {
            format!("Batch encode ({count} file{})", plural_suffix(count))
        }
        crate::processing::OperationKind::RemoteAcquisition => {
            format!("Remote acquisition ({count} title{})", plural_suffix(count))
        }
        crate::processing::OperationKind::MetadataSave => {
            format!("Metadata save ({count} file{})", plural_suffix(count))
        }
    }
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

fn now_ms() -> i64 {
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

fn lock_cancel_flags(
    flags: &Mutex<HashMap<String, Arc<AtomicBool>>>,
) -> Result<MutexGuard<'_, HashMap<String, Arc<AtomicBool>>>> {
    flags
        .lock()
        .map_err(|_| AppError::General("Work runtime cancellation lock failed".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::OperationKind;

    #[test]
    fn work_operation_record_format_is_stable_and_path_free() {
        let mut snapshot = new_processing_snapshot(
            OperationId("operation-123".to_string()),
            1,
            OperationKind::ProcessingBatch,
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
        // Operation-kind labels are pinned by the owning processing contract
        // test (`processing_contract_operation_kind_log_labels_are_stable`).
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
