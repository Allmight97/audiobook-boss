use super::classification::is_cancellation_error;
use super::entries::{terminal_cancelled_result, terminal_failure_result};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::processing::{
    classify_run_terminal, ProcessResultEntry, ProcessResultStatus, RunTerminalClass,
};

#[derive(Debug, PartialEq, Eq)]
pub(in crate::processing) struct TerminalFailureEvent {
    pub(in crate::processing) input_index: usize,
    pub(in crate::processing) job_id: Option<String>,
    pub(in crate::processing) message: String,
}

#[derive(Debug, PartialEq)]
pub(in crate::processing) struct FinalizedBatchResults {
    pub(in crate::processing) results: Vec<ProcessResultEntry>,
    pub(in crate::processing) failure_events: Vec<TerminalFailureEvent>,
}

pub(in crate::processing) fn collect_batch_results(
    outcomes: Vec<Result<ProcessResultEntry>>,
) -> Result<FinalizedBatchResults> {
    let mut failure_events = Vec::new();

    let results: Vec<_> = outcomes
        .into_iter()
        .enumerate()
        .map(|(index, outcome)| normalize_batch_outcome(index, outcome, &mut failure_events))
        .collect();

    let summary =
        abb_processing_core::summarize_result_statuses(results.iter().map(|entry| entry.status));
    if classify_run_terminal(&summary) == RunTerminalClass::Cancelled {
        return Err(AppError::cancelled());
    }

    Ok(FinalizedBatchResults {
        results,
        failure_events,
    })
}

fn normalize_batch_outcome(
    index: usize,
    outcome: Result<ProcessResultEntry>,
    failure_events: &mut Vec<TerminalFailureEvent>,
) -> ProcessResultEntry {
    match outcome {
        Ok(entry) => normalize_batch_entry(entry, failure_events),
        Err(error) => normalize_batch_error(index, error, failure_events),
    }
}

fn normalize_batch_entry(
    entry: ProcessResultEntry,
    failure_events: &mut Vec<TerminalFailureEvent>,
) -> ProcessResultEntry {
    if entry.status == ProcessResultStatus::Failed {
        failure_events.push(TerminalFailureEvent {
            input_index: entry.input_index,
            job_id: entry.job_id.clone(),
            message: entry.message.clone(),
        });
    }

    entry
}

fn normalize_batch_error(
    index: usize,
    error: AppError,
    failure_events: &mut Vec<TerminalFailureEvent>,
) -> ProcessResultEntry {
    if is_cancellation_error(&error) {
        return terminal_cancelled_result(index, None, error.to_string());
    }

    let envelope: AppErrorEnvelope = error.into();
    failure_events.push(TerminalFailureEvent {
        input_index: index,
        job_id: None,
        message: envelope.message.clone(),
    });
    terminal_failure_result(index, None, envelope)
}
