use super::*;
use crate::errors::{AppErrorCategory, AppErrorCode};

fn log_identity(operation_id: Option<&str>, job_id: &str, input_index: usize) -> JobIdentity {
    JobIdentity {
        operation_id: operation_id.map(str::to_string),
        job_id: Some(job_id.to_string()),
        input_index,
    }
}

#[test]
fn processing_job_record_format_pins_started_and_terminal_contracts() {
    assert_eq!(
        format_processing_job_record(
            &log_identity(None, "job-123", 0),
            OperationKind::ProcessingBatch,
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
                4),
            OperationKind::ProcessingBatch,
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
                5),
            OperationKind::ProcessingBatch,
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
        &log_identity(Some("operation-123"), "job-456", 2),
        OperationKind::ProcessingBatch,
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

#[test]
fn the_requested_fields_are_logged_alphabetically_with_their_operation() {
    use crate::metadata::{AlbumSortPatchOp, MetadataIntentPatch, PatchOp};

    let set = |value: &str| Some(PatchOp::Set(value.to_string()));
    let every_field = MetadataIntentPatch {
        title: set("T"),
        artist: Some(PatchOp::Clear),
        album: set("A"),
        composer: Some(PatchOp::Clear),
        genre: set("G"),
        date: set("2020"),
        description: Some(PatchOp::Clear),
        series: set("S"),
        series_part: Some(PatchOp::Clear),
        subseries: set("SS"),
        subseries_part: Some(PatchOp::Clear),
        album_sort: Some(AlbumSortPatchOp::Clear),
        cover_art: Some(PatchOp::Set(vec![1, 2, 3])),
        comment: set("C"),
        track: Some(PatchOp::Set((1, None))),
        disk: Some(PatchOp::Clear),
    };

    assert_eq!(
        describe_requested_fields(Some(&every_field)),
        "album=set,album_sort=clear,artist=clear,comment=set,composer=clear,cover_art=set,\
         date=set,description=clear,disk=clear,genre=set,series=set,series_part=clear,\
         subseries=set,subseries_part=clear,title=set,track=set"
    );
    assert_eq!(
        describe_requested_fields(Some(&MetadataIntentPatch {
            title: set("T"),
            ..Default::default()
        })),
        "title=set"
    );
    assert_eq!(
        describe_requested_fields(Some(&MetadataIntentPatch::default())),
        "none"
    );
    assert_eq!(describe_requested_fields(None), "none");
}
