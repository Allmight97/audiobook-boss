use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    ProcessingMerge,
    #[default]
    ProcessingBatch,
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
    let mut classifier = RunTerminalClassifier::default();
    for status in statuses {
        classifier.observe_status(status);
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
    fn classifier_treats_all_errors_cancelled_only_when_no_success_skip_or_failure() {
        let mut classifier = RunTerminalClassifier::default();
        classifier.observe_cancelled();
        assert!(classifier.is_fully_cancelled());

        classifier.observe_status(ProcessResultStatus::Success);
        assert!(!classifier.is_fully_cancelled());
        assert_eq!(classifier.class(), RunTerminalClass::Mixed);
    }
}
