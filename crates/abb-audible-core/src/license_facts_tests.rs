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
fn content_url_extension_without_drm_picks_import_ready_or_protocol_failure() {
    let ready = license_facts_from_value(&json!({
        "content_url": "https://cdn.example.test/book.m4a?Signature=x"
    }));
    assert_eq!(
        choose_acquisition_strategy(&ready),
        AcquisitionStrategy::DownloadImportReady
    );

    for url in [
        "https://cdn.example.test/book.pdf",
        "https://cdn.example.test/book.bin",
    ] {
        let facts = license_facts_from_value(&json!({ "content_url": url }));
        assert_eq!(
            choose_acquisition_strategy(&facts),
            AcquisitionStrategy::ProviderProtocolFailed,
            "{url}"
        );
    }
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

    assert_eq!(facts.media_container, MediaContainerKind::Aaxc);
    assert_eq!(facts.media_protection, MediaProtectionKind::AudibleAaxc);
    assert!(facts.content_url.is_some());
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
        status_code: None,
        content_url_path: None,
        content_format: None,
        license_response_present: false,
        asin_present: false,
        content_url: Some("https://cdn.example.test/book.m4b".to_string()),
        media_container: MediaContainerKind::M4b,
        media_protection: MediaProtectionKind::None,
        drm_kind: None,
        supplemental_pdf_url: None,
        denial: None,
    };
    assert_eq!(
        choose_acquisition_strategy(&import_ready),
        AcquisitionStrategy::DownloadImportReady
    );

    let encrypted_aax = LicenseFacts {
        status_code: None,
        content_url_path: None,
        content_format: None,
        license_response_present: false,
        asin_present: false,
        content_url: Some("https://cdn.example.test/book.aax".to_string()),
        media_container: MediaContainerKind::Aax,
        media_protection: MediaProtectionKind::AudibleAax,
        drm_kind: Some("Mpeg".to_string()),
        supplemental_pdf_url: None,
        denial: None,
    };
    assert_eq!(
        choose_acquisition_strategy(&encrypted_aax),
        AcquisitionStrategy::DownloadThenDecryptAax
    );

    let widevine_protected = LicenseFacts {
        status_code: None,
        content_url_path: None,
        content_format: None,
        license_response_present: false,
        asin_present: false,
        content_url: Some("https://cdn.example.test/book.aaxc".to_string()),
        media_container: MediaContainerKind::Aaxc,
        media_protection: MediaProtectionKind::Widevine,
        drm_kind: Some("Widevine".to_string()),
        supplemental_pdf_url: None,
        denial: None,
    };
    assert_eq!(
        choose_acquisition_strategy(&widevine_protected),
        AcquisitionStrategy::ProtectedUnsupported
    );

    let missing_url = LicenseFacts {
        status_code: None,
        content_url_path: None,
        content_format: None,
        license_response_present: false,
        asin_present: false,
        content_url: None,
        media_container: MediaContainerKind::Unknown,
        media_protection: MediaProtectionKind::None,
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

#[test]
fn every_alias_of_a_reply_fact_is_read() {
    for (pdf_key, license_key, asin_key) in [
        ("pdf_url", "license_response", "asin"),
        ("pdfUrl", "licenseResponse", "Asin"),
    ] {
        let facts = license_facts_from_value(&json!({
            "content_license": { asin_key: "B000000001", license_key: "blob" },
            "details": { pdf_key: "https://cdn.example.test/book.pdf" }
        }));
        assert_eq!(
            facts.supplemental_pdf_url.as_deref(),
            Some("https://cdn.example.test/book.pdf"),
            "{pdf_key}"
        );
        assert!(facts.license_response_present, "{license_key}");
        assert!(facts.asin_present, "{asin_key}");
    }
    for key in [
        "content_url",
        "contentUrl",
        "download_url",
        "downloadUrl",
        "offline_url",
        "offlineUrl",
        "url",
    ] {
        let facts = license_facts_from_value(
            &json!({ "a": [{ key: "https://cdn.example.test/book.aax" }] }),
        );
        assert_eq!(
            facts.content_url.as_deref(),
            Some("https://cdn.example.test/book.aax"),
            "{key}"
        );
    }
}

#[test]
fn the_facts_locate_the_content_url_and_read_the_status_and_format() {
    let facts = license_facts_from_value(&json!({
        "content_license": {
            "status_code": "Granted",
            "content_metadata": {
                "content_reference": {"content_format": "AAX_44_128"},
                "content_url": {"offline_url": "https://cdn.example.test/book.aax"}
            }
        }
    }));

    assert_eq!(facts.status_code.as_deref(), Some("Granted"));
    assert_eq!(facts.content_format.as_deref(), Some("AAX_44_128"));
    let key = |name: &str| OwnedPathStep::Key(name.to_string());
    assert_eq!(
        facts.content_url_path,
        Some(vec![
            key("content_license"),
            key("content_metadata"),
            key("content_url"),
            key("offline_url")
        ])
    );
}
