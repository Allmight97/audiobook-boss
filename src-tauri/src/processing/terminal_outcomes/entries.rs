use crate::errors::sanitize_path_for_display;
use crate::errors::AppErrorEnvelope;
use crate::output_artifact::{PlannedOutputAction, ResolvedOutputPlan};
use crate::processing::plan::ResolvedProcessingPlan;
use crate::processing::{JobType, ProcessCommandResult, ProcessResultEntry, ProcessResultStatus};

pub(in crate::processing) fn build_all_skipped_batch_result(
    plan: &ResolvedProcessingPlan,
) -> Option<ProcessCommandResult> {
    if plan.job_type != JobType::Batch
        || plan.jobs.is_empty()
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
        .map(|job| skipped_result(job.input_index, None, &job.output))
        .collect();
    Some(ProcessCommandResult::new(JobType::Batch, skipped_results))
}

pub(in crate::processing) fn no_write_skipped_result(
    input_index: Option<usize>,
    job_id: Option<String>,
    output: &ResolvedOutputPlan,
) -> Option<ProcessResultEntry> {
    match output.action {
        PlannedOutputAction::SkipExisting => Some(skipped_result(input_index, job_id, output)),
        PlannedOutputAction::ReviewRequired => {
            Some(review_required_skipped_result(input_index, job_id, output))
        }
        _ => None,
    }
}

fn review_required_skipped_result(
    input_index: Option<usize>,
    job_id: Option<String>,
    output: &ResolvedOutputPlan,
) -> ProcessResultEntry {
    let message = format!(
        "Skipped '{}' because the selected collision policy does not allow overwriting this output.",
        sanitize_path_for_display(&output.requested_path)
    );
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Skipped,
        message,
        error: None,
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id,
    }
}

pub(super) fn skipped_result(
    input_index: Option<usize>,
    job_id: Option<String>,
    output: &ResolvedOutputPlan,
) -> ProcessResultEntry {
    let message = format!(
        "Skipped existing output at '{}'",
        sanitize_path_for_display(&output.requested_path)
    );
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Skipped,
        message,
        error: None,
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id,
    }
}

pub(super) fn terminal_cancelled_result(
    input_index: Option<usize>,
    job_id: Option<String>,
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
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id,
    }
}

pub(in crate::processing) fn terminal_failure_result(
    input_index: Option<usize>,
    job_id: Option<String>,
    error: AppErrorEnvelope,
) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index,
        status: ProcessResultStatus::Failed,
        message: error.message.clone(),
        error: Some(error),
        preview_file_path: None,
        preview_actual_seconds: None,
        job_id,
    }
}
