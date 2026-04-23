use audiobook_boss_lib::commands::{
    JobType, ProcessCommandResult, ProcessResultEntry, ProcessResultStatus,
};
use audiobook_boss_lib::{AppErrorCategory, AppErrorCode, AppErrorEnvelope};

#[test]
fn process_command_result_batch_summary_counts_success_cancelled_and_failures() {
    let results = vec![
        ProcessResultEntry {
            input_index: Some(0),
            status: ProcessResultStatus::Success,
            message: "ok".to_string(),
            error: None,
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-1".to_string()),
        },
        ProcessResultEntry {
            input_index: Some(1),
            status: ProcessResultStatus::Cancelled,
            message: "Processing was cancelled".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ProcessingCancelled,
                AppErrorCategory::Cancellation,
                "Processing was cancelled".to_string(),
                None,
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-2".to_string()),
        },
        ProcessResultEntry {
            input_index: Some(2),
            status: ProcessResultStatus::Failed,
            message: "failed".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::InvalidInput,
                AppErrorCategory::Validation,
                "failed".to_string(),
                None,
            )),
            preview_file_path: None,
            preview_actual_seconds: None,
            job_id: None,
        },
    ];

    let response = ProcessCommandResult::new(JobType::Batch, results.clone());

    assert_eq!(response.job_type, JobType::Batch);
    assert_eq!(response.summary.total, 3);
    assert_eq!(response.summary.succeeded, 1);
    assert_eq!(response.summary.cancelled, 1);
    assert_eq!(response.summary.failed, 1);
    assert_eq!(response.results, results);
}

#[test]
fn process_command_result_merge_has_single_success_entry() {
    let entry = ProcessResultEntry {
        input_index: None,
        status: ProcessResultStatus::Success,
        message: "merged".to_string(),
        error: None,
        preview_file_path: Some("/tmp/out.preview.m4b".to_string()),
        preview_actual_seconds: Some(30.0),
        job_id: Some("job-merge".to_string()),
    };

    let response = ProcessCommandResult::new(JobType::Merge, vec![entry.clone()]);

    assert_eq!(response.job_type, JobType::Merge);
    assert_eq!(response.summary.total, 1);
    assert_eq!(response.summary.succeeded, 1);
    assert_eq!(response.summary.cancelled, 0);
    assert_eq!(response.summary.failed, 0);
    assert_eq!(response.results, vec![entry]);
}

#[test]
fn process_result_entry_serializes_structured_error_envelope() {
    let entry = ProcessResultEntry {
        input_index: Some(3),
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
        job_id: None,
    };

    let json = serde_json::to_value(&entry).expect("entry should serialize");

    assert_eq!(json["error"]["code"], "processing_cancelled");
    assert_eq!(json["error"]["category"], "cancellation");
    assert_eq!(json["error"]["message"], "Processing was cancelled");
    assert!(json["error"]["detail"].is_null());
}
