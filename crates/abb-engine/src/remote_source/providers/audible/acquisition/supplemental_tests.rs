use super::super::TitleProgressContext;
use super::*;
use crate::remote_source::RemoteAcquisitionFailureKind;
use audible_api::auth::localization;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

fn fixture_auth_without_pdf_cookies() -> Auth {
    Auth {
        locale: localization::find_by_country_code(super::super::super::COUNTRY_CODE)
            .expect("locale"),
        device_registration: audible_api::auth::register::Registration {
            device_serial: "device-serial".to_string(),
            client_id: "client-id".to_string(),
            adp_token: "adp-token".to_string(),
            device_private_key: "device-private-key".to_string(),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            expires: 0,
            website_cookies: HashMap::new(),
            store_authentication_cookie: String::new(),
            device_info: json!({ "device_type": super::super::super::AUDIBLE_IOS_DEVICE_TYPE }),
            customer_info: json!({ "user_id": "account-1" }),
        },
        authorization_code: "authorization-code".to_string(),
        code_verifier: "code-verifier".to_string(),
    }
}

fn materialized_source_file(path: std::path::PathBuf) -> MaterializedSourceFile {
    MaterializedSourceFile {
        input_id: "input-1".to_string(),
        title_id: "B000000001".to_string(),
        path,
        size_bytes: b"audio-bytes".len() as u64,
        sha256: abb_media_core::sha256_hex(b"audio-bytes"),
    }
}

fn test_title_ctx<'a>(
    job_dir: &'a Path,
    progress_context: TitleProgressContext<'a>,
) -> TitleAcquisitionCtx<'a> {
    TitleAcquisitionCtx {
        job_id: "job-1",
        title_id: "B000000001",
        item_id: "item-1",
        item_dir: job_dir,
        progress_context,
    }
}

fn test_progress_context() -> TitleProgressContext<'static> {
    TitleProgressContext {
        title_id: "B000000001",
        item_index: 1,
        total_items: 1,
    }
}

#[tokio::test]
async fn requested_advertised_supplemental_pdf_failure_fails_the_title() {
    let root = tempfile::TempDir::new().expect("temp root");
    let auth = fixture_auth_without_pdf_cookies();
    let file = materialized_source_file(root.path().join("Book.m4b"));
    let ctx = test_title_ctx(root.path(), test_progress_context());

    let error = download_if_requested(
        SupplementalPdfAcquisitionRequest {
            auth: &auth,
            file: &file,
            title_name: Some("Book"),
            include_pdf: true,
            api_pdf_hint_present: true,
            ctx,
        },
        &|| false,
    )
    .await
    .expect_err("advertised requested Supplemental PDF failure should fail title");

    let diagnostic = error.into_diagnostic(Some("B000000001".to_string()));
    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::SupplementalPdfFailed
    );
    assert!(diagnostic
        .message
        .contains("requested Supplemental PDF is required"));
    assert!(!diagnostic.message.contains("https://"));
    assert!(!diagnostic.message.contains("B000000001"));
}

#[tokio::test]
async fn absent_or_non_requested_supplemental_pdf_does_not_block_audio_handoff() {
    let root = tempfile::TempDir::new().expect("temp root");
    let auth = fixture_auth_without_pdf_cookies();
    let first_audio = root.path().join("Absent.pdf-hint.m4b");
    std::fs::write(&first_audio, b"audio-bytes").expect("write audio");
    let file = materialized_source_file(first_audio.clone());
    let ctx = test_title_ctx(root.path(), test_progress_context());

    let (assets, diagnostics) = download_if_requested(
        SupplementalPdfAcquisitionRequest {
            auth: &auth,
            file: &file,
            title_name: Some("Book"),
            include_pdf: true,
            api_pdf_hint_present: false,
            ctx,
        },
        &|| false,
    )
    .await
    .expect("requested but absent Supplemental PDF should not fail");

    assert!(assets.is_empty());
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (&diagnostic.kind, diagnostic.title_id.as_deref()))
            .collect::<Vec<_>>(),
        vec![(
            &RemoteAcquisitionFailureKind::SupplementalPdfUnavailable,
            Some(file.title_id.as_str())
        )],
        "the user learns the requested PDF was not offered"
    );
    assert!(
        first_audio.exists(),
        "audio handoff must remain when no Supplemental PDF was advertised"
    );

    let second_audio = root.path().join("Not requested.m4b");
    std::fs::write(&second_audio, b"audio-bytes").expect("write audio");
    let file = materialized_source_file(second_audio.clone());
    let ctx = test_title_ctx(root.path(), test_progress_context());

    let (assets, diagnostics) = download_if_requested(
        SupplementalPdfAcquisitionRequest {
            auth: &auth,
            file: &file,
            title_name: Some("Book"),
            include_pdf: false,
            api_pdf_hint_present: true,
            ctx,
        },
        &|| false,
    )
    .await
    .expect("advertised but non-requested Supplemental PDF should not fail");

    assert!(assets.is_empty());
    assert!(diagnostics.is_empty());
    assert!(
        second_audio.exists(),
        "audio handoff must remain when Supplemental PDF was not requested"
    );
}

#[test]
fn supplemental_pdf_hint_needs_a_request_and_either_pdf_url() {
    for (title_url, license_url) in [
        (None, None),
        (Some(()), None),
        (None, Some(())),
        (Some(()), Some(())),
    ] {
        let details = AudibleTitleDetails {
            title: Some("Remote Book".to_string()),
            supplemental_pdf_url: title_url
                .map(|()| "https://metadata.example.test/book.pdf".to_string()),
        };
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/book.aax".to_string(),
            strategy: abb_remote_source_core::AcquisitionStrategy::DownloadThenDecryptAax,
            decryption_material: None,
            supplemental_pdf_url: license_url
                .map(|()| "https://license.example.test/book.pdf".to_string()),
            denial: None,
        };

        assert_eq!(
            hint_present_for_acquisition(true, &details, &lane),
            title_url.is_some() || license_url.is_some()
        );
        assert!(!hint_present_for_acquisition(false, &details, &lane));
    }
}
