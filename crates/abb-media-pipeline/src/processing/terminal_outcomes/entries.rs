use crate::errors::sanitize_path_for_display;
use crate::errors::AppErrorEnvelope;
use crate::output_artifact::{PlannedOutputAction, ResolvedOutputPlan};
use crate::processing::plan::ResolvedProcessingPlan;
use crate::processing::{ProcessCommandResult, ProcessResultEntry, ProcessResultStatus};

pub(in crate::processing) fn build_all_skipped_batch_result(
    plan: &ResolvedProcessingPlan,
) -> Option<ProcessCommandResult> {
    if plan.jobs.is_empty()
        || !plan
            .jobs
            .iter()
            .all(|job| job.output.action == PlannedOutputAction::SkipExisting)
    {
        return None;
    }

    let skipped_results = plan
        .jobs
        .iter()
        .map(|job| skipped_entry(job.input_index, skip_existing_message(&job.output)))
        .collect();
    Some(ProcessCommandResult::new(skipped_results))
}

pub(in crate::processing) fn no_write_skip_message(output: &ResolvedOutputPlan) -> Option<String> {
    match output.action {
        PlannedOutputAction::SkipExisting => Some(skip_existing_message(output)),
        PlannedOutputAction::ReviewRequired => Some(format!(
            "Skipped '{}' because the selected collision policy does not allow overwriting this output.",
            sanitize_path_for_display(&output.requested_path)
        )),
        _ => None,
    }
}

fn skip_existing_message(output: &ResolvedOutputPlan) -> String {
    format!(
        "Skipped existing output at '{}'",
        sanitize_path_for_display(&output.requested_path)
    )
}

pub(super) fn skipped_entry(input_index: usize, message: String) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Skipped,
        message,
        error: None,
        output_path: None,
        supplemental_warning: None,
        warnings: Vec::new(),
        job_id: None,
    }
}

pub(super) fn terminal_cancelled_result(
    input_index: usize,
    message: impl Into<String>,
) -> ProcessResultEntry {
    let message = message.into();
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Cancelled,
        message: message.clone(),
        error: Some(AppErrorEnvelope::new(
            crate::errors::AppErrorCode::ProcessingCancelled,
            crate::errors::AppErrorCategory::Cancellation,
            message,
            None,
        )),
        output_path: None,
        supplemental_warning: None,
        warnings: Vec::new(),
        job_id: None,
    }
}

pub(super) fn terminal_failure_result(
    input_index: usize,
    job_id: Option<String>,
    error: AppErrorEnvelope,
) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Failed,
        message: error.message.clone(),
        error: Some(error),
        output_path: None,
        supplemental_warning: None,
        warnings: Vec::new(),
        job_id,
    }
}
