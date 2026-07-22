use super::types::{
    ChildJobStatus, OperationId, OperationListSnapshot, OperationLogEntry, OperationSnapshot,
    OperationTerminalSummary, WorkOperationStatus, WorkProgressStage, OPERATION_LOG_TAIL_CAP,
};
use crate::errors::{AppError, Result};
use crate::processing::ProgressEvent;
use crate::processing::{
    EventStage, OperationResultSummary, ProcessCommandResult, ProcessResultStatus,
};
use std::collections::BTreeMap;

use super::terminal::{
    child_status_from_result_status, is_terminal, operation_terminal_summary,
    stage_from_child_status, stage_from_status, terminal_summary_from_summary,
    work_status_from_summary,
};

const TERMINAL_CHILD_STATUSES: [ChildJobStatus; 4] = [
    ChildJobStatus::Completed,
    ChildJobStatus::Skipped,
    ChildJobStatus::Cancelled,
    ChildJobStatus::Failed,
];

/// Appends to the operation's bounded activity tail. Consecutive entries that
/// are identical in message, stage, AND child collapse (progress spam must not
/// rotate real history out, but distinct children or stage transitions with
/// the same wording are real history); the tail drops oldest past
/// `OPERATION_LOG_TAIL_CAP`.
fn push_operation_log(
    snapshot: &mut OperationSnapshot,
    timestamp_ms: i64,
    message: &str,
    stage: Option<WorkProgressStage>,
    child_job_id: Option<String>,
) {
    if message.is_empty() {
        return;
    }
    if snapshot.log_tail.last().is_some_and(|entry| {
        entry.message == message && entry.stage == stage && entry.child_job_id == child_job_id
    }) {
        return;
    }
    snapshot.log_tail.push(OperationLogEntry {
        timestamp_ms,
        message: message.to_string(),
        stage,
        child_job_id,
    });
    if snapshot.log_tail.len() > OPERATION_LOG_TAIL_CAP {
        let overflow = snapshot.log_tail.len() - OPERATION_LOG_TAIL_CAP;
        snapshot.log_tail.drain(0..overflow);
    }
}

/// Cap on retained terminal operations (running/accepted operations are never
/// pruned). Keeps unbounded history from growing forever while every
/// currently-active operation stays visible. The frontend Work Center
/// tombstone (`PURGED_OPERATION_TOMBSTONE_CAP` in
/// `src/ui/workCenter/state.svelte.ts`) must stay larger than this cap so an
/// operation pruned here can never be re-delivered after its frontend
/// dedupe entry has been evicted.
const TERMINAL_OPERATIONS_CAP: usize = 20;

#[derive(Default)]
pub(crate) struct WorkRuntimeState {
    operations: BTreeMap<String, OperationSnapshot>,
}

impl WorkRuntimeState {
    pub(crate) fn insert_operation(&mut self, snapshot: OperationSnapshot) {
        self.operations
            .insert(snapshot.operation_id.0.clone(), snapshot);
    }

    pub(crate) fn list(&self) -> OperationListSnapshot {
        let mut operations = self.operations.values().cloned().collect::<Vec<_>>();
        operations.sort_by_key(|operation| std::cmp::Reverse(operation.sequence));
        OperationListSnapshot { operations }
    }

    pub(crate) fn get(&self, operation_id: &OperationId) -> Result<OperationSnapshot> {
        self.operations
            .get(operation_id.as_str())
            .cloned()
            .ok_or_else(|| AppError::InvalidInput("Work operation was not found.".to_string()))
    }

    pub(crate) fn mark_running(
        &mut self,
        operation_id: &OperationId,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        if !is_terminal(snapshot.status) {
            snapshot.started_at_ms.get_or_insert(now_ms);
            if snapshot.cancel_requested || snapshot.status == WorkOperationStatus::Cancelling {
                return Ok(snapshot.clone());
            }
            snapshot.status = WorkOperationStatus::Running;
            snapshot.progress.stage = WorkProgressStage::Analyzing;
            snapshot.progress.message = "Processing started.".to_string();
            push_operation_log(
                snapshot,
                now_ms,
                "Processing started.",
                Some(WorkProgressStage::Analyzing),
                None,
            );
            for child in &mut snapshot.children {
                if child.status == ChildJobStatus::Queued {
                    child.cancellable = true;
                }
            }
        }
        Ok(snapshot.clone())
    }

