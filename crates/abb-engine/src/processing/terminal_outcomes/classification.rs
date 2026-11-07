use crate::errors::{AppError, AppErrorEnvelope};

#[derive(Debug)]
pub(in crate::processing) enum ProcessingJobTerminalOutcome {
    Success {
        message: String,
        output_path: Option<String>,
        supplemental_warning: Option<String>,
        warnings: Vec<String>,
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

pub(super) fn is_cancellation_error(error: &AppError) -> bool {
    matches!(error, AppError::Cancellation(_))
}
