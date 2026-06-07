use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    ProcessingMerge,
    #[default]
    ProcessingBatch,
    RemoteAcquisition,
    MetadataSave,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTerminalClass {
    Empty,
    Success,
    Skipped,
    Cancelled,
    Failed,
    Mixed,
}

#[derive(Default)]
pub struct RunTerminalClassifier {
    saw_success: bool,
    saw_skipped: bool,
    saw_cancelled: bool,
    saw_failed: bool,
}

impl RunTerminalClassifier {
    pub fn observe_status(&mut self, status: ProcessResultStatus) {
        match status {
            ProcessResultStatus::Success => self.saw_success = true,
            ProcessResultStatus::Skipped => self.saw_skipped = true,
            ProcessResultStatus::Cancelled => self.saw_cancelled = true,
            ProcessResultStatus::Failed => self.saw_failed = true,
        }
    }

    pub fn observe_failure(&mut self) {
        self.saw_failed = true;
    }

    pub fn observe_cancelled(&mut self) {
        self.saw_cancelled = true;
    }

    pub fn is_fully_cancelled(&self) -> bool {
        self.saw_cancelled && !(self.saw_success || self.saw_skipped || self.saw_failed)
    }

    pub fn class(&self) -> RunTerminalClass {
        let observed = [
            (self.saw_success, RunTerminalClass::Success),
            (self.saw_skipped, RunTerminalClass::Skipped),
            (self.saw_cancelled, RunTerminalClass::Cancelled),
            (self.saw_failed, RunTerminalClass::Failed),
        ];
        let mut classes = observed
            .into_iter()
            .filter_map(|(saw_class, class)| saw_class.then_some(class));
        let Some(first) = classes.next() else {
            return RunTerminalClass::Empty;
        };

        if classes.next().is_some() {
            RunTerminalClass::Mixed
        } else {
            first
        }
    }
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

pub fn classify_terminal_statuses(
    statuses: impl IntoIterator<Item = ProcessResultStatus>,
) -> RunTerminalClass {
    classify_run_terminal(&summarize_result_statuses(statuses))
}

/// Classifies a run from its aggregate counts, sharing the exact rule used by
/// `classify_terminal_statuses` so the status-iterator and count-summary paths
/// can never diverge.
pub fn classify_run_terminal(summary: &OperationResultSummary) -> RunTerminalClass {
    let mut classifier = RunTerminalClassifier::default();
    if summary.succeeded > 0 {
        classifier.observe_status(ProcessResultStatus::Success);
    }
    if summary.skipped > 0 {
        classifier.observe_status(ProcessResultStatus::Skipped);
    }
    if summary.cancelled > 0 {
        classifier.observe_cancelled();
    }
    if summary.failed > 0 {
        classifier.observe_failure();
    }
    classifier.class()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_each_terminal_status() {
        let summary = summarize_result_statuses([
            ProcessResultStatus::Success,
            ProcessResultStatus::Skipped,
            ProcessResultStatus::Cancelled,
            ProcessResultStatus::Failed,
            ProcessResultStatus::Failed,
        ]);

        assert_eq!(
            summary,
            OperationResultSummary {
                total: 5,
                succeeded: 1,
                skipped: 1,
                cancelled: 1,
                failed: 2,
            }
        );
    }

    #[test]
    fn terminal_classification_preserves_empty_single_and_mixed_classes() {
        assert_eq!(classify_terminal_statuses([]), RunTerminalClass::Empty);
        assert_eq!(
            classify_terminal_statuses([ProcessResultStatus::Success]),
            RunTerminalClass::Success
        );
        assert_eq!(
            classify_terminal_statuses([ProcessResultStatus::Cancelled]),
            RunTerminalClass::Cancelled
        );
        assert_eq!(
            classify_terminal_statuses([
                ProcessResultStatus::Success,
                ProcessResultStatus::Failed,
                ProcessResultStatus::Skipped,
            ]),
            RunTerminalClass::Mixed
        );
    }

    #[test]
    fn classify_run_terminal_agrees_with_status_path() {
        let cases = [
            vec![],
            vec![ProcessResultStatus::Success],
            vec![ProcessResultStatus::Skipped],
            vec![ProcessResultStatus::Cancelled],
            vec![ProcessResultStatus::Failed],
            vec![ProcessResultStatus::Success, ProcessResultStatus::Skipped],
            vec![ProcessResultStatus::Success, ProcessResultStatus::Failed],
            vec![
                ProcessResultStatus::Skipped,
                ProcessResultStatus::Cancelled,
                ProcessResultStatus::Failed,
            ],
        ];

        for case in cases {
            let summary = summarize_result_statuses(case.clone());
            assert_eq!(
                classify_run_terminal(&summary),
                classify_terminal_statuses(case),
            );
        }
    }

    #[test]
    fn classify_run_terminal_treats_success_plus_skipped_as_mixed() {
        let summary =
            summarize_result_statuses([ProcessResultStatus::Success, ProcessResultStatus::Skipped]);
        assert_eq!(classify_run_terminal(&summary), RunTerminalClass::Mixed);
    }

    #[test]
    fn classify_run_terminal_treats_skipped_only_as_skipped() {
        let summary = summarize_result_statuses([ProcessResultStatus::Skipped]);
        assert_eq!(classify_run_terminal(&summary), RunTerminalClass::Skipped);
    }

    #[test]
    fn whole_operation_summary_constructors_are_consistent() {
        let failed = OperationResultSummary::all_failed(3);
        assert_eq!(failed.total, 3);
        assert_eq!(failed.failed, 3);
        assert_eq!(classify_run_terminal(&failed), RunTerminalClass::Failed);

        let cancelled = OperationResultSummary::all_cancelled(0);
        assert_eq!(cancelled.total, 1);
        assert_eq!(cancelled.cancelled, 1);
        assert_eq!(
            classify_run_terminal(&cancelled),
            RunTerminalClass::Cancelled
        );
    }

    #[test]
    fn classifier_treats_all_errors_cancelled_only_when_no_success_skip_or_failure() {
        let mut classifier = RunTerminalClassifier::default();
        classifier.observe_cancelled();
        assert!(classifier.is_fully_cancelled());

        classifier.observe_status(ProcessResultStatus::Success);
        assert!(!classifier.is_fully_cancelled());
        assert_eq!(classifier.class(), RunTerminalClass::Mixed);
    }
}
