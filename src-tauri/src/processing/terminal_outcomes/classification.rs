use crate::errors::{AppError, AppErrorEnvelope};
use crate::processing::{ProcessResultEntry, ProcessResultStatus};

#[derive(Debug)]
pub(in crate::processing) enum ProcessingJobTerminalOutcome {
    Success {
        message: String,
        preview_file_path: Option<String>,
        preview_actual_seconds: Option<f64>,
    },
    Cancelled(AppError),
    Failed(AppErrorEnvelope),
}

pub(in crate::processing) fn classify_processing_error(
    error: AppError,
) -> ProcessingJobTerminalOutcome {
    if is_cancellation_error(&error) {
        ProcessingJobTerminalOutcome::Cancelled(error)
    } else {
        ProcessingJobTerminalOutcome::Failed(error.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::processing) enum RunTerminalClass {
    Empty,
    Success,
    Skipped,
    Cancelled,
    Failed,
    Mixed,
}

#[derive(Default)]
pub(super) struct RunTerminalClassifier {
    saw_success: bool,
    saw_skipped: bool,
    saw_cancelled: bool,
    saw_failed: bool,
}

impl RunTerminalClassifier {
    pub(super) fn observe_status(&mut self, status: ProcessResultStatus) {
        match status {
            ProcessResultStatus::Success => self.saw_success = true,
            ProcessResultStatus::Skipped => self.saw_skipped = true,
            ProcessResultStatus::Cancelled => self.saw_cancelled = true,
            ProcessResultStatus::Failed => self.saw_failed = true,
        }
    }

    pub(super) fn observe_error(&mut self, error: &AppError) {
        if is_cancellation_error(error) {
            self.saw_cancelled = true;
        } else {
            self.saw_failed = true;
        }
    }

    pub(super) fn is_fully_cancelled(&self) -> bool {
        self.saw_cancelled && !(self.saw_success || self.saw_skipped || self.saw_failed)
    }

    fn class(&self) -> RunTerminalClass {
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

pub(super) fn classify_terminal_results(results: &[ProcessResultEntry]) -> RunTerminalClass {
    let mut classifier = RunTerminalClassifier::default();
    for result in results {
        classifier.observe_status(result.status);
    }
    classifier.class()
}

pub(super) fn is_cancellation_error(error: &AppError) -> bool {
    matches!(error, AppError::Cancellation(_))
}

pub(super) fn cancellation_error_for_failed_entry(entry: &ProcessResultEntry) -> Option<AppError> {
    let envelope = entry.error.as_ref()?;
    if entry.status == ProcessResultStatus::Failed
        && envelope.category == crate::errors::AppErrorCategory::Cancellation
    {
        return Some(AppError::Cancellation(envelope.message.clone()));
    }
    None
}