    pub(crate) fn apply_progress_event(
        &mut self,
        operation_id: &OperationId,
        event: &ProgressEvent,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        if is_terminal(snapshot.status) {
            return Ok(snapshot.clone());
        }

        let child_count = snapshot.children.len();
        let mut matched_child = false;
        let mut child_scoped_batch_event = false;
        let mut matched_child_job_id: Option<String> = None;

        for child in &mut snapshot.children {
            if !progress_matches_child(child, event, child_count) {
                continue;
            }
            matched_child = true;
            child_scoped_batch_event = child_count > 1;
            matched_child_job_id = Some(child.child_job_id.clone());

            let child_status = child_status_from_event_stage(event.stage);
            if let Some(job_id) = event.job_id.as_deref() {
                child.job_id = Some(job_id.to_string());
            }
            child.status = child_status;
            child.progress.stage = work_stage_from_event_stage(event.stage);
            child.progress.percentage = event.percentage.clamp(0.0, 100.0);
            child.progress.message = event.message.clone();
            child.progress.eta_seconds = event.eta_seconds;
            child.cancellable =
                matches!(child_status, ChildJobStatus::Running) && event.job_id.is_some();
            child.message = Some(event.message.clone());
        }

        if !matched_child {
            return Ok(snapshot.clone());
        }

        if snapshot.cancel_requested || snapshot.status == WorkOperationStatus::Cancelling {
            return Ok(snapshot.clone());
        }

        snapshot.progress.stage = if child_scoped_batch_event {
            in_flight_stage(snapshot.progress.stage)
        } else {
            work_stage_from_event_stage(event.stage)
        };
        snapshot.progress.percentage = if child_scoped_batch_event {
            aggregate_child_progress(&snapshot.children)
        } else {
            event.percentage.clamp(0.0, 100.0)
        };
        snapshot.progress.message = event.message.clone();
        // A multi-child batch event carries one child's ETA, which does not
        // describe the aggregated operation percentage rendered beside it —
        // suppress rather than present a wrong number.
        snapshot.progress.eta_seconds = if child_scoped_batch_event {
            None
        } else {
            event.eta_seconds
        };
        snapshot.status = operation_status_from_event_stage(
            event.stage,
            snapshot.status,
            child_scoped_batch_event,
        );
        push_operation_log(
            snapshot,
            now_ms,
            &event.message,
            Some(work_stage_from_event_stage(event.stage)),
            matched_child_job_id,
        );

        Ok(snapshot.clone())
    }

    pub(crate) fn request_cancel(
        &mut self,
        operation_id: &OperationId,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        if !is_terminal(snapshot.status) {
            snapshot.status = WorkOperationStatus::Cancelling;
            snapshot.cancel_requested = true;
            snapshot.cancellable = false;
            snapshot.progress.message = "Cancellation requested.".to_string();
            snapshot.progress.stage = WorkProgressStage::Cleaning;
            push_operation_log(
                snapshot,
                now_ms,
                "Cancellation requested.",
                Some(WorkProgressStage::Cleaning),
                None,
            );
            for child in &mut snapshot.children {
                child.cancel_requested = true;
                child.cancellable = false;
            }
        } else {
            snapshot.finished_at_ms.get_or_insert(now_ms);
        }
        Ok(snapshot.clone())
    }

    pub(crate) fn complete_from_process_result(
        &mut self,
        operation_id: &OperationId,
        result: &ProcessCommandResult,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        apply_operation_terminal(
            snapshot,
            work_status_from_summary(&result.summary),
            terminal_summary_from_summary(&result.summary),
            now_ms,
        );

        for entry in &result.results {
            let target_index = entry.input_index.unwrap_or(0);
            if let Some(child) = snapshot
                .children
                .iter_mut()
                .find(|child| child.input_index.unwrap_or(0) == target_index)
            {
                child.status = child_status_from_result_status(entry.status);
                child.progress.stage = stage_from_child_status(child.status);
                child.progress.percentage = 100.0;
                child.progress.message = entry.message.clone();
                child.job_id = entry.job_id.clone();
                child.cancellable = false;
                child.message = Some(entry.message.clone());
            }
        }

        let snapshot = snapshot.clone();
        self.prune_terminal_operations();
        Ok(snapshot)
    }

