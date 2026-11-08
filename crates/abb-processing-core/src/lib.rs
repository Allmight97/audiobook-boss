use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    #[default]
    ProcessingBatch,
    MetadataSave,
}

impl OperationKind {
    /// The `kind=` value of the dev-log records `scripts/dev-log-analysis.ts` parses.
    pub fn log_label(self) -> &'static str {
        match self {
            OperationKind::ProcessingBatch => "processing_batch",
            OperationKind::MetadataSave => "metadata_save",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationResultSummary {
    pub total: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub cancelled: usize,
    pub failed: usize,
}

impl OperationResultSummary {
    /// Summary for a whole-operation failure that produced no per-item results.
    pub fn all_failed(total: usize) -> Self {
        let total = total.max(1);
        Self {
            total,
            succeeded: 0,
            skipped: 0,
            cancelled: 0,
            failed: total,
        }
    }

    /// Summary for a whole-operation cancellation that produced no per-item results.
    pub fn all_cancelled(total: usize) -> Self {
        let total = total.max(1);
        Self {
            total,
            succeeded: 0,
            skipped: 0,
            cancelled: total,
            failed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProcessResultStatus {
    Success,
    Skipped,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RunTerminalClass {
    Empty,
    Success,
    Skipped,
    Cancelled,
    Failed,
    Mixed,
}

pub fn summarize_result_statuses(
    statuses: impl IntoIterator<Item = ProcessResultStatus>,
) -> OperationResultSummary {
    let mut summary = OperationResultSummary {
        total: 0,
        succeeded: 0,
        skipped: 0,
        cancelled: 0,
        failed: 0,
    };

    for status in statuses {
        summary.total += 1;
        match status {
            ProcessResultStatus::Success => summary.succeeded += 1,
            ProcessResultStatus::Skipped => summary.skipped += 1,
            ProcessResultStatus::Cancelled => summary.cancelled += 1,
            ProcessResultStatus::Failed => summary.failed += 1,
        }
    }

    summary
}

/// Classifies a run from its aggregate counts. Callers starting from a status
/// iterator summarize first via `summarize_result_statuses`.
pub fn classify_run_terminal(summary: &OperationResultSummary) -> RunTerminalClass {
    let mut classes = [
        (summary.succeeded, RunTerminalClass::Success),
        (summary.skipped, RunTerminalClass::Skipped),
        (summary.cancelled, RunTerminalClass::Cancelled),
        (summary.failed, RunTerminalClass::Failed),
    ]
    .into_iter()
    .filter_map(|(count, class)| (count > 0).then_some(class));
    let Some(first) = classes.next() else {
        return RunTerminalClass::Empty;
    };
    if classes.next().is_some() {
        RunTerminalClass::Mixed
    } else {
        first
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
