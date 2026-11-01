use super::*;
use crate::errors::AppError;
use crate::remote_source::providers::audible::audio_download::download_status_failure;
use crate::remote_source::scoped_output::{partial_sibling, rollback_committed_file};
use secrecy::ExposeSecret;
use serde_json::json;
use std::path::Path;

async fn job_after_license_reply(status: u16) -> AcquisitionJob {
    job_after_license_body(status, r#"{"message":"Refused"}"#).await
}

async fn job_after_license_body(status: u16, body: &str) -> AcquisitionJob {
    use crate::remote_source::providers::audible::http::stub::{signing_auth, stub_api};
    let (api, server) = stub_api(vec![
        (200, r#"{"item":{"title":"Book"}}"#.to_string()),
        (status, body.to_string()),
    ])
    .await;
    let root = tempfile::TempDir::new().expect("temp root");
    let mut job = AcquisitionJob {
        job_id: "job-1".into(),
        provider_id: ProviderId::Audible,
        status: RemoteAcquisitionStatus::Acquiring,
        progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
        handoff: None,
    };

    let outcome = acquire_one(
        &api,
        &signing_auth("us"),
        &AaxcleanMaterializer::new_for_helper(None),
        TitleAcquisitionRequest {
            title_id: "B000000001",
            include_pdf: false,
            job_id: "job-1",
            job_dir: root.path(),
            license_decrypt_context: None,
            progress_context: TitleProgressContext {
                title_id: "B000000001",
                item_index: 1,
                total_items: 1,
            },
        },
        &mut |_| {},
        &|| false,
    )
    .await;
    record_title_outcome(&mut job, "B000000001", outcome).expect("not cancelled");
    let requests = server.await.expect("stub");
    assert!(
        requests[1].starts_with("POST /1.0/content/B000000001/licenserequest "),
        "{requests:?}"
    );
    assert_eq!(job.diagnostics.len(), 1);
    job
}

#[tokio::test]
async fn a_license_401_says_sign_in_is_needed() {
    let job = job_after_license_reply(401).await;

    assert_eq!(
        job.diagnostics[0].kind,
        RemoteAcquisitionFailureKind::AuthRequired
    );
}

#[tokio::test]
async fn a_license_403_fails_only_that_title() {
    let job = job_after_license_reply(403).await;

    assert_eq!(
        job.diagnostics[0].kind,
        RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed
    );
    assert!(
        job.diagnostics[0].message.contains("HTTP 403"),
        "{:?}",
        job.diagnostics[0]
    );
}

#[tokio::test]
async fn a_throttled_license_says_audible_is_limiting_downloads() {
    let job = job_after_license_body(
        200,
        r#"{"content_license":{"status_code":"Denied","drm_type":"Adrm","license_denial_reasons":[{"rejectionReason":"RequesterEligibility","validationType":"Membership"},{"rejectionReason":"CustomerThrottled","validationType":"Ownership"}]}}"#,
    )
    .await;

    assert_eq!(
        job.diagnostics[0].kind,
        RemoteAcquisitionFailureKind::LicenseDenied
    );
    assert!(
        job.diagnostics[0]
            .message
            .contains("temporarily limiting downloads"),
        "{:?}",
        job.diagnostics[0]
    );
}

#[tokio::test]
async fn a_denied_license_names_its_reasons() {
    let job = job_after_license_body(
        200,
        r#"{"content_license":{"status_code":"Denied","license_denial_reasons":[{"rejectionReason":"RequesterEligibility","validationType":"Membership"}]}}"#,
    )
    .await;

    assert_eq!(
        job.diagnostics[0].kind,
        RemoteAcquisitionFailureKind::LicenseDenied
    );
    assert!(
        job.diagnostics[0]
            .message
            .contains("RequesterEligibility (Membership)"),
        "{:?}",
        job.diagnostics[0]
    );
}

#[test]
fn a_title_without_usable_audio_reports_exactly_its_own_diagnostic() {
    let mut job = AcquisitionJob {
        job_id: "job-1".into(),
        provider_id: ProviderId::Audible,
        status: RemoteAcquisitionStatus::Acquiring,
        progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
        handoff: None,
    };
    let specific = RemoteSourceDiagnostic {
        kind: RemoteAcquisitionFailureKind::ProtectedUnsupported,
        title_id: Some("B000000001".into()),
        message: "Audible returned Dash; not supported.".into(),
    };

    record_title_outcome(
        &mut job,
        "B000000001",
        Ok(AcquiredTitle {
            file: None,
            assets: Vec::new(),
            diagnostics: vec![specific.clone()],
        }),
    )
    .expect("record outcome");

    assert_eq!(job.diagnostics, vec![specific]);
    assert!(job.materialized_files.is_empty());
}

#[test]
fn generated_staging_path_does_not_use_provider_title_id() {
    let root = Path::new("/tmp/abb-remote-job");
    let unsafe_title_id = "../../account-title";

    let path = generated_staging_path(root, "m4b");

    assert!(path.starts_with(root));
    assert!(!path.to_string_lossy().contains(unsafe_title_id));
    assert_eq!(
        path.extension().and_then(|value| value.to_str()),
        Some("m4b")
    );
}

#[test]
fn item_staged_paths_do_not_use_provider_title_id() {
    let root = tempfile::TempDir::new().expect("temp root");
    let item_dir = staging::create_item_dir(root.path(), "item-1").expect("item dir");
    let unsafe_title_id = "../../account-title";

    let protected_path =
        staged_protected_source_path(&item_dir, AcquisitionStrategy::DownloadThenDecryptAaxc);
    let materialized_path =
        staged_materialized_path(&item_dir, Some("../../account:title,?"), unsafe_title_id);

    assert!(protected_path.starts_with(root.path()));
    assert!(materialized_path.starts_with(root.path()));
    assert!(!protected_path.to_string_lossy().contains(unsafe_title_id));
    assert!(!materialized_path
        .to_string_lossy()
        .contains(unsafe_title_id));
    assert_eq!(
        protected_path.file_name().and_then(|value| value.to_str()),
        Some("source.aaxc")
    );
    assert_eq!(
        materialized_path
            .file_name()
            .and_then(|value| value.to_str()),
        Some("account - title.m4b")
    );
}

#[test]
fn staged_materialized_path_uses_sanitized_remote_title() {
    let root = tempfile::TempDir::new().expect("temp root");
    let item_dir = staging::create_item_dir(root.path(), "item-1").expect("item dir");

    let materialized_path = staged_materialized_path(
        &item_dir,
        Some("Secure Love: Create a Relationship That Lasts a Lifetime"),
        "B000000001",
    );

    assert!(materialized_path.starts_with(root.path()));
    assert_eq!(
        materialized_path
            .file_name()
            .and_then(|value| value.to_str()),
        Some("Secure Love - Create a Relationship That Lasts a Lifetime.m4b")
    );
}

#[test]
fn staged_materialized_path_falls_back_to_title_ref_for_empty_title() {
    let root = tempfile::TempDir::new().expect("temp root");
    let item_dir = staging::create_item_dir(root.path(), "item-1").expect("item dir");
    let title_id = "../../account-title";

    let materialized_path = staged_materialized_path(&item_dir, Some("../"), title_id);
    let expected = format!("Audible {}.m4b", title_ref(title_id));

    assert!(materialized_path.starts_with(root.path()));
    assert_eq!(
        materialized_path
            .file_name()
            .and_then(|value| value.to_str()),
        Some(expected.as_str())
    );
    assert!(!materialized_path.to_string_lossy().contains(title_id));
}

#[test]
fn sha256_file_streams_expected_digest() {
    let root = tempfile::TempDir::new().expect("temp root");
    let path = root.path().join("book.m4b");
    let bytes = b"materialized-audio";
    std::fs::write(&path, bytes).expect("write source");

    assert_eq!(
        paths::sha256_file(&path).expect("hash file"),
        abb_media_core::sha256_hex(bytes)
    );
}

#[test]
fn cancellation_guard_returns_cancellation_error() {
    let error = ensure_not_cancelled(&|| true).expect_err("cancelled");

    assert!(matches!(error, AppError::Cancellation(_)));
    assert!(error
        .to_string()
        .contains("Remote source acquisition was cancelled"));
}

#[test]
fn cancellation_guard_allows_active_acquisition() {
    ensure_not_cancelled(&|| false).expect("active acquisition");
}

#[test]
fn encrypted_download_is_not_reported_as_materialized_without_decryption() {
    let root = tempfile::TempDir::new().expect("temp root");
    let encrypted = root.path().join("book.aax");
    std::fs::write(&encrypted, b"encrypted").expect("write encrypted fixture");

    let result = validation::materialized_file_from_path("B000000001", &encrypted);

    let error = result.expect_err("encrypted download must not be import-ready");
    assert!(error.to_string().contains("requires Audible decryption"));
}

#[test]
fn dash_lane_is_reported_unsupported_not_deferred() {
    let lane = LicenseLane {
        content_url: "https://cdn.example.test/manifest.mpd".to_string(),
        strategy: AcquisitionStrategy::DownloadThenDecryptDash,
        decryption_material: None,
        supplemental_pdf_url: None,
        denial: None,
    };

    let acquired = unsupported_result_for_unmaterializable_lane("B000000001", "job-1", &lane)
        .expect("dash unsupported");

    assert!(acquired.file.is_none());
    assert_eq!(
        acquired.diagnostics[0].kind,
        RemoteAcquisitionFailureKind::ProtectedUnsupported
    );
    assert!(acquired.diagnostics[0]
        .message
        .contains("Dash/Widevine materialization is not supported"));
}

#[test]
fn rollback_committed_file_removes_committed_staged_output() {
    let root = tempfile::TempDir::new().expect("temp root");
    let final_path = root.path().join("book.aax");
    std::fs::write(&final_path, b"encrypted").expect("write final");

    rollback_committed_file(&final_path).expect("rollback committed file");

    assert!(!final_path.exists());
    assert!(!partial_sibling(&final_path).exists());
}

#[test]
fn typed_acquisition_errors_map_to_diagnostics_without_message_matching() {
    let error = AudibleAcquisitionError::download(download_status_failure(403));

    assert_eq!(error.kind(), RemoteAcquisitionFailureKind::DownloadFailed);
    let diagnostic = error.into_diagnostic(Some("B000000001".to_string()));

    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::DownloadFailed
    );
    assert_eq!(diagnostic.title_id.as_deref(), Some("B000000001"));
    assert!(!diagnostic.message.contains("token"));
    assert!(!diagnostic.message.contains("license"));
}

#[test]
fn helper_lane_follows_voucher_key_shape_not_content_url_extension() {
    use crate::remote_source::materializer::{AaxcleanLane, AaxcleanSecret};
    use crate::remote_source::providers::audible::materialization::helper_material_from_audible_material;
    use abb_audible_core::audible_decryption_material_from_license;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

    let response = json!({
        "content_license": {
            "content_url": "https://cdn.example.test/book.aax",
            "drm_type": "Mpeg",
            "voucher": {
                "key": URL_SAFE_NO_PAD.encode([
                    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                    0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10
                ]),
                "iv": URL_SAFE_NO_PAD.encode([
                    0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09,
                    0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01
                ])
            }
        }
    });
    let material = audible_decryption_material_from_license(
        &response,
        AcquisitionStrategy::DownloadThenDecryptAax,
        "B000000001",
        None,
    )
    .expect("aaxc-shaped material");
    let lane = LicenseLane {
        content_url: "https://cdn.example.test/book.aax".to_string(),
        strategy: AcquisitionStrategy::DownloadThenDecryptAax,
        decryption_material: Some(material),
        supplemental_pdf_url: None,
        denial: None,
    };

    let (helper_lane, secret) =
        helper_material_from_audible_material(&lane).expect("helper material");

    assert_eq!(helper_lane, AaxcleanLane::Aaxc);
    match secret {
        AaxcleanSecret::Aaxc { key_hex, iv_hex } => {
            assert_eq!(key_hex.expose_secret(), "0102030405060708090a0b0c0d0e0f10");
            assert_eq!(iv_hex.expose_secret(), "100f0e0d0c0b0a090807060504030201");
        }
        AaxcleanSecret::Aax { .. } => panic!("unexpected aax helper secret"),
    }
}