    /// Terminalize a command-driven inline operation (metadata save) from its
    /// raw result summary plus per-child terminal facts. Operation status maps
    /// through the same canonical classifier as the processing path; children
    /// match by `input_index` (inline operations carry no per-child `job_id`).
    pub(crate) fn complete_from_summary(
        &mut self,
        operation_id: &OperationId,
        summary: &OperationResultSummary,
        child_terminals: &[(usize, ProcessResultStatus, String)],
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        apply_operation_terminal(
            snapshot,
            work_status_from_summary(summary),
            terminal_summary_from_summary(summary),
            now_ms,
        );

        for (input_index, status, message) in child_terminals {
            if let Some(child) = snapshot
                .children
                .iter_mut()
                .find(|child| child.input_index == Some(*input_index))
            {
                child.status = child_status_from_result_status(*status);
                child.progress.stage = stage_from_child_status(child.status);
                child.progress.percentage = 100.0;
                child.progress.message = message.clone();
                child.cancellable = false;
                child.cancel_requested = false;
                child.message = Some(message.clone());
            }
        }

        let snapshot = snapshot.clone();
        self.prune_terminal_operations();
        Ok(snapshot)
    }

    pub(crate) fn fail(
        &mut self,
        operation_id: &OperationId,
        message: String,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        snapshot.status = WorkOperationStatus::Failed;
        snapshot.finished_at_ms = Some(now_ms);
        snapshot.cancellable = false;
        snapshot.progress.stage = WorkProgressStage::Failed;
        snapshot.progress.percentage = 100.0;
        snapshot.progress.message = message.clone();
        push_operation_log(
            snapshot,
            now_ms,
            &message,
            Some(WorkProgressStage::Failed),
            None,
        );
        snapshot.errors.push(message.clone());
        let summary = OperationResultSummary::all_failed(snapshot.children.len());
        snapshot.terminal_summary = Some(operation_terminal_summary(&summary, message));
        for child in &mut snapshot.children {
            if !matches!(
                child.status,
                ChildJobStatus::Completed | ChildJobStatus::Skipped | ChildJobStatus::Cancelled
            ) {
                child.status = ChildJobStatus::Failed;
                child.cancellable = false;
            }
        }
        let snapshot = snapshot.clone();
        self.prune_terminal_operations();
        Ok(snapshot)
    }

    pub(crate) fn cancel(
        &mut self,
        operation_id: &OperationId,
        message: String,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        snapshot.status = WorkOperationStatus::Cancelled;
        snapshot.finished_at_ms = Some(now_ms);
        snapshot.cancel_requested = true;
        snapshot.cancellable = false;
        snapshot.progress.stage = WorkProgressStage::Cancelled;
        snapshot.progress.percentage = 100.0;
        snapshot.progress.message = message.clone();
        push_operation_log(
            snapshot,
            now_ms,
            &message,
            Some(WorkProgressStage::Cancelled),
            None,
        );
        let summary = OperationResultSummary::all_cancelled(snapshot.children.len());
        snapshot.terminal_summary = Some(operation_terminal_summary(&summary, message));
        for child in &mut snapshot.children {
            if !matches!(
                child.status,
                ChildJobStatus::Completed | ChildJobStatus::Skipped
            ) {
                child.status = ChildJobStatus::Cancelled;
                child.cancellable = false;
                child.cancel_requested = true;
            }
        }
        let snapshot = snapshot.clone();
        self.prune_terminal_operations();
        Ok(snapshot)
    }

    fn snapshot_mut(&mut self, operation_id: &OperationId) -> Result<&mut OperationSnapshot> {
        self.operations
            .get_mut(operation_id.as_str())
            .ok_or_else(|| AppError::InvalidInput("Work operation was not found.".to_string()))
    }

