use super::classification::{
    cancellation_error_for_failed_entry, classify_terminal_results, is_cancellation_error,
    RunTerminalClass, RunTerminalClassifier,
};
use super::entries::{terminal_cancelled_result, terminal_failure_result};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::processing::{ProcessResultEntry, ProcessResultStatus};

#[derive(Debug, PartialEq, Eq)]
pub(in crate::processing) struct TerminalFailureEvent {
    pub(in crate::processing) input_index: Option<usize>,
    pub(in crate::processing) job_id: Option<String>,
    pub(in crate::processing) message: String,
}

#[derive(Debug, PartialEq)]
pub(in crate::processing) struct FinalizedBatchResults {
    pub(in crate::processing) results: Vec<ProcessResultEntry>,
    pub(in crate::processing) failure_events: Vec<TerminalFailureEvent>,
    pub(in crate::processing) terminal_class: RunTerminalClass,
}

pub(in crate::processing) fn collect_batch_results(
    input_count: usize,
    outcomes: Vec<Result<ProcessResultEntry>>,
) -> Result<FinalizedBatchResults> {
    let mut ordered_results: Vec<Option<ProcessResultEntry>> = vec![None; input_count];
    let mut failure_events = Vec::new();
    let mut classifier = RunTerminalClassifier::default();

    for (index, outcome) in outcomes.into_iter().enumerate() {
        let entry = normalize_batch_outcome(index, outcome, &mut failure_events, &mut classifier);
        ordered_results[index] = Some(entry);
    }

    if classifier.is_fully_cancelled() {
        return Err(AppError::cancelled());
    }

    repair_missing_batch_results(&mut ordered_results, &mut failure_events);

    let results: Vec<ProcessResultEntry> = ordered_results.into_iter().flatten().collect();
    let terminal_class = classify_terminal_results(&results);

    Ok(FinalizedBatchResults {
        results,
        failure_events,
        terminal_class,
    })
}

fn normalize_batch_outcome(
    index: usize,
    outcome: Result<ProcessResultEntry>,
    failure_events: &mut Vec<TerminalFailureEvent>,
    classifier: &mut RunTerminalClassifier,
) -> ProcessResultEntry {
    match outcome {
        Ok(entry) => normalize_batch_entry(index, entry, failure_events, classifier),
        Err(error) => normalize_batch_error(index, error, failure_events, classifier),
    }
}

fn normalize_batch_entry(
    index: usize,
    mut entry: ProcessResultEntry,
    failure_events: &mut Vec<TerminalFailureEvent>,
    classifier: &mut RunTerminalClassifier,
) -> ProcessResultEntry {
    if entry.input_index.is_none() {
        entry.input_index = Some(index);
    }

    if let Some(error) = cancellation_error_for_failed_entry(&entry) {
        classifier.observe_cancelled();
        return terminal_cancelled_result(
            entry.input_index,
            entry.job_id.clone(),
            error.to_string(),
        );
    }

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
        return terminal_cancelled_result(Some(index), None, error.to_string());
    }
    classifier.observe_failure();

    let envelope: AppErrorEnvelope = error.into();
    failure_events.push(TerminalFailureEvent {
        input_index: Some(index),
        job_id: None,
        message: envelope.message.clone(),
    });
    terminal_failure_result(Some(index), None, envelope)
}

fn repair_missing_batch_results(
    ordered_results: &mut [Option<ProcessResultEntry>],
    failure_events: &mut Vec<TerminalFailureEvent>,
) {
    for (index, slot) in ordered_results.iter_mut().enumerate() {
        if slot.is_none() {
            let error_message = format!(
                "Missing terminal result for queued input index {index}; marking as failed"
            );
            failure_events.push(TerminalFailureEvent {
                input_index: Some(index),
                job_id: None,
                message: error_message.clone(),
            });
            *slot = Some(terminal_failure_result(
                Some(index),
                None,
                AppErrorEnvelope::new(
                    crate::errors::AppErrorCode::InternalError,
                    crate::errors::AppErrorCategory::Internal,
                    error_message,
                    None,
                ),
            ));
        }
    }
}
