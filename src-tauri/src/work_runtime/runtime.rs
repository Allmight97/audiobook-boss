use super::snapshot::new_processing_snapshot;
use super::state::WorkRuntimeState;
use super::types::{
    OperationId, OperationListSnapshot, OperationSnapshot, SubmitProcessingOperationRequest,
    WorkOperationListSnapshotEvent, WorkOperationSnapshotEvent, WorkSubmissionAccepted,
};
use crate::errors::{AppError, Result};
use crate::processing::context::processing::ProgressEventListener;
use crate::processing::run::{
    preflight_payload, process_payload_with_options, ProcessingRunOptions,
};
use crate::processing::JobType;
use crate::processing::ProgressEvent;
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
                    operation_id: Some(operation_id_for_task.0.clone()),
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

    fn apply_progress_and_emit(
        &self,
        window: &tauri::Window,
        operation_id: &OperationId,
        event: &ProgressEvent,
    ) {
        match lock_state(&self.inner.state)
            .and_then(|mut state| state.apply_progress_event(operation_id, event))
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
        self.emit_snapshot(window, &snapshot);
        self.emit_list(window);
        Ok(snapshot)
    }

    fn mark_running_and_emit(&self, window: &tauri::Window, operation_id: &OperationId) {
        match lock_state(&self.inner.state)
            .and_then(|mut state| state.mark_running(operation_id, now_ms()))
        {
            Ok(snapshot) => {
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
