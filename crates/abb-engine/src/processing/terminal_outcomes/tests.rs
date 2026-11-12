mod classification_tests {
    use super::super::classification::{is_cancellation_error, TitleOutcome};
    use crate::errors::{AppError, AppErrorCategory, AppErrorCode};
    use crate::processing::{
        EventStage, JobIdentity, ProcessResultStatus, ProgressEvent, ProgressEventListener,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn processing_error_classification_keeps_cancellation_and_failure_distinct() {
        let cancelled = TitleOutcome::from_error(None, AppError::cancelled());
        let failed = TitleOutcome::from_error(
            Some("job-1".to_string()),
            AppError::General("decoder unavailable".to_string()),
        );

        assert!(is_cancellation_error(&AppError::cancelled()));
        assert!(matches!(cancelled, TitleOutcome::Cancelled { .. }));
        match failed {
            TitleOutcome::Failed { job_id, error } => {
                assert_eq!(job_id.as_deref(), Some("job-1"));
                assert_eq!(error.code, AppErrorCode::InternalError);
                assert_eq!(error.category, AppErrorCategory::Internal);
                assert_eq!(error.message, "Operation failed: decoder unavailable");
            }
            other => panic!("expected failure outcome, got {other:?}"),
        }
    }

    fn recorded() -> (ProgressEventListener, Arc<Mutex<Vec<ProgressEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let listener: ProgressEventListener = Arc::new(move |event: &ProgressEvent| {
            sink.lock().expect("events").push(event.clone());
        });
        (listener, events)
    }

    fn identity(input_index: usize) -> JobIdentity {
        JobIdentity {
            operation_id: "operation".to_string(),
            job_id: None,
            input_index,
        }
    }

    #[test]
    fn each_unpublished_outcome_reports_one_terminal_event_with_its_entry() {
        let (listener, events) = recorded();
        let failed = TitleOutcome::from_error(
            Some("job-7".to_string()),
            AppError::General("decoder unavailable".to_string()),
        )
        .report(identity(0), &listener);
        let cancelled =
            TitleOutcome::from_error(None, AppError::cancelled()).report(identity(1), &listener);
        let skipped = TitleOutcome::Skipped {
            message: "Skipped existing output".to_string(),
        }
        .report(identity(2), &listener);

        assert_eq!(
            [failed.status, cancelled.status, skipped.status],
            [
                ProcessResultStatus::Failed,
                ProcessResultStatus::Cancelled,
                ProcessResultStatus::Skipped
            ]
        );
        assert_eq!(failed.job_id.as_deref(), Some("job-7"));
        let events = events.lock().expect("events");
        let ends: Vec<_> = events
            .iter()
            .map(|event| (event.stage, event.input_index, event.job_id.clone()))
            .collect();
        assert_eq!(
            ends,
            [
                (EventStage::Failed, Some(0), Some("job-7".to_string())),
                (EventStage::Cancelled, Some(1), None),
                (EventStage::Skipped, Some(2), None),
            ]
        );
    }

    #[test]
    fn a_published_outcome_reports_no_second_event() {
        let (listener, events) = recorded();
        let entry = TitleOutcome::Published {
            job_id: "job-1".to_string(),
            message: "Exported".to_string(),
            output_path: Some("/tmp/out.m4b".to_string()),
            supplemental_warning: None,
            warnings: Vec::new(),
        }
        .report(identity(0), &listener);

        assert_eq!(entry.status, ProcessResultStatus::Success);
        assert!(events.lock().expect("events").is_empty());
    }
}

mod entry_tests {
    use super::super::entries::{
        build_all_skipped_batch_result, no_write_skip_message, skipped_entry,
        terminal_cancelled_result, terminal_failure_result,
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
            skipped_entry(0, None, no_write_skip_message(&existing).expect("skipped")),
            skipped_entry(1, None, no_write_skip_message(&review).expect("skipped")),
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
    fn no_write_skip_message_covers_skip_and_review_required_outputs() {
        let skip_existing = output_plan(
            PlannedOutputAction::SkipExisting,
            "/tmp/existing-output.m4b",
        );
        let review_required = output_plan(
            PlannedOutputAction::ReviewRequired,
            "/tmp/review-output.m4b",
        );
        let writable = output_plan(PlannedOutputAction::Write, "/tmp/new-output.m4b");

        assert!(no_write_skip_message(&skip_existing)
            .expect("skip-existing output should be skipped")
            .contains("Skipped existing output"));
        assert!(no_write_skip_message(&review_required)
            .expect("review-required output should be skipped")
            .contains("selected collision policy does not allow overwriting"));
        assert!(
            no_write_skip_message(&writable).is_none(),
            "writable outputs must still enter normal processing"
        );
    }
}

mod batch_tests {
    use super::super::batch::batch_results;
    use super::super::entries::{terminal_cancelled_result, terminal_failure_result};
    use crate::errors::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};
    use crate::processing::{ProcessResultEntry, ProcessResultStatus};

    fn published(input_index: usize) -> ProcessResultEntry {
        ProcessResultEntry {
            input_index,
            status: ProcessResultStatus::Success,
            message: "Successfully created audiobook: /tmp/ok.m4b".to_string(),
            error: None,
            output_path: None,
            supplemental_warning: None,
            warnings: Vec::new(),
            job_id: Some("job-1".to_string()),
        }
    }

    #[test]
    fn a_batch_with_any_uncancelled_title_keeps_every_entry() {
        let failure = AppErrorEnvelope::new(
            AppErrorCode::FfmpegError,
            AppErrorCategory::Toolchain,
            "decoder unavailable".to_string(),
            None,
        );
        let results = batch_results(vec![
            published(0),
            terminal_failure_result(1, Some("job-2".to_string()), failure),
            terminal_cancelled_result(2, None, "Processing was cancelled"),
        ])
        .expect("mixed terminal states stay a batch result");

        let statuses: Vec<_> = results.iter().map(|entry| entry.status).collect();
        assert_eq!(
            statuses,
            [
                ProcessResultStatus::Success,
                ProcessResultStatus::Failed,
                ProcessResultStatus::Cancelled
            ]
        );
    }

    #[test]
    fn a_batch_whose_every_title_was_cancelled_is_a_cancellation() {
        let error = batch_results(vec![
            terminal_cancelled_result(0, None, "Processing was cancelled"),
            terminal_cancelled_result(1, None, "Processing was cancelled"),
        ])
        .expect_err("fully cancelled batch should stay a top-level cancellation");

        assert!(matches!(error, AppError::Cancellation(_)));
    }
}
