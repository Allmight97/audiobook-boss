use abb_engine::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};

#[test]
fn cancelled_error_maps_to_dedicated_envelope_and_wire_payload() {
    let envelope: AppErrorEnvelope = AppError::cancelled().into();

    assert_eq!(envelope.code, AppErrorCode::ProcessingCancelled);
    assert_eq!(envelope.category, AppErrorCategory::Cancellation);
    assert_eq!(envelope.message, "Processing was cancelled");
    assert_eq!(envelope.detail, None);

    let payload = serde_json::to_value(&envelope).expect("serialize envelope");

    assert!(
        payload.is_object(),
        "envelope should serialize as an object"
    );
    assert_eq!(payload["code"], "processing_cancelled");
    assert_eq!(payload["category"], "cancellation");
    assert_eq!(payload["message"], "Processing was cancelled");
    assert!(payload["detail"].is_null());
}

#[test]
fn wrapped_io_error_keeps_diagnostic_detail() {
    let envelope: AppErrorEnvelope = AppError::Io(std::io::Error::other("disk full")).into();

    assert_eq!(envelope.code, AppErrorCode::IoError);
    assert_eq!(envelope.category, AppErrorCategory::Io);
    assert!(envelope.message.contains("IO operation failed"));
    assert_eq!(envelope.detail.as_deref(), Some("disk full"));
}
