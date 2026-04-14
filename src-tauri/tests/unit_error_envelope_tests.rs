use audiobook_boss_lib::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};

#[test]
fn cancelled_error_maps_to_dedicated_envelope_and_invoke_error_payload() {
    let envelope: AppErrorEnvelope = AppError::cancelled().into();

    assert_eq!(envelope.code, AppErrorCode::ProcessingCancelled);
    assert_eq!(envelope.category, AppErrorCategory::Cancellation);
    assert_eq!(envelope.message, "Processing was cancelled");
    assert_eq!(envelope.detail, None);

    let invoke_error: tauri::ipc::InvokeError = AppError::cancelled().into();
    let payload = invoke_error.0;

    assert!(
        payload.is_object(),
        "invoke error should serialize as an object"
    );
    assert_eq!(payload["code"], "processing_cancelled");
    assert_eq!(payload["category"], "cancellation");
    assert_eq!(payload["message"], "Processing was cancelled");
    assert!(payload["detail"].is_null());
}

#[test]
fn toolchain_required_error_uses_dedicated_category() {
    let envelope: AppErrorEnvelope =
        AppError::toolchain_required("FDK AAC requires a validated external FFmpeg toolchain.")
            .into();

    assert_eq!(envelope.code, AppErrorCode::ToolchainRequired);
    assert_eq!(envelope.category, AppErrorCategory::Toolchain);
    assert_eq!(
        envelope.message,
        "FDK AAC requires a validated external FFmpeg toolchain."
    );
    assert_eq!(envelope.detail, None);
}

#[test]
fn wrapped_io_error_keeps_diagnostic_detail() {
    let envelope: AppErrorEnvelope = AppError::Io(std::io::Error::other("disk full")).into();

    assert_eq!(envelope.code, AppErrorCode::IoError);
    assert_eq!(envelope.category, AppErrorCategory::Io);
    assert!(envelope.message.contains("IO operation failed"));
    assert_eq!(envelope.detail.as_deref(), Some("disk full"));
}
