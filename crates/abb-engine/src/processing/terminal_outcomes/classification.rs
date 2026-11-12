use super::entries::{skipped_entry, terminal_cancelled_result, terminal_failure_result};
use crate::errors::{AppError, AppErrorEnvelope};
use crate::execution::{EmitContext, ProgressEmitter};
use crate::processing::{
    JobIdentity, OperationKind, ProcessResultEntry, ProcessResultStatus, ProgressEventListener,
};

const CANCELLED_EVENT_MESSAGE: &str = "Processing was cancelled";

#[derive(Debug)]
pub(in crate::processing) enum TitleOutcome {
    Published {
        job_id: String,
        message: String,
        output_path: Option<String>,
        supplemental_warning: Option<String>,
        warnings: Vec<String>,
    },
    Skipped {
        message: String,
    },
    Cancelled {
        message: String,
    },
    Failed {
        job_id: Option<String>,
        error: AppErrorEnvelope,
    },
}

impl TitleOutcome {
    pub(in crate::processing) fn from_error(job_id: Option<String>, error: AppError) -> Self {
        if is_cancellation_error(&error) {
            Self::Cancelled {
                message: error.to_string(),
            }
        } else {
            Self::Failed {
                job_id,
                error: error.into(),
            }
        }
    }

    pub(in crate::processing) fn is_published(&self) -> bool {
        matches!(self, Self::Published { .. })
    }

    pub(in crate::processing) fn report(
        self,
        identity: JobIdentity,
        listener: &ProgressEventListener,
    ) -> ProcessResultEntry {
        let input_index = identity.input_index;
        let emitter = |job_id: Option<String>| {
            ProgressEmitter::new(
                EmitContext {
                    operation_kind: OperationKind::ProcessingBatch,
                    identity: JobIdentity {
                        job_id,
                        ..identity.clone()
                    },
                },
                listener.clone(),
            )
        };
        match self {
            Self::Published {
                job_id,
                message,
                output_path,
                supplemental_warning,
                warnings,
            } => ProcessResultEntry {
                input_index,
                status: ProcessResultStatus::Success,
                message,
                error: None,
                output_path,
                job_id: Some(job_id),
                supplemental_warning,
                warnings,
            },
            Self::Skipped { message } => {
                emitter(None).emit_terminal_skipped(&message);
                skipped_entry(input_index, None, message)
            }
            Self::Cancelled { message } => {
                emitter(None).emit_terminal_cancelled(CANCELLED_EVENT_MESSAGE);
                terminal_cancelled_result(input_index, None, message)
            }
            Self::Failed { job_id, error } => {
                emitter(job_id.clone()).emit_terminal_failed(&error.message);
                terminal_failure_result(input_index, job_id, error)
            }
        }
    }
}

pub(super) fn is_cancellation_error(error: &AppError) -> bool {
    matches!(error, AppError::Cancellation(_))
}
