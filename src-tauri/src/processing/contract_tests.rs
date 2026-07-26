use crate::processing::{EventStage, OperationKind, ProcessResultStatus, ProgressEvent};

#[test]
fn processing_contract_lifecycle_kinds_are_distinct() {
    assert_ne!(
        OperationKind::ProcessingMerge,
        OperationKind::ProcessingBatch
    );
    assert_ne!(
        OperationKind::MetadataSave,
        OperationKind::RemoteAcquisition
    );
}

#[test]
fn processing_contract_progress_event_shape_matches_frontend_contract() {
    let event = ProgressEvent {
        operation_kind: OperationKind::ProcessingBatch,
        stage: EventStage::Converting,
        percentage: 42.5,
        message: "Converting".to_string(),
        current_file: Some("book.m4b".to_string()),
        eta_seconds: Some(12.0),
        job_id: None,
        input_index: Some(0),
    };

    assert_eq!(event.stage, EventStage::Converting);
    assert!((event.percentage - 42.5).abs() < f32::EPSILON);
    assert_eq!(event.operation_kind, OperationKind::ProcessingBatch);
}

#[test]
fn processing_contract_terminal_status_values_remain_stable() {
    assert_ne!(ProcessResultStatus::Success, ProcessResultStatus::Failed);
    assert_ne!(ProcessResultStatus::Cancelled, ProcessResultStatus::Skipped);
}
