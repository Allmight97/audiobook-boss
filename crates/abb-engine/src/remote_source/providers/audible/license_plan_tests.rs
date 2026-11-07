use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::json;

fn classify(license: &Value) -> LicenseReply {
    classify_license("job-1", "B000000001", license, None)
}

fn refusal(reply: &LicenseReply) -> (RemoteAcquisitionFailureKind, &str) {
    match &reply.plan {
        LicensePlan::Refused { kind, message } => (kind.clone(), message.as_str()),
        _ => panic!("the license should be refused"),
    }
}

fn aaxc_shaped_voucher() -> Value {
    json!({
        "key": URL_SAFE_NO_PAD.encode([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10
        ]),
        "iv": URL_SAFE_NO_PAD.encode([
            0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09,
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01
        ])
    })
}

#[test]
fn an_import_ready_reply_downloads_its_url() {
    let reply = classify(&json!({
        "content_license": {
            "content_url": "https://cdn.example.test/book.m4b",
            "pdf_url": "https://cdn.example.test/book.pdf"
        }
    }));

    assert!(matches!(
        &reply.plan,
        LicensePlan::ImportReady { url } if url == "https://cdn.example.test/book.m4b"
    ));
    assert_eq!(
        reply.supplemental_pdf_url.as_deref(),
        Some("https://cdn.example.test/book.pdf")
    );
}

#[test]
fn the_decrypt_kind_follows_the_key_material_not_the_url_extension() {
    let reply = classify(&json!({
        "content_license": {
            "content_url": "https://cdn.example.test/book.aax",
            "drm_type": "Mpeg",
            "voucher": aaxc_shaped_voucher()
        }
    }));

    match reply.plan {
        LicensePlan::Decrypt { url, kind, .. } => {
            assert_eq!(url, "https://cdn.example.test/book.aax");
            assert_eq!(kind, DecryptKind::Aaxc);
        }
        _ => panic!("a voucher with a key and IV should decrypt"),
    }
}

#[test]
fn a_protected_reply_without_a_key_is_refused_before_any_download() {
    let reply = classify(&json!({
        "content_license": {
            "status_code": "Granted",
            "drm_type": "Mpeg",
            "content_metadata": {"content_url": {"offline_url": "https://cdn.invalid/book.aaxc"}}
        }
    }));

    let (kind, message) = refusal(&reply);
    assert_eq!(kind, RemoteAcquisitionFailureKind::ProtectedUnsupported);
    assert!(message.contains("key needed to decrypt"), "{message}");
}

#[test]
fn a_dash_reply_is_refused_as_unsupported() {
    let reply = classify(&json!({
        "content_license": {
            "content_url": "https://cdn.example.test/manifest.mpd",
            "drm_type": "Mpeg"
        }
    }));

    let (kind, message) = refusal(&reply);
    assert_eq!(kind, RemoteAcquisitionFailureKind::ProtectedUnsupported);
    assert!(
        message.contains("Dash/Widevine materialization is not supported"),
        "{message}"
    );
}

#[test]
fn a_widevine_reply_is_refused_as_unsupported() {
    let reply = classify(&json!({
        "content_license": {
            "content_url": "https://cdn.example.test/book.aaxc",
            "drm_type": "Widevine"
        }
    }));

    let (kind, message) = refusal(&reply);
    assert_eq!(kind, RemoteAcquisitionFailureKind::ProtectedUnsupported);
    assert!(
        message.contains("cannot materialize this lane"),
        "{message}"
    );
}

#[test]
fn a_reply_with_no_download_url_is_a_protocol_failure() {
    let reply = classify(&json!({"content_license": {"status_code": "Granted"}}));

    let (kind, message) = refusal(&reply);
    assert_eq!(
        kind,
        RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed
    );
    assert_eq!(
        message,
        "Audible license response did not include a downloadable audio URL."
    );
}

#[test]
fn a_denied_reply_is_refused_whether_or_not_it_carries_a_url() {
    for content in [
        json!({"content_metadata": {"content_url": {"offline_url": "https://cdn.example.test/book.m4b"}}}),
        json!({}),
    ] {
        let mut license = json!({"content_license": {
            "status_code": "Denied",
            "license_denial_reasons": [
                {"rejectionReason": "RequesterEligibility", "validationType": "Membership"}
            ]
        }});
        license["content_license"]
            .as_object_mut()
            .expect("object")
            .extend(content.as_object().expect("object").clone());

        let reply = classify(&license);

        let (kind, message) = refusal(&reply);
        assert_eq!(kind, RemoteAcquisitionFailureKind::LicenseDenied);
        assert!(
            message.contains("RequesterEligibility (Membership)"),
            "{message}"
        );
    }
}
