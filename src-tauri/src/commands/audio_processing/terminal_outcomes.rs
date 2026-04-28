use super::plan::ResolvedProcessingPlan;
use crate::audio::output_path::{PlannedOutputAction, ResolvedOutputPlan};
use crate::commands::audio_types::{
    JobType, ProcessCommandResult, ProcessResultEntry, ProcessResultStatus,
};
use crate::errors::sanitize_path_for_display;
use crate::errors::{AppError, AppErrorEnvelope, Result};

#[derive(Debug)]
pub(super) enum ProcessingJobTerminalOutcome {
    Success {
        message: String,
        preview_file_path: Option<String>,
        preview_actual_seconds: Option<f64>,
    },
    Cancelled(AppError),
    Failed(AppErrorEnvelope),
}

pub(super) fn classify_processing_error(error: AppError) -> ProcessingJobTerminalOutcome {
    if is_cancellation_error(&error) {
        ProcessingJobTerminalOutcome::Cancelled(error)
    } else {
        ProcessingJobTerminalOutcome::Failed(error.into())
    }
}

pub(super) fn build_all_skipped_batch_result(
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

#[derive(Debug, PartialEq, Eq)]
pub(super) struct TerminalFailureEvent {
    pub(super) input_index: Option<usize>,
    pub(super) job_id: Option<String>,
    pub(super) message: String,
}

#[derive(Debug, PartialEq)]
pub(super) struct FinalizedBatchResults {
    pub(super) results: Vec<ProcessResultEntry>,
    pub(super) failure_events: Vec<TerminalFailureEvent>,
    pub(super) terminal_class: RunTerminalClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunTerminalClass {
    Empty,
    Success,
    Skipped,
    Cancelled,
    Failed,
    Mixed,
}

#[derive(Default)]
struct RunTerminalClassifier {
    saw_success: bool,
    saw_skipped: bool,
    saw_cancelled: bool,
    saw_failed: bool,
}

impl RunTerminalClassifier {
    fn observe_status(&mut self, status: ProcessResultStatus) {
        match status {
            ProcessResultStatus::Success => self.saw_success = true,
            ProcessResultStatus::Skipped => self.saw_skipped = true,
            ProcessResultStatus::Cancelled => self.saw_cancelled = true,
            ProcessResultStatus::Failed => self.saw_failed = true,
        }
    }

    fn observe_error(&mut self, error: &AppError) {
        if is_cancellation_error(error) {
            self.saw_cancelled = true;
        } else {
            self.saw_failed = true;
        }
    }

    fn is_fully_cancelled(&self) -> bool {
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

fn classify_terminal_results(results: &[ProcessResultEntry]) -> RunTerminalClass {
    let mut classifier = RunTerminalClassifier::default();
    for result in results {
        classifier.observe_status(result.status);
    }
    classifier.class()
}

fn terminal_cancelled_result(
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

pub(super) fn terminal_failure_result(
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

fn is_cancellation_error(error: &AppError) -> bool {
    matches!(error, AppError::Cancellation(_))
}

pub(super) fn collect_batch_results(
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
        classifier.observe_error(&error);
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
    classifier.observe_error(&error);
    if is_cancellation_error(&error) {
        return terminal_cancelled_result(Some(index), None, error.to_string());
    }

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

fn cancellation_error_for_failed_entry(entry: &ProcessResultEntry) -> Option<AppError> {
    let envelope = entry.error.as_ref()?;
    if entry.status == ProcessResultStatus::Failed
        && envelope.category == crate::errors::AppErrorCategory::Cancellation
    {
        return Some(AppError::Cancellation(envelope.message.clone()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        build_all_skipped_batch_result, cancellation_error_for_failed_entry,
        classify_processing_error, classify_terminal_results, collect_batch_results,
        is_cancellation_error, terminal_cancelled_result, terminal_failure_result,
        ProcessingJobTerminalOutcome, RunTerminalClass, TerminalFailureEvent,
    };
    use crate::audio::output_path::{
        CollisionPolicy, OutputKind, PlannedOutputAction, ResolvedOutputPlan,
    };
    use crate::commands::audio_types::{JobType, ProcessResultEntry, ProcessResultStatus};
    use crate::errors::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};
    use std::path::PathBuf;

    fn output_plan(action: PlannedOutputAction, path: impl Into<PathBuf>) -> ResolvedOutputPlan {
        let path = path.into();
        ResolvedOutputPlan {
            kind: OutputKind::Final,
            requested_path: path.clone(),
            resolved_path: path,
            rename_candidate: None,
            collision: None,
            action,
        }
    }

    fn planned_batch_job(
        index: usize,
        action: PlannedOutputAction,
    ) -> super::super::plan::PlannedProcessingJob {
        super::super::plan::PlannedProcessingJob {
            input_index: Some(index),
            input_path: Some(PathBuf::from(format!("/tmp/input-{index}.m4b"))),
            output: output_plan(action, format!("/tmp/output-{index}.m4b")),
            metadata: None,
            allow_passthrough_cover_art: false,
        }
    }

    fn batch_plan(
        jobs: Vec<super::super::plan::PlannedProcessingJob>,
    ) -> super::super::plan::ResolvedProcessingPlan {
        super::super::plan::ResolvedProcessingPlan {
            job_type: JobType::Batch,
            preview_seconds: None,
            collision_policy: CollisionPolicy::SkipExisting,
            plan_signature: "test-plan".to_string(),
            jobs,
        }
    }

    #[test]
    fn terminal_failure_result_preserves_job_id_when_available() {
        let error = AppErrorEnvelope::new(
            AppErrorCode::InternalError,
            AppErrorCategory::Internal,
            "Processing failed".to_string(),
            None,
        );

        let entry = terminal_failure_result(Some(4), Some("job-123".to_string()), error);

        assert_eq!(entry.input_index, Some(4));
        assert_eq!(entry.job_id.as_deref(), Some("job-123"));
        assert_eq!(entry.status, ProcessResultStatus::Failed);
        assert!(entry.error.is_some());
    }

    #[test]
    fn terminal_cancelled_result_preserves_job_id_when_available() {
        let entry = terminal_cancelled_result(
            Some(4),
            Some("job-123".to_string()),
            "Processing was cancelled",
        );

        assert_eq!(entry.input_index, Some(4));
        assert_eq!(entry.job_id.as_deref(), Some("job-123"));
        assert_eq!(entry.status, ProcessResultStatus::Cancelled);
        assert_eq!(entry.message, "Processing was cancelled");
        assert_eq!(
            entry
                .error
                .as_ref()
                .expect("cancelled entry should include structured error")
                .category,
            AppErrorCategory::Cancellation
        );
    }

    #[test]
    fn all_skip_existing_batch_builds_direct_skipped_result() {
        let plan = batch_plan(vec![
            planned_batch_job(0, PlannedOutputAction::SkipExisting),
            planned_batch_job(1, PlannedOutputAction::SkipExisting),
        ]);

        let result =
            build_all_skipped_batch_result(&plan).expect("all skipped batch should short-circuit");

        assert_eq!(result.job_type, JobType::Batch);
        assert_eq!(result.summary.total, 2);
        assert_eq!(result.summary.skipped, 2);
        assert_eq!(result.summary.succeeded, 0);
        assert_eq!(result.summary.failed, 0);
        assert_eq!(result.summary.cancelled, 0);
        assert_eq!(
            result
                .results
                .iter()
                .map(|entry| entry.status)
                .collect::<Vec<_>>(),
            vec![ProcessResultStatus::Skipped, ProcessResultStatus::Skipped]
        );
        assert_eq!(result.results[0].input_index, Some(0));
        assert_eq!(result.results[1].input_index, Some(1));
        assert!(result.results.iter().all(|entry| entry.job_id.is_none()));
    }

    #[test]
    fn all_skip_existing_batch_ignores_mixed_runnable_plan() {
        let plan = batch_plan(vec![
            planned_batch_job(0, PlannedOutputAction::SkipExisting),
            planned_batch_job(1, PlannedOutputAction::Write),
        ]);

        assert!(
            build_all_skipped_batch_result(&plan).is_none(),
            "mixed runnable work must still use normal queue scheduling"
        );
    }

    #[test]
    fn terminal_classification_uses_structured_status_not_result_message() {
        let native_success = ProcessResultEntry {
            input_index: Some(0),
            status: ProcessResultStatus::Success,
            message: "Successfully created audiobook: /tmp/native.m4b".to_string(),
            error: None,
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-native".to_string()),
        };
        let external_success = ProcessResultEntry {
            input_index: Some(1),
            status: ProcessResultStatus::Success,
            message: "Successfully created audiobook: /tmp/external.m4b".to_string(),
            error: None,
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-external".to_string()),
        };

        assert_eq!(
            classify_terminal_results(&[native_success]),
            RunTerminalClass::Success
        );
        assert_eq!(
            classify_terminal_results(&[external_success]),
            RunTerminalClass::Success
        );
    }

    #[test]
    fn terminal_classification_preserves_post_commit_success_truth() {
        let post_commit_cancel_success = ProcessResultEntry {
            input_index: None,
            status: ProcessResultStatus::Success,
            message: "Successfully created audiobook: /tmp/output.m4b".to_string(),
            error: None,
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-123".to_string()),
        };

        assert_eq!(
            classify_terminal_results(&[post_commit_cancel_success]),
            RunTerminalClass::Success
        );
    }

    #[test]
    fn terminal_classification_distinguishes_cancelled_failed_and_mixed_results() {
        let cancelled = terminal_cancelled_result(Some(0), None, "Processing was cancelled");
        let failed = terminal_failure_result(
            Some(1),
            Some("job-2".to_string()),
            AppErrorEnvelope::new(
                AppErrorCode::ToolchainRequired,
                AppErrorCategory::Toolchain,
                "decoder unavailable".to_string(),
                Some("ffmpeg missing".to_string()),
            ),
        );
        let skipped = ProcessResultEntry {
            input_index: Some(2),
            status: ProcessResultStatus::Skipped,
            message: "Skipped existing output at '/tmp/output.m4b'".to_string(),
            error: None,
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: None,
        };

        assert_eq!(
            classify_terminal_results(std::slice::from_ref(&cancelled)),
            RunTerminalClass::Cancelled
        );
        assert_eq!(
            classify_terminal_results(std::slice::from_ref(&failed)),
            RunTerminalClass::Failed
        );
        assert_eq!(
            classify_terminal_results(&[cancelled, failed, skipped]),
            RunTerminalClass::Mixed
        );
    }

    #[test]
    fn processing_error_classification_keeps_cancellation_and_failure_distinct() {
        let cancelled = classify_processing_error(AppError::cancelled());
        let failed = classify_processing_error(AppError::toolchain_required("decoder unavailable"));

        match cancelled {
            ProcessingJobTerminalOutcome::Cancelled(error) => {
                assert!(is_cancellation_error(&error));
            }
            other => panic!("expected cancellation outcome, got {other:?}"),
        }

        match failed {
            ProcessingJobTerminalOutcome::Failed(envelope) => {
                assert_eq!(envelope.code, AppErrorCode::ToolchainRequired);
                assert_eq!(envelope.category, AppErrorCategory::Toolchain);
                assert_eq!(envelope.message, "decoder unavailable");
            }
            other => panic!("expected failure outcome, got {other:?}"),
        }
    }

    #[test]
    fn cancellation_error_for_failed_entry_returns_cancelled_error() {
        let entry = ProcessResultEntry {
            input_index: Some(2),
            status: ProcessResultStatus::Failed,
            message: "Processing was cancelled".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ProcessingCancelled,
                AppErrorCategory::Cancellation,
                "Processing was cancelled".to_string(),
                None,
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-123".to_string()),
        };

        let error = cancellation_error_for_failed_entry(&entry).expect("cancellation error");

        assert!(is_cancellation_error(&error));
        assert_eq!(error.to_string(), "Processing was cancelled");
    }

    #[test]
    fn non_cancellation_errors_stay_failed_results() {
        let entry = ProcessResultEntry {
            input_index: Some(2),
            status: ProcessResultStatus::Failed,
            message: "decoder unavailable".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ToolchainRequired,
                AppErrorCategory::Toolchain,
                "decoder unavailable".to_string(),
                Some("ffmpeg missing".to_string()),
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-123".to_string()),
        };

        assert!(cancellation_error_for_failed_entry(&entry).is_none());
        assert!(!is_cancellation_error(&AppError::toolchain_required(
            "decoder unavailable"
        )));
    }

    #[test]
    fn mixed_cancel_and_fail_classification_keeps_failure_visible() {
        let cancelled = AppError::cancelled();
        let failed = ProcessResultEntry {
            input_index: Some(1),
            status: ProcessResultStatus::Failed,
            message: "decoder unavailable".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ToolchainRequired,
                AppErrorCategory::Toolchain,
                "decoder unavailable".to_string(),
                Some("ffmpeg missing".to_string()),
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-2".to_string()),
        };

        assert!(is_cancellation_error(&cancelled));
        assert!(cancellation_error_for_failed_entry(&failed).is_none());
        assert_eq!(failed.status, ProcessResultStatus::Failed);
        assert_eq!(failed.job_id.as_deref(), Some("job-2"));
    }

    #[test]
    fn collect_batch_results_preserves_mixed_success_and_cancelled_entries() {
        let results = collect_batch_results(
            2,
            vec![
                Ok(ProcessResultEntry {
                    input_index: Some(0),
                    status: ProcessResultStatus::Success,
                    message: "Successfully created audiobook: /tmp/ok.m4b".to_string(),
                    error: None,
                    preview_file_path: None,
                    preview_actual_seconds: None,
                    job_id: Some("job-1".to_string()),
                }),
                Err(AppError::cancelled()),
            ],
        )
        .expect("mixed success and cancellation should remain a successful batch result");

        assert_eq!(results.failure_events, Vec::<TerminalFailureEvent>::new());
        assert_eq!(results.results.len(), 2);
        assert_eq!(results.results[0].status, ProcessResultStatus::Success);
        assert_eq!(results.results[1].status, ProcessResultStatus::Cancelled);
        assert_eq!(results.results[1].input_index, Some(1));
        assert_eq!(results.results[1].message, "Processing was cancelled");
    }

    #[test]
    fn collect_batch_results_preserves_mixed_success_failure_and_cancelled_entries() {
        let failed = ProcessResultEntry {
            input_index: Some(1),
            status: ProcessResultStatus::Failed,
            message: "decoder unavailable".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ToolchainRequired,
                AppErrorCategory::Toolchain,
                "decoder unavailable".to_string(),
                Some("ffmpeg missing".to_string()),
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-2".to_string()),
        };

        let results = collect_batch_results(
            3,
            vec![
                Ok(ProcessResultEntry {
                    input_index: Some(0),
                    status: ProcessResultStatus::Success,
                    message: "Successfully created audiobook: /tmp/ok.m4b".to_string(),
                    error: None,
                    preview_file_path: None,
                    preview_actual_seconds: None,
                    job_id: Some("job-1".to_string()),
                }),
                Ok(failed),
                Err(AppError::cancelled()),
            ],
        )
        .expect("mixed terminal states should remain a successful batch result");

        assert_eq!(results.results.len(), 3);
        assert_eq!(results.results[0].status, ProcessResultStatus::Success);
        assert_eq!(results.results[1].status, ProcessResultStatus::Failed);
        assert_eq!(results.results[2].status, ProcessResultStatus::Cancelled);
        assert_eq!(
            results.failure_events,
            vec![TerminalFailureEvent {
                input_index: Some(1),
                job_id: Some("job-2".to_string()),
                message: "decoder unavailable".to_string(),
            }]
        );
    }

    #[test]
    fn collect_batch_results_repairs_missing_batch_slots_as_failures() {
        let results = collect_batch_results(
            2,
            vec![Ok(ProcessResultEntry {
                input_index: Some(0),
                status: ProcessResultStatus::Success,
                message: "Successfully created audiobook: /tmp/ok.m4b".to_string(),
                error: None,
                preview_file_path: None,
                preview_actual_seconds: None,
                job_id: Some("job-1".to_string()),
            })],
        )
        .expect("missing slot should be repaired as a failed terminal result");

        assert_eq!(results.results.len(), 2);
        assert_eq!(results.results[0].status, ProcessResultStatus::Success);
        assert_eq!(results.results[1].input_index, Some(1));
        assert_eq!(results.results[1].status, ProcessResultStatus::Failed);
        assert_eq!(
            results.failure_events,
            vec![TerminalFailureEvent {
                input_index: Some(1),
                job_id: None,
                message: "Missing terminal result for queued input index 1; marking as failed"
                    .to_string(),
            }]
        );
    }

    #[test]
    fn collect_batch_results_returns_cancelled_when_every_job_cancelled() {
        let error = collect_batch_results(
            2,
            vec![Err(AppError::cancelled()), Err(AppError::cancelled())],
        )
        .expect_err("fully cancelled batch should stay a top-level cancellation");

        assert!(is_cancellation_error(&error));
    }
}
