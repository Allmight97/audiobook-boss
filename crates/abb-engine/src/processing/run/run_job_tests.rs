use super::*;
use crate::errors::{AppErrorCategory, AppErrorCode};

fn log_identity(
    operation_id: Option<&str>,
    job_id: &str,
    input_index: usize,
    operation_kind: OperationKind,
) -> ProcessingJobLogIdentity {
    ProcessingJobLogIdentity {
        operation_id: operation_id.map(str::to_string),
        job_id: job_id.to_string(),
        input_index,
        operation_kind,
    }
}

#[test]
fn processing_job_record_format_pins_started_and_terminal_contracts() {
    assert_eq!(
        format_processing_job_record(
            &log_identity(None, "job-123", 0, OperationKind::ProcessingBatch),
            ProcessingJobLogEvent::Started,
            ProcessingJobLogStatus::Running,
            None,
        ),
        "processing_job event=started operation_id=foreground job_id=job-123 input_index=0 kind=processing_batch status=running"
    );

    assert_eq!(
        format_processing_job_record(
            &log_identity(
                Some("operation-123"),
                "job-456",
                4,
                OperationKind::ProcessingBatch,
            ),
            ProcessingJobLogEvent::Terminal,
            ProcessingJobLogStatus::Success,
            Some(321),
        ),
        "processing_job event=terminal operation_id=operation-123 job_id=job-456 input_index=4 kind=processing_batch status=success elapsed_ms=321"
    );

    assert_eq!(
        format_processing_job_record(
            &log_identity(
                Some("operation-123"),
                "job-789",
                5,
                OperationKind::ProcessingBatch,
            ),
            ProcessingJobLogEvent::Terminal,
            ProcessingJobLogStatus::Cancelled,
            Some(654),
        ),
        "processing_job event=terminal operation_id=operation-123 job_id=job-789 input_index=5 kind=processing_batch status=cancelled elapsed_ms=654"
    );
}

#[test]
fn failed_processing_job_record_includes_stable_typed_error_fields() {
    let failure = AppErrorEnvelope::new(
        AppErrorCode::FfmpegError,
        AppErrorCategory::Toolchain,
        "path-free failure",
        None,
    );

    let record = format_processing_job_record(
        &log_identity(
            Some("operation-123"),
            "job-456",
            2,
            OperationKind::ProcessingBatch,
        ),
        ProcessingJobLogEvent::Terminal,
        ProcessingJobLogStatus::Failed(&failure),
        Some(987),
    );

    assert_eq!(
        record,
        "processing_job event=terminal operation_id=operation-123 job_id=job-456 input_index=2 kind=processing_batch status=failed elapsed_ms=987 code=ffmpeg_error category=toolchain"
    );
    assert!(!record.contains(&failure.message));
}
