use super::stage_table::{for_finished_operation, for_result};
use super::state::push_operation_log;
use super::types::{OperationSnapshot, OperationTerminalSummary, WorkOperationStatus};
use crate::processing::{ProcessResultEntry, ProcessResultStatus};

pub(super) struct ChildOutcome<'a> {
    position: usize,
    result: ProcessResultStatus,
    message: String,
    entry: Option<&'a ProcessResultEntry>,
    cancel_requested: Option<bool>,
    stamps_finish: bool,
}

impl<'a> ChildOutcome<'a> {
    pub(super) fn reported(position: usize, entry: &'a ProcessResultEntry) -> Self {
        Self {
            position,
            result: entry.status,
            message: entry.message.clone(),
            entry: Some(entry),
            cancel_requested: None,
            stamps_finish: false,
        }
    }

    pub(super) fn inline(position: usize, result: ProcessResultStatus, message: String) -> Self {
        Self {
            position,
            result,
            message,
            entry: None,
            cancel_requested: Some(false),
            stamps_finish: false,
        }
    }

    pub(super) fn aborted(position: usize, result: ProcessResultStatus, message: String) -> Self {
        Self {
            position,
            result,
            message,
            entry: None,
            cancel_requested: (result == ProcessResultStatus::Cancelled).then_some(true),
            stamps_finish: true,
        }
    }
}

pub(super) struct TerminalPlan<'a> {
    pub status: WorkOperationStatus,
    pub summary: OperationTerminalSummary,
    pub cancel_requested: bool,
    pub children: Vec<ChildOutcome<'a>>,
}

pub(super) fn apply_terminal_plan(
    snapshot: &mut OperationSnapshot,
    plan: TerminalPlan<'_>,
    now_ms: i64,
) {
    let stage = for_finished_operation(plan.status)
        .expect("a terminal plan names a finished operation status")
        .stage;
    snapshot.status = plan.status;
    snapshot.finished_at_ms = Some(now_ms);
    snapshot.cancellable = false;
    snapshot.cancel_requested |= plan.cancel_requested;
    snapshot.progress.stage = stage;
    snapshot.progress.percentage = 100.0;
    snapshot.progress.message = plan.summary.message.clone();
    push_operation_log(snapshot, &plan.summary.message, Some(stage), None);
    snapshot.terminal_summary = Some(plan.summary);

    let mut warning_logs = Vec::new();
    for outcome in plan.children {
        let row = for_result(outcome.result);
        let child = &mut snapshot.children[outcome.position];
        child.status = row.child;
        child.progress.stage = row.stage;
        child.progress.percentage = 100.0;
        child.progress.message = outcome.message.clone();
        child.cancellable = false;
        child.message = Some(outcome.message);
        if let Some(requested) = outcome.cancel_requested {
            child.cancel_requested = requested;
        }
        if outcome.stamps_finish {
            child.finished_at_ms.get_or_insert(now_ms);
        }
        if let Some(entry) = outcome.entry {
            child.job_id = entry.job_id.clone();
            child.supplemental_warning = entry.supplemental_warning.clone();
            child.warnings = entry.warnings.clone();
            child.output_path = entry.output_path.clone();
            if let Some(warning) = &entry.supplemental_warning {
                warning_logs.push((warning.clone(), child.child_job_id.clone()));
            }
        }
    }
    for (warning, child_job_id) in warning_logs {
        push_operation_log(snapshot, &warning, None, Some(child_job_id));
    }
}
