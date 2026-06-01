use crate::errors::{AppError, AppErrorEnvelope};
use crate::processing::{ProcessResultEntry, ProcessResultStatus};
pub(in crate::processing) use abb_processing_core::RunTerminalClass;

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

pub(super) type RunTerminalClassifier = abb_processing_core::RunTerminalClassifier;

pub(super) fn classify_terminal_results(results: &[ProcessResultEntry]) -> RunTerminalClass {
    abb_processing_core::classify_terminal_statuses(results.iter().map(|result| result.status))
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