    /// Bound retained terminal-operation history: once more than
    /// `TERMINAL_OPERATIONS_CAP` operations are terminal, drop the oldest
    /// (lowest sequence) beyond the cap. Running/accepted operations are
    /// never inspected here and are never pruned.
    fn prune_terminal_operations(&mut self) {
        let mut terminal_ids: Vec<(u64, String)> = self
            .operations
            .iter()
            .filter(|(_, snapshot)| is_terminal(snapshot.status))
            .map(|(id, snapshot)| (snapshot.sequence, id.clone()))
            .collect();
        if terminal_ids.len() <= TERMINAL_OPERATIONS_CAP {
            return;
        }
        terminal_ids.sort_by_key(|(sequence, _)| *sequence);
        let excess = terminal_ids.len() - TERMINAL_OPERATIONS_CAP;
        for (_, id) in terminal_ids.into_iter().take(excess) {
            self.operations.remove(&id);
        }
    }
}

/// Shared operation-level terminalization for both the processing path
/// (`complete_from_process_result`) and command-driven inline operations
/// (`complete_from_summary`). Child terminalization stays caller-specific.
fn apply_operation_terminal(
    snapshot: &mut OperationSnapshot,
    status: WorkOperationStatus,
    terminal_summary: OperationTerminalSummary,
    now_ms: i64,
) {
    snapshot.status = status;
    snapshot.finished_at_ms = Some(now_ms);
    snapshot.cancellable = false;
    snapshot.progress.stage = stage_from_status(status);
    snapshot.progress.percentage = 100.0;
    snapshot.progress.message = terminal_summary.message.clone();
    push_operation_log(
        snapshot,
        now_ms,
        &terminal_summary.message,
        Some(stage_from_status(status)),
        None,
    );
    snapshot.terminal_summary = Some(terminal_summary);
}

fn progress_matches_child(
    child: &super::types::ChildJobSnapshot,
    event: &ProgressEvent,
    child_count: usize,
) -> bool {
    if let Some(job_id) = event.job_id.as_deref() {
        if let Some(existing_job_id) = child.job_id.as_deref() {
            return Some(job_id) == Some(existing_job_id);
        }
    }

    if let Some(input_index) = event.input_index {
        return child.input_index == Some(input_index);
    }

    child_count == 1
}

fn aggregate_child_progress(children: &[super::types::ChildJobSnapshot]) -> f32 {
    if children.is_empty() {
        return 0.0;
    }

    let total = children
        .iter()
        .map(|child| {
            if TERMINAL_CHILD_STATUSES.contains(&child.status) {
                100.0
            } else {
                child.progress.percentage.clamp(0.0, 100.0)
            }
        })
        .sum::<f32>();

    total / children.len() as f32
}

fn in_flight_stage(current: WorkProgressStage) -> WorkProgressStage {
    if matches!(
        current,
        WorkProgressStage::Pending | WorkProgressStage::Analyzing
    ) {
        WorkProgressStage::Converting
    } else {
        current
    }
}

fn child_status_from_event_stage(stage: EventStage) -> ChildJobStatus {
    match stage {
        EventStage::Completed => ChildJobStatus::Completed,
        EventStage::Skipped => ChildJobStatus::Skipped,
        EventStage::Failed => ChildJobStatus::Failed,
        EventStage::Cancelled => ChildJobStatus::Cancelled,
        _ => ChildJobStatus::Running,
    }
}

fn work_stage_from_event_stage(stage: EventStage) -> WorkProgressStage {
    match stage {
        EventStage::Analyzing => WorkProgressStage::Analyzing,
        EventStage::Converting => WorkProgressStage::Converting,
        EventStage::Writing => WorkProgressStage::Writing,
        EventStage::Completed | EventStage::Skipped => WorkProgressStage::Complete,
        EventStage::Failed => WorkProgressStage::Failed,
        EventStage::Cancelled => WorkProgressStage::Cancelled,
    }
}

fn operation_status_from_event_stage(
    stage: EventStage,
    current: WorkOperationStatus,
    child_scoped_batch_event: bool,
) -> WorkOperationStatus {
    if is_terminal(current) {
        return current;
    }

    if child_scoped_batch_event {
        return WorkOperationStatus::Running;
    }

    match stage {
        EventStage::Failed => WorkOperationStatus::Failed,
        EventStage::Cancelled => WorkOperationStatus::Cancelled,
        _ => WorkOperationStatus::Running,
    }
}
