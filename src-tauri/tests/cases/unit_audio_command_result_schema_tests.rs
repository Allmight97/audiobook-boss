use audiobook_boss_lib::commands::{ProcessCommandResult, ProcessResultEntry, ProcessResultStatus};
use audiobook_boss_lib::processing::RunTerminalClass;
use audiobook_boss_lib::{AppErrorCategory, AppErrorCode, AppErrorEnvelope};

#[test]
fn process_command_result_batch_summary_counts_success_cancelled_and_failures() {
    let results = vec![
        ProcessResultEntry {
            input_index: 0,
            status: ProcessResultStatus::Success,
            message: "ok".to_string(),
            error: None,
            output_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-1".to_string()),
            supplemental_warning: None,
        },
        ProcessResultEntry {
            input_index: 1,
            status: ProcessResultStatus::Cancelled,
            message: "Processing was cancelled".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::ProcessingCancelled,
                AppErrorCategory::Cancellation,
                "Processing was cancelled".to_string(),
                None,
            )),
            output_path: None,
            preview_actual_seconds: None,
            job_id: Some("job-2".to_string()),
            supplemental_warning: None,
        },
        ProcessResultEntry {
            input_index: 2,
            status: ProcessResultStatus::Failed,
            message: "failed".to_string(),
            error: Some(AppErrorEnvelope::new(
                AppErrorCode::InvalidInput,
                AppErrorCategory::Validation,
                "failed".to_string(),
                None,
            )),
            output_path: None,
            preview_actual_seconds: None,
            job_id: None,
            supplemental_warning: None,
        },
    ];

    let response = ProcessCommandResult::new(results.clone());

    assert_eq!(response.summary.total, 3);
    assert_eq!(response.summary.succeeded, 1);
    assert_eq!(response.summary.cancelled, 1);
    assert_eq!(response.summary.failed, 1);
    // success + cancelled + failed observed → backend classifies the run as Mixed.
    assert_eq!(response.terminal_class, RunTerminalClass::Mixed);
    assert_eq!(response.results, results);
}

#[test]
fn process_result_entry_serializes_structured_error_envelope() {
    let entry = ProcessResultEntry {
        input_index: 3,
        status: ProcessResultStatus::Failed,
        message: "Processing was cancelled".to_string(),
        error: Some(AppErrorEnvelope::new(
            AppErrorCode::ProcessingCancelled,
            AppErrorCategory::Cancellation,
            "Processing was cancelled".to_string(),
            None,
        )),
        output_path: None,
        preview_actual_seconds: None,
        job_id: None,
        supplemental_warning: None,
    };

    let json = serde_json::to_value(&entry).expect("entry should serialize");

    assert_eq!(json["error"]["code"], "processing_cancelled");
    assert_eq!(json["error"]["category"], "cancellation");
    assert_eq!(json["error"]["message"], "Processing was cancelled");
    assert!(json["error"]["detail"].is_null());
}
