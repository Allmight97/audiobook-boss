use super::types::{OperationPhase, OperationTerminalSummary, WorkOperationStatus};
use crate::processing::{classify_run_terminal, OperationResultSummary, RunTerminalClass};

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

pub(super) fn phase_of(status: WorkOperationStatus) -> OperationPhase {
    if super::stage_table::for_finished_operation(status).is_some() {
        OperationPhase::Finished
    } else if status == WorkOperationStatus::Accepted {
        OperationPhase::Accepted
    } else {
        OperationPhase::Active
    }
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
