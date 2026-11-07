use super::types::{
    ChildJobStatus, OperationId, OperationLogEntry, OperationPhase, OperationSnapshot,
    WorkOperationStatus, WorkOperationsSnapshot, WorkOperationsUpdate, WorkProgressStage,
    OPERATION_LOG_TAIL_CAP,
};
use crate::errors::{AppError, Result};
use crate::processing::ProgressEvent;
use crate::processing::{
    OperationKind, OperationResultSummary, ProcessCommandResult, ProcessResultStatus,
};
use std::collections::BTreeMap;

use super::stage_table::{for_event, CANCELLING_STAGE};
use super::terminal::{
    operation_terminal_summary, terminal_summary_from_summary, work_status_from_summary,
};
use super::terminalize::{apply_terminal_plan, ChildOutcome, TerminalPlan};

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
pub(super) fn push_operation_log(
    snapshot: &mut OperationSnapshot,
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
        message: message.to_string(),
        stage,
        child_job_id,
    });
    if snapshot.log_tail.len() > OPERATION_LOG_TAIL_CAP {
        let overflow = snapshot.log_tail.len() - OPERATION_LOG_TAIL_CAP;
        snapshot.log_tail.drain(0..overflow);
    }
}

const TERMINAL_OPERATIONS_CAP: usize = 20;

#[derive(Default)]
pub(crate) struct WorkRuntimeState {
    operations: BTreeMap<String, OperationSnapshot>,
    revision: u64,
    terminal_order: Vec<String>,
    concurrency_locked: bool,
}

impl WorkRuntimeState {
    pub(crate) fn insert_operation(
        &mut self,
        mut snapshot: OperationSnapshot,
    ) -> OperationSnapshot {
        snapshot.revision = 1;
        self.operations
            .insert(snapshot.operation_id.0.clone(), snapshot.clone());
        snapshot
    }

    pub(crate) fn list(&self) -> WorkOperationsSnapshot {
        let operations = self.ordered();
        WorkOperationsSnapshot {
            revision: self.revision,
            order: operations.iter().map(|o| o.operation_id.clone()).collect(),
            operations: operations.into_iter().cloned().collect(),
            concurrency_locked: self.concurrency_locked,
        }
    }

    pub(crate) fn publish(&mut self, changed: OperationSnapshot) -> WorkOperationsUpdate {
        self.update(Some(changed))
    }

    pub(crate) fn set_concurrency_locked(&mut self, locked: bool) -> Option<WorkOperationsUpdate> {
        if self.concurrency_locked == locked {
            return None;
        }
        self.concurrency_locked = locked;
        Some(self.update(None))
    }

    fn update(&mut self, changed: Option<OperationSnapshot>) -> WorkOperationsUpdate {
        self.revision += 1;
        WorkOperationsUpdate {
            revision: self.revision,
            order: self
                .ordered()
                .into_iter()
                .map(|o| o.operation_id.clone())
                .collect(),
            changed,
            concurrency_locked: self.concurrency_locked,
        }
    }

    fn ordered(&self) -> Vec<&OperationSnapshot> {
        let mut operations = self.operations.values().collect::<Vec<_>>();
        operations.sort_by_key(|operation| {
            let bucket = match operation.phase {
                OperationPhase::Active => 0,
                OperationPhase::Accepted => 1,
                OperationPhase::Finished => 2,
            };
            (bucket, std::cmp::Reverse(operation.sequence))
        });
        operations
    }

    pub(crate) fn operation(&self, operation_id: &str) -> Option<&OperationSnapshot> {
        self.operations.get(operation_id)
    }

