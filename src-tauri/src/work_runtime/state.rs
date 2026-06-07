use super::types::{
    ChildJobStatus, OperationId, OperationListSnapshot, OperationSnapshot, WorkOperationStatus,
    WorkProgressStage,
};
use crate::errors::{AppError, Result};
use crate::processing::{OperationResultSummary, ProcessCommandResult};
use std::collections::BTreeMap;

use super::terminal::{
    child_status_from_result_status, is_terminal, operation_terminal_summary,
    stage_from_child_status, stage_from_status, terminal_summary_from_process_result,
    work_status_from_process_result,
};

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
            for child in &mut snapshot.children {
                if child.status == ChildJobStatus::Queued {
                    child.cancellable = true;
                }
            }
        }
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
        let summary = terminal_summary_from_process_result(result);
        snapshot.status = work_status_from_process_result(result);
        snapshot.finished_at_ms = Some(now_ms);
        snapshot.cancellable = false;
        snapshot.progress.stage = stage_from_status(snapshot.status);
        snapshot.progress.percentage = 100.0;
        snapshot.progress.message = summary.message.clone();
        snapshot.terminal_summary = Some(summary);

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

        Ok(snapshot.clone())
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
        Ok(snapshot.clone())
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
        Ok(snapshot.clone())
    }

    fn snapshot_mut(&mut self, operation_id: &OperationId) -> Result<&mut OperationSnapshot> {
        self.operations
            .get_mut(operation_id.as_str())
            .ok_or_else(|| AppError::InvalidInput("Work operation was not found.".to_string()))
    }
}
