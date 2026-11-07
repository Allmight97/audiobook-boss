mod classification_tests {
    use super::super::classification::{
        classify_processing_error, is_cancellation_error, ProcessingJobTerminalOutcome,
    };
    use crate::errors::{AppError, AppErrorCategory, AppErrorCode};

    #[test]
    fn processing_error_classification_keeps_cancellation_and_failure_distinct() {
        let cancelled = classify_processing_error(AppError::cancelled());
        let failed =
            classify_processing_error(AppError::General("decoder unavailable".to_string()));

        match cancelled {
            ProcessingJobTerminalOutcome::Cancelled(error) => {
                assert!(is_cancellation_error(&error));
            }
            other => panic!("expected cancellation outcome, got {other:?}"),
        }

        match failed {
            ProcessingJobTerminalOutcome::Failed(envelope) => {
                assert_eq!(envelope.code, AppErrorCode::InternalError);
                assert_eq!(envelope.category, AppErrorCategory::Internal);
                assert_eq!(envelope.message, "Operation failed: decoder unavailable");
            }
            other => panic!("expected failure outcome, got {other:?}"),
        }
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
    use crate::processing::ProcessResultStatus;
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
            source_metadata: crate::metadata::AudiobookMetadata::default(),
            source_paths: vec![],
            input_index: index,
            input_path: PathBuf::from(format!("/tmp/input-{index}.m4b")),
            output: output_plan(action, format!("/tmp/output-{index}.m4b")),
            metadata: None,
            cover_art_passthrough: crate::metadata::CoverArtPassthroughPolicy::Preserve,
            audio_plan: crate::audio::TitleAudioPlan {
                format: crate::audio::AudiobookFormat::M4b,
                handling: crate::processing::AudioHandling::Encode {
                    settings: crate::audio::EncoderSettings::default(),
                },
                sample_rate: 44_100,
                channels: 1,
                source_codec: "AAC".into(),
                reason: None,
            },
            metadata_intent: None,
        }
    }

    fn batch_plan(jobs: Vec<PlannedProcessingJob>) -> ResolvedProcessingPlan {
        ResolvedProcessingPlan {
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

        let entry = terminal_failure_result(4, Some("job-123".to_string()), error);

        assert_eq!(entry.input_index, 4);
        assert_eq!(entry.job_id.as_deref(), Some("job-123"));
        assert_eq!(entry.status, ProcessResultStatus::Failed);
        assert!(entry.error.is_some());
    }

    #[test]
    fn terminal_cancelled_result_preserves_job_id_when_available() {
        let entry =
            terminal_cancelled_result(4, Some("job-123".to_string()), "Processing was cancelled");

        assert_eq!(entry.input_index, 4);
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
    fn entries_that_wrote_nothing_carry_no_output_path() {
        let error = AppErrorEnvelope::new(
            AppErrorCode::InternalError,
            AppErrorCategory::Internal,
            "Processing failed".to_string(),
            None,
        );
        let existing = output_plan(PlannedOutputAction::SkipExisting, "/tmp/existing.m4b");
        let review = output_plan(PlannedOutputAction::ReviewRequired, "/tmp/review.m4b");
        let entries = [
            no_write_skipped_result(0, None, &existing).expect("skipped"),
            no_write_skipped_result(1, None, &review).expect("skipped"),
            terminal_cancelled_result(2, None, "Processing was cancelled"),
            terminal_failure_result(3, None, error),
        ];

        assert!(entries.iter().all(|entry| entry.output_path.is_none()));
    }

    #[test]
    fn all_skip_existing_batch_builds_direct_skipped_result() {
        let plan = batch_plan(vec![
            planned_batch_job(0, PlannedOutputAction::SkipExisting),
            planned_batch_job(1, PlannedOutputAction::SkipExisting),
        ]);

        let result =
            build_all_skipped_batch_result(&plan).expect("all skipped batch should short-circuit");

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
        assert_eq!(result.results[0].input_index, 0);
        assert_eq!(result.results[1].input_index, 1);
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

        let skip_result = no_write_skipped_result(0, None, &skip_existing)
            .expect("skip-existing output should become a skipped result");
        assert_eq!(skip_result.status, ProcessResultStatus::Skipped);
        assert_eq!(skip_result.input_index, 0);
        assert!(skip_result.message.contains("Skipped existing output"));

        let review_result = no_write_skipped_result(1, None, &review_required)
            .expect("review-required output should become a skipped result");
        assert_eq!(review_result.status, ProcessResultStatus::Skipped);
        assert_eq!(review_result.input_index, 1);
        assert!(review_result
            .message
            .contains("selected collision policy does not allow overwriting"));

        assert!(
            no_write_skipped_result(2, None, &writable).is_none(),
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
        let results = collect_batch_results(vec![
            Ok(ProcessResultEntry {
                input_index: 0,
                status: ProcessResultStatus::Success,
                message: "Successfully created audiobook: /tmp/ok.m4b".to_string(),
                error: None,
                output_path: None,
                supplemental_warning: None,
                warnings: Vec::new(),
                job_id: Some("job-1".to_string()),
            }),
            Err(AppError::cancelled()),
        ])
        .expect("mixed success and cancellation should remain a successful batch result");

        assert_eq!(results.failure_events, Vec::<TerminalFailureEvent>::new());
        assert_eq!(results.results.len(), 2);
        assert_eq!(results.results[0].status, ProcessResultStatus::Success);
        assert_eq!(results.results[1].status, ProcessResultStatus::Cancelled);
        assert_eq!(results.results[1].input_index, 1);
        assert_eq!(results.results[1].message, "Processing was cancelled");
    }

    #[test]
    fn collect_batch_results_preserves_mixed_success_failure_and_cancelled_entries() {
        let failed = ProcessResultEntry {
            input_index: 1,
            status: ProcessResultStatus::Failed,
            message: "decoder unavailable".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::FfmpegError,
                AppErrorCategory::Toolchain,
                "decoder unavailable".to_string(),
                Some("ffmpeg missing".to_string()),
            )),
            output_path: None,
            supplemental_warning: None,
            warnings: Vec::new(),
            job_id: Some("job-2".to_string()),
        };

        let results = collect_batch_results(vec![
            Ok(ProcessResultEntry {
                input_index: 0,
                status: ProcessResultStatus::Success,
                message: "Successfully created audiobook: /tmp/ok.m4b".to_string(),
                error: None,
                output_path: None,
                supplemental_warning: None,
                warnings: Vec::new(),
                job_id: Some("job-1".to_string()),
            }),
            Ok(failed),
            Err(AppError::cancelled()),
        ])
        .expect("mixed terminal states should remain a successful batch result");

        assert_eq!(results.results.len(), 3);
        assert_eq!(results.results[0].status, ProcessResultStatus::Success);
        assert_eq!(results.results[1].status, ProcessResultStatus::Failed);
        assert_eq!(results.results[2].status, ProcessResultStatus::Cancelled);
        assert_eq!(
            results.failure_events,
            vec![TerminalFailureEvent {
                input_index: 1,
                job_id: Some("job-2".to_string()),
                message: "decoder unavailable".to_string(),
            }]
        );
    }

    #[test]
    fn collect_batch_results_returns_cancelled_when_every_job_cancelled() {
        let error =
            collect_batch_results(vec![Err(AppError::cancelled()), Err(AppError::cancelled())])
                .expect_err("fully cancelled batch should stay a top-level cancellation");

        assert!(matches!(error, AppError::Cancellation(_)));
    }
}