    pub(crate) fn mark_running(
        &mut self,
        operation_id: &OperationId,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        if !snapshot.is_finished() {
            snapshot.started_at_ms.get_or_insert(now_ms);
            if snapshot.cancel_requested || snapshot.status == WorkOperationStatus::Cancelling {
                return Ok(snapshot.clone());
            }
            snapshot.set_status(WorkOperationStatus::Running);
            snapshot.progress.stage = WorkProgressStage::Analyzing;
            snapshot.progress.message = "Processing started.".to_string();
            push_operation_log(
                snapshot,
                "Processing started.",
                Some(WorkProgressStage::Analyzing),
                None,
            );
            let titles_cancellable = titles_cancel_individually(snapshot.kind);
            for child in &mut snapshot.children {
                if child.status == ChildJobStatus::Queued && !child.cancel_requested {
                    child.cancellable = titles_cancellable;
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
        if snapshot.is_finished() {
            return Ok(snapshot.clone());
        }

        let child_count = snapshot.children.len();
        let titles_cancellable = titles_cancel_individually(snapshot.kind);
        let mut matched_child = false;
        let mut child_scoped_batch_event = false;
        let mut matched_child_job_id: Option<String> = None;

        for child in &mut snapshot.children {
            if !progress_matches_child(child, event) {
                continue;
            }
            matched_child = true;
            child_scoped_batch_event = child_count > 1;
            matched_child_job_id = Some(child.child_job_id.clone());

            let row = for_event(event.stage);
            if let Some(job_id) = event.job_id.as_deref() {
                child.job_id = Some(job_id.to_string());
            }
            if row.child == ChildJobStatus::Running {
                child.started_at_ms.get_or_insert(now_ms);
            } else {
                child.finished_at_ms.get_or_insert(now_ms);
            }
            child.status = row.child;
            child.progress.stage = row.stage;
            child.progress.percentage = finite_percentage(event.percentage);
            child.progress.message = event.message.clone();
            child.progress.eta_seconds = event.eta_seconds;
            child.cancellable = titles_cancellable
                && row.child == ChildJobStatus::Running
                && event.job_id.is_some()
                && !child.cancel_requested;
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
            for_event(event.stage).stage
        };
        snapshot.progress.percentage = if child_scoped_batch_event {
            aggregate_child_progress(&snapshot.children)
        } else {
            finite_percentage(event.percentage)
        };
        snapshot.progress.message = event.message.clone();
        snapshot.progress.eta_seconds = if child_scoped_batch_event {
            None
        } else {
            event.eta_seconds
        };
        push_operation_log(
            snapshot,
            &event.message,
            Some(for_event(event.stage).stage),
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
        if !snapshot.is_finished() {
            snapshot.set_status(WorkOperationStatus::Cancelling);
            snapshot.cancel_requested = true;
            snapshot.cancellable = false;
            snapshot.progress.message = "Cancellation requested.".to_string();
            snapshot.progress.stage = CANCELLING_STAGE;
            push_operation_log(
                snapshot,
                "Cancellation requested.",
                Some(CANCELLING_STAGE),
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

    /// Requests cancellation of one title. Returns its input index when this
    /// call newly requested it; a repeated or already-finished request returns
    /// the snapshot unchanged.
    pub(crate) fn request_child_cancel(
        &mut self,
        operation_id: &OperationId,
        child_job_id: &str,
    ) -> Result<(OperationSnapshot, Option<usize>)> {
        let snapshot = self.snapshot_mut(operation_id)?;
        if !titles_cancel_individually(snapshot.kind) {
            return Err(AppError::InvalidInput(
                "Items in this operation can't be cancelled one at a time.".to_string(),
            ));
        }
        let operation_terminal = snapshot.is_finished();
        let child = snapshot
            .children
            .iter_mut()
            .find(|child| child.child_job_id == child_job_id)
            .ok_or_else(|| AppError::InvalidInput("Work item was not found.".to_string()))?;
        if operation_terminal
            || TERMINAL_CHILD_STATUSES.contains(&child.status)
            || child.cancel_requested
        {
            return Ok((snapshot.clone(), None));
        }
        child.cancel_requested = true;
        child.cancellable = false;
        child.progress.message = "Cancellation requested.".to_string();
        let title_index = child.input_index;
        let message = format!("Cancellation requested for {}.", child.label);
        push_operation_log(
            snapshot,
            &message,
            Some(CANCELLING_STAGE),
            Some(child_job_id.to_string()),
        );
        Ok((snapshot.clone(), title_index))
    }

    pub(crate) fn complete_from_process_result(
        &mut self,
        operation_id: &OperationId,
        result: &ProcessCommandResult,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        self.terminalize(operation_id, now_ms, |snapshot| TerminalPlan {
            status: work_status_from_summary(&result.summary),
            summary: terminal_summary_from_summary(&result.summary),
            cancel_requested: false,
            children: result
                .results
                .iter()
                .filter_map(|entry| {
                    child_position(snapshot, entry.input_index)
                        .map(|position| ChildOutcome::reported(position, entry))
                })
                .collect(),
        })
    }

    /// Terminalizes an inline operation (a metadata save) from its raw result
    /// summary and per-child terminal facts. Status maps through the same
    /// classifier as processing; children match by `input_index`.
    pub(crate) fn complete_from_summary(
        &mut self,
        operation_id: &OperationId,
        summary: &OperationResultSummary,
        child_terminals: &[(usize, ProcessResultStatus, String)],
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        self.terminalize(operation_id, now_ms, |snapshot| TerminalPlan {
            status: work_status_from_summary(summary),
            summary: terminal_summary_from_summary(summary),
            cancel_requested: false,
            children: child_terminals
                .iter()
                .filter_map(|(input_index, status, message)| {
                    child_position(snapshot, *input_index)
                        .map(|position| ChildOutcome::inline(position, *status, message.clone()))
                })
                .collect(),
        })
    }

    pub(crate) fn fail(
        &mut self,
        operation_id: &OperationId,
        message: String,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        self.terminalize(operation_id, now_ms, |snapshot| TerminalPlan {
            status: WorkOperationStatus::Failed,
            summary: operation_terminal_summary(
                &OperationResultSummary::all_failed(snapshot.children.len()),
                message.clone(),
            ),
            cancel_requested: false,
            children: unsettled_children(snapshot, |status| {
                !matches!(
                    status,
                    ChildJobStatus::Completed | ChildJobStatus::Skipped | ChildJobStatus::Cancelled
                )
            })
            .into_iter()
            .map(|position| {
                ChildOutcome::aborted(position, ProcessResultStatus::Failed, message.clone())
            })
            .collect(),
        })
    }

    pub(crate) fn cancel(
        &mut self,
        operation_id: &OperationId,
        message: String,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        self.terminalize(operation_id, now_ms, |snapshot| TerminalPlan {
            status: WorkOperationStatus::Cancelled,
            summary: operation_terminal_summary(
                &OperationResultSummary::all_cancelled(snapshot.children.len()),
                message.clone(),
            ),
            cancel_requested: true,
            children: unsettled_children(snapshot, |status| {
                !matches!(status, ChildJobStatus::Completed | ChildJobStatus::Skipped)
            })
            .into_iter()
            .map(|position| {
                ChildOutcome::aborted(position, ProcessResultStatus::Cancelled, message.clone())
            })
            .collect(),
        })
    }

    fn terminalize<'a>(
        &mut self,
        operation_id: &OperationId,
        now_ms: i64,
        plan: impl FnOnce(&OperationSnapshot) -> TerminalPlan<'a>,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id)?;
        let plan = plan(snapshot);
        apply_terminal_plan(snapshot, plan, now_ms);
        let snapshot = snapshot.clone();
        self.record_terminal_operation(operation_id.as_str());
        self.prune_terminal_operations();
        Ok(snapshot)
    }

    pub(crate) fn set_output_update(
        &mut self,
        operation_id: &OperationId,
        input_index: usize,
        update: Option<crate::processing::OutputUpdate>,
    ) -> Option<OperationSnapshot> {
        let snapshot = self.snapshot_mut(operation_id).ok()?;
        let child = snapshot
            .children
            .iter_mut()
            .find(|child| child.input_index == Some(input_index))?;
        child.output_update = update;
        Some(snapshot.clone())
    }

    pub(crate) fn child_job_id(
        &self,
        operation_id: &OperationId,
        input_index: usize,
    ) -> Option<String> {
        self.operations
            .get(operation_id.as_str())?
            .children
            .iter()
            .find(|child| child.input_index == Some(input_index))
            .map(|child| child.child_job_id.clone())
    }

    fn snapshot_mut(&mut self, operation_id: &OperationId) -> Result<&mut OperationSnapshot> {
        let snapshot = self
            .operations
            .get_mut(operation_id.as_str())
            .ok_or_else(|| AppError::InvalidInput("Work operation was not found.".to_string()))?;
        snapshot.revision += 1;
        Ok(snapshot)
    }

    fn record_terminal_operation(&mut self, operation_id: &str) {
        if !self.terminal_order.iter().any(|id| id == operation_id) {
            self.terminal_order.push(operation_id.to_string());
        }
    }

    fn prune_terminal_operations(&mut self) {
        while self.terminal_order.len() > TERMINAL_OPERATIONS_CAP {
            let oldest = self.terminal_order.remove(0);
            self.operations.remove(&oldest);
        }
    }
}

fn child_position(snapshot: &OperationSnapshot, input_index: usize) -> Option<usize> {
    snapshot
        .children
        .iter()
        .position(|child| child.input_index == Some(input_index))
}

fn unsettled_children(
    snapshot: &OperationSnapshot,
    takes: impl Fn(ChildJobStatus) -> bool,
) -> Vec<usize> {
    snapshot
        .children
        .iter()
        .enumerate()
        .filter(move |(_, child)| takes(child.status))
        .map(|(position, _)| position)
        .collect()
}

fn titles_cancel_individually(kind: OperationKind) -> bool {
    kind == OperationKind::ProcessingBatch
}

fn finite_percentage(percentage: f32) -> f32 {
    if percentage.is_finite() {
        percentage.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn progress_matches_child(child: &super::types::ChildJobSnapshot, event: &ProgressEvent) -> bool {
    if let Some(job_id) = event.job_id.as_deref() {
        if let Some(existing_job_id) = child.job_id.as_deref() {
            return Some(job_id) == Some(existing_job_id);
        }
    }

    event
        .input_index
        .is_some_and(|input_index| child.input_index == Some(input_index))
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
