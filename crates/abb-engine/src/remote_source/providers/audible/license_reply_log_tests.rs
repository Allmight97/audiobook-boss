use super::*;
use serde_json::json;

#[test]
fn denied_reply_logs_status_and_reason_codes_without_messages() {
    let fields = license_reply_log_fields(&json!({
        "content_license": {
            "status_code": "Denied",
            "drm_type": "Adrm",
            "message": "secret-free-text",
            "license_denial_reasons": [{
                "rejectionReason": "RequesterNotEligible",
                "validationType": "DeviceLimit",
                "message": "You have reached the limit"
            }]
        },
        "response_groups": ["always-returned"]
    }));

    assert!(fields.contains("status_code=Denied"));
    assert!(fields.contains("denial_reasons=RequesterNotEligible/DeviceLimit"));
    assert!(fields.contains("drm_type=Adrm"));
    assert!(fields.contains("reply_keys=content_license,response_groups"));
    assert!(fields.contains("url_key_path=none"));
    assert!(!fields.contains("secret-free-text"));
    assert!(!fields.contains("reached the limit"));
}

#[test]
fn granted_reply_logs_url_path_but_never_the_url() {
    let fields = license_reply_log_fields(&json!({
        "content_license": {
            "status_code": "Granted",
            "license_response": "fake-license-material",
            "content_metadata": {
                "content_reference": {"content_format": "AAX_44_128"},
                "content_url": {"offline_url": "https://cdn.example.test/book.aax?Signature=fake"}
            }
        }
    }));

    assert!(fields.contains("status_code=Granted"));
    assert!(fields.contains("content_format=AAX_44_128"));
    assert!(
        fields.contains("url_key_path=$.content_license.content_metadata.content_url.offline_url")
    );
    assert!(!fields.contains("https://"));
    assert!(!fields.contains("fake-license-material"));
}
