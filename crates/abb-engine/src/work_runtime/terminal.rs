use super::types::{
    ChildJobStatus, OperationTerminalSummary, WorkOperationStatus, WorkProgressStage,
};
use crate::processing::{
    classify_run_terminal, OperationResultSummary, ProcessResultStatus, RunTerminalClass,
};

pub(crate) fn operation_terminal_summary(
    summary: &OperationResultSummary,
    message: String,
) -> OperationTerminalSummary {
    OperationTerminalSummary {
        total: summary.total,
        succeeded: summary.succeeded,
        skipped: summary.skipped,
        cancelled: summary.cancelled,
        failed: summary.failed,
        message,
    }
}

/// Terminal status from a raw result summary via the canonical classifier.
/// Shared by the processing path and command-driven inline operations
/// (metadata save) so both map `RunTerminalClass → WorkOperationStatus`
/// identically. Do not reintroduce a parallel rule from snapshot counts.
pub(crate) fn work_status_from_summary(summary: &OperationResultSummary) -> WorkOperationStatus {
    work_status_from_terminal_class(classify_run_terminal(summary))
}

pub(crate) fn terminal_summary_from_summary(
    summary: &OperationResultSummary,
) -> OperationTerminalSummary {
    operation_terminal_summary(summary, terminal_message(summary))
}

fn work_status_from_terminal_class(class: RunTerminalClass) -> WorkOperationStatus {
    match class {
        RunTerminalClass::Empty | RunTerminalClass::Success | RunTerminalClass::Skipped => {
            WorkOperationStatus::Completed
        }
        RunTerminalClass::Cancelled => WorkOperationStatus::Cancelled,
        RunTerminalClass::Failed => WorkOperationStatus::Failed,
        RunTerminalClass::Mixed => WorkOperationStatus::Mixed,
    }
}

pub(crate) fn child_status_from_result_status(status: ProcessResultStatus) -> ChildJobStatus {
    match status {
        ProcessResultStatus::Success => ChildJobStatus::Completed,
        ProcessResultStatus::Skipped => ChildJobStatus::Skipped,
        ProcessResultStatus::Cancelled => ChildJobStatus::Cancelled,
        ProcessResultStatus::Failed => ChildJobStatus::Failed,
    }
}

pub(crate) fn stage_from_status(status: WorkOperationStatus) -> WorkProgressStage {
    match status {
        WorkOperationStatus::Completed | WorkOperationStatus::Mixed => WorkProgressStage::Complete,
        WorkOperationStatus::Cancelled => WorkProgressStage::Cancelled,
        WorkOperationStatus::Failed => WorkProgressStage::Failed,
        WorkOperationStatus::Accepted => WorkProgressStage::Pending,
        WorkOperationStatus::Running => WorkProgressStage::Converting,
        WorkOperationStatus::Cancelling => WorkProgressStage::Cleaning,
    }
}

pub(crate) fn stage_from_child_status(status: ChildJobStatus) -> WorkProgressStage {
    match status {
        ChildJobStatus::Completed | ChildJobStatus::Skipped => WorkProgressStage::Complete,
        ChildJobStatus::Cancelled => WorkProgressStage::Cancelled,
        ChildJobStatus::Failed => WorkProgressStage::Failed,
        ChildJobStatus::Queued => WorkProgressStage::Pending,
        ChildJobStatus::Running => WorkProgressStage::Converting,
    }
}

pub(crate) fn is_terminal(status: WorkOperationStatus) -> bool {
    matches!(
        status,
        WorkOperationStatus::Completed
            | WorkOperationStatus::Cancelled
            | WorkOperationStatus::Failed
            | WorkOperationStatus::Mixed
    )
}

fn terminal_message(summary: &OperationResultSummary) -> String {
    if summary.failed > 0 {
        return format!(
            "Finished with {} succeeded, {} failed, {} skipped, {} cancelled.",
            summary.succeeded, summary.failed, summary.skipped, summary.cancelled
        );
    }
    if summary.cancelled > 0 {
        if summary.skipped > 0 {
            if summary.succeeded == 0 {
                return format!(
                    "Finished with {} skipped and {} cancelled.",
                    summary.skipped, summary.cancelled
                );
            }
            return format!(
                "Finished with {} succeeded, {} skipped, {} cancelled.",
                summary.succeeded, summary.skipped, summary.cancelled
            );
        }
        return format!(
            "Finished with {} succeeded and {} cancelled.",
            summary.succeeded, summary.cancelled
        );
    }
    if summary.skipped > 0 && summary.succeeded > 0 {
        return format!(
            "Finished with {} succeeded and {} skipped.",
            summary.succeeded, summary.skipped
        );
    }
    if summary.skipped > 0 && summary.succeeded == 0 {
        return format!("Skipped {} item(s).", summary.skipped);
    }
    format!("Completed {} item(s).", summary.succeeded + summary.skipped)
}
