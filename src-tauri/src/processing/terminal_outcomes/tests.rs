mod classification_tests {
    use super::super::classification::{
        cancellation_error_for_failed_entry, classify_processing_error, classify_terminal_results,
        is_cancellation_error, ProcessingJobTerminalOutcome, RunTerminalClass,
    };
    use crate::errors::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};
    use crate::processing::{ProcessResultEntry, ProcessResultStatus};

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
        let cancelled = ProcessResultEntry {
            input_index: Some(0),
            status: ProcessResultStatus::Cancelled,
            message: "Processing was cancelled".to_string(),
            error: None,
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: None,
        };
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
}

mod entry_tests {
    use super::super::entries::{
        build_all_skipped_batch_result, no_write_skipped_result, terminal_cancelled_result,
        terminal_failure_result,
    };
    use crate::errors::{AppErrorCategory, AppErrorCode, AppErrorEnvelope};
    use crate::output_artifact::{
        CollisionPolicy, OutputKind, PlannedOutputAction, ResolvedOutputPlan,
    };
    use crate::processing::plan::{PlannedProcessingJob, ResolvedProcessingPlan};
    use crate::processing::{JobType, ProcessResultStatus};
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

    fn planned_batch_job(index: usize, action: PlannedOutputAction) -> PlannedProcessingJob {
        PlannedProcessingJob {
            input_index: Some(index),
            input_path: Some(PathBuf::from(format!("/tmp/input-{index}.m4b"))),
            output: output_plan(action, format!("/tmp/output-{index}.m4b")),
            metadata: None,
            cover_art_passthrough: crate::metadata::CoverArtPassthroughPolicy::Preserve,
        }
    }

    fn batch_plan(jobs: Vec<PlannedProcessingJob>) -> ResolvedProcessingPlan {
        ResolvedProcessingPlan {
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
    fn no_write_skipped_result_covers_skip_and_review_required_outputs() {
        let skip_existing = output_plan(
            PlannedOutputAction::SkipExisting,
            "/tmp/existing-output.m4b",
        );
        let review_required = output_plan(
            PlannedOutputAction::ReviewRequired,
            "/tmp/review-output.m4b",
        );
        let writable = output_plan(PlannedOutputAction::Write, "/tmp/new-output.m4b");

        let skip_result = no_write_skipped_result(Some(0), None, &skip_existing)
            .expect("skip-existing output should become a skipped result");
        assert_eq!(skip_result.status, ProcessResultStatus::Skipped);
        assert_eq!(skip_result.input_index, Some(0));
        assert!(skip_result.message.contains("Skipped existing output"));

        let review_result = no_write_skipped_result(Some(1), None, &review_required)
            .expect("review-required output should become a skipped result");
        assert_eq!(review_result.status, ProcessResultStatus::Skipped);
        assert_eq!(review_result.input_index, Some(1));
        assert!(review_result
            .message
            .contains("selected collision policy does not allow overwriting"));

        assert!(
            no_write_skipped_result(Some(2), None, &writable).is_none(),
            "writable outputs must still enter normal processing"
        );
    }
}

mod batch_tests {
    use super::super::batch::{collect_batch_results, TerminalFailureEvent};
    use crate::errors::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};
    use crate::processing::{ProcessResultEntry, ProcessResultStatus};

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

        assert!(matches!(error, AppError::Cancellation(_)));
    }
}
