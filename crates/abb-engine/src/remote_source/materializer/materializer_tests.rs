use super::*;

#[test]
fn helper_request_serializes_secrets_only_in_stdin_payload() {
    let request = MaterializationRequest {
        job_id: "job-1".to_string(),
        operation_id: "operation-1".to_string(),
        lane: AaxcleanLane::Aaxc,
        input_path: PathBuf::from("/tmp/source.aaxc"),
        output_temp_path: PathBuf::from("/tmp/materialized.m4b.partial"),
        output_path: PathBuf::from("/tmp/materialized.m4b"),
        secret: AaxcleanSecret::Aaxc {
            key_hex: SecretString::from("0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d"),
            iv_hex: SecretString::from("2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b"),
        },
    };

    let payload = helper_request_json(&request).expect("helper request");
    let value: serde_json::Value = serde_json::from_str(&payload).expect("json");

    assert_eq!(value["schemaVersion"], REQUEST_SCHEMA_VERSION);
    assert_eq!(value["lane"], "aaxc");
    assert_eq!(
        value["secret"]["keyHex"],
        "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d"
    );
    assert!(!payload.contains("license"));
}

#[test]
fn helper_errors_are_sanitized_again_in_rust_boundary() {
    let message = safe_helper_error_message(
        Some("materialization_failed"),
        Some("/tmp/book.aaxc key=secret license=secret"),
    );

    assert!(message.contains("Provider-private details were withheld"));
    assert!(!message.contains("/tmp/book"));
    assert!(!message.contains("secret"));
}
