use super::classification::{is_cancellation_error, RunTerminalClassifier};
use super::entries::{terminal_cancelled_result, terminal_failure_result};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::processing::{ProcessResultEntry, ProcessResultStatus};

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
    let mut classifier = RunTerminalClassifier::default();

    let results = outcomes
        .into_iter()
        .enumerate()
        .map(|(index, outcome)| {
            normalize_batch_outcome(index, outcome, &mut failure_events, &mut classifier)
        })
        .collect();

    if classifier.is_fully_cancelled() {
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
    classifier: &mut RunTerminalClassifier,
) -> ProcessResultEntry {
    match outcome {
        Ok(entry) => normalize_batch_entry(entry, failure_events, classifier),
        Err(error) => normalize_batch_error(index, error, failure_events, classifier),
    }
}

fn normalize_batch_entry(
    entry: ProcessResultEntry,
    failure_events: &mut Vec<TerminalFailureEvent>,
    classifier: &mut RunTerminalClassifier,
) -> ProcessResultEntry {
    classifier.observe_status(entry.status);
    match entry.status {
        ProcessResultStatus::Failed => {
            failure_events.push(TerminalFailureEvent {
                input_index: entry.input_index,
                job_id: entry.job_id.clone(),
                message: entry.message.clone(),
            });
        }
        ProcessResultStatus::Success
        | ProcessResultStatus::Skipped
        | ProcessResultStatus::Cancelled => {}
    }

    entry
}

fn normalize_batch_error(
    index: usize,
    error: AppError,
    failure_events: &mut Vec<TerminalFailureEvent>,
    classifier: &mut RunTerminalClassifier,
) -> ProcessResultEntry {
    if is_cancellation_error(&error) {
        classifier.observe_cancelled();
        return terminal_cancelled_result(index, None, error.to_string());
    }
    classifier.observe_failure();

    let envelope: AppErrorEnvelope = error.into();
    failure_events.push(TerminalFailureEvent {
        input_index: index,
        job_id: None,
        message: envelope.message.clone(),
    });
    terminal_failure_result(index, None, envelope)
}
