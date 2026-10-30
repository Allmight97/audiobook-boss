use super::*;
use serde_json::json;

#[test]
fn audio_url_priority_survives_competing_branches_and_empty_candidates() {
    let book = "https://cdn.example.test/book.aaxc";
    for response in [
        json!({"a": {"url": "https://cdn.example.test/cover.jpg"},
            "z": {"content_url": book}, "license": "voucher"}),
        json!({"url": "https://cdn.example.test/sample.m4b",
            "content_license": {"content_metadata": {"content_url": {"offline_url": book}}},
            "license": "voucher"}),
        json!({"a": {"content_url": ""}, "z": [{"content_url": book}],
            "license": "voucher"}),
    ] {
        let facts = license_facts_from_value(&response);
        assert_eq!(facts.content_url.as_deref(), Some(book));
        assert_eq!(
            choose_acquisition_strategy(&facts),
            AcquisitionStrategy::DownloadThenDecryptAaxc
        );
    }
    let fallback = license_facts_from_value(&json!({
        "content_url": "", "nested": {"url": "https://cdn.example.test/book.m4b"}
    }));
    assert_eq!(
        choose_acquisition_strategy(&fallback),
        AcquisitionStrategy::DownloadImportReady
    );
}

#[test]
fn license_response_facts_classify_content_protection_and_voucher() {
    let response = json!({
        "content_license": {
            "content_metadata": {
                "content_url": {
                    "offline_url": "https://cdn.example.test/book.aaxc?Signature=fake-secret"
                }
            },
            "drm_type": "Mpeg",
            "voucher": "fake-voucher-material",
            "license": "fake-license-material"
        },
        "details": {
            "pdf_url": "https://cdn.example.test/book.pdf?token=fake-pdf-token"
        }
    });

    let facts = license_facts_from_value(&response);

    assert_eq!(facts.content_kind, MaterializedSourceKind::EncryptedAaxc);
    assert_eq!(facts.media_container, MediaContainerKind::Aaxc);
    assert_eq!(facts.media_protection, MediaProtectionKind::AudibleAaxc);
    assert!(facts.content_url.is_some());
    assert!(facts.decryption_material_present);
    assert_eq!(facts.drm_kind.as_deref(), Some("Mpeg"));
    assert!(facts.supplemental_pdf_url.is_some());
}

#[test]
fn denied_license_reports_its_reason_codes_and_throttling() {
    let response = json!({
        "content_license": {
            "status_code": "Denied",
            "drm_type": "Adrm",
            "message": "free text",
            "content_metadata": {"content_reference": {"content_format": "AAX_44_128"}},
            "license_denial_reasons": [
                {"rejectionReason": "RequesterEligibility", "validationType": "Membership", "message": "m"},
                {"rejectionReason": "CustomerThrottled", "validationType": "Ownership", "message": "m"}
            ]
        },
        "response_groups": ["content_reference"]
    });

    let facts = license_facts_from_value(&response);

    let denial = facts.denial.clone().expect("denied license");
    assert_eq!(
        denial.reasons,
        vec![
            LicenseDenialReason {
                rejection_reason: "RequesterEligibility".into(),
                validation_type: "Membership".into(),
            },
            LicenseDenialReason {
                rejection_reason: "CustomerThrottled".into(),
                validation_type: "Ownership".into(),
            },
        ]
    );
    assert!(denial.is_throttled());
    assert_eq!(
        choose_acquisition_strategy(&facts),
        AcquisitionStrategy::ProviderProtocolFailed
    );
}

#[test]
fn granted_license_has_no_denial() {
    let facts = license_facts_from_value(&json!({
        "content_license": {
            "status_code": "Granted",
            "content_metadata": {"content_url": {"offline_url": "https://cdn.example.test/book.aax"}},
            "license_response": "fake"
        }
    }));
    assert_eq!(facts.denial, None);
}

#[test]
fn license_facts_choose_acquisition_strategy() {
    let import_ready = LicenseFacts {
        content_url: Some("https://cdn.example.test/book.m4b".to_string()),
        content_kind: MaterializedSourceKind::ImportReadyM4b,
        media_container: MediaContainerKind::M4b,
        media_protection: MediaProtectionKind::None,
        decryption_material_present: false,
        drm_kind: None,
        supplemental_pdf_url: None,
        denial: None,
    };
    assert_eq!(
        choose_acquisition_strategy(&import_ready),
        AcquisitionStrategy::DownloadImportReady
    );

    let encrypted_aax = LicenseFacts {
        content_url: Some("https://cdn.example.test/book.aax".to_string()),
        content_kind: MaterializedSourceKind::EncryptedAax,
        media_container: MediaContainerKind::Aax,
        media_protection: MediaProtectionKind::AudibleAax,
        decryption_material_present: true,
        drm_kind: Some("Mpeg".to_string()),
        supplemental_pdf_url: None,
        denial: None,
    };
    assert_eq!(
        choose_acquisition_strategy(&encrypted_aax),
        AcquisitionStrategy::DownloadThenDecryptAax
    );

    let protected_without_decrypt_material = LicenseFacts {
        content_url: Some("https://cdn.example.test/book.aaxc".to_string()),
        content_kind: MaterializedSourceKind::EncryptedAaxc,
        media_container: MediaContainerKind::Aaxc,
        media_protection: MediaProtectionKind::Widevine,
        decryption_material_present: false,
        drm_kind: Some("Widevine".to_string()),
        supplemental_pdf_url: None,
        denial: None,
    };
    assert_eq!(
        choose_acquisition_strategy(&protected_without_decrypt_material),
        AcquisitionStrategy::ProtectedUnsupported
    );

    let missing_url = LicenseFacts {
        content_url: None,
        content_kind: MaterializedSourceKind::Unsupported,
        media_container: MediaContainerKind::Unknown,
        media_protection: MediaProtectionKind::None,
        decryption_material_present: false,
        drm_kind: None,
        supplemental_pdf_url: None,
        denial: None,
    };
    assert_eq!(
        choose_acquisition_strategy(&missing_url),
        AcquisitionStrategy::ProviderProtocolFailed
    );
}

#[test]
fn dash_license_facts_choose_dash_materializer_lane() {
    let response = json!({
        "content_license": {
            "content_url": "https://cdn.example.test/manifest.mpd",
            "drm_type": "Mpeg",
            "license": { "key_id": "fake-key-id", "key": "fake-key" }
        }
    });

    let facts = license_facts_from_value(&response);

    assert_eq!(facts.media_container, MediaContainerKind::Dash);
    assert_eq!(facts.media_protection, MediaProtectionKind::AudibleDash);
    assert_eq!(
        choose_acquisition_strategy(&facts),
        AcquisitionStrategy::DownloadThenDecryptDash
    );
}
