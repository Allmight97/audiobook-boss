use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    #[default]
    ProcessingBatch,
    MetadataSave,
}

impl OperationKind {
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
    #[specta(type = specta_typescript::Number)]
    pub total: usize,
    #[specta(type = specta_typescript::Number)]
    pub succeeded: usize,
    #[specta(type = specta_typescript::Number)]
    pub skipped: usize,
    #[specta(type = specta_typescript::Number)]
    pub cancelled: usize,
    #[specta(type = specta_typescript::Number)]
    pub failed: usize,
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
