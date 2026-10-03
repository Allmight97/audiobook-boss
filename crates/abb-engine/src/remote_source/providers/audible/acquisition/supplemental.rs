use super::TitleAcquisitionCtx;
use super::{ensure_not_cancelled, remote_acquisition_cancelled, AudibleAcquisitionResult};
use crate::errors::AppError;
use crate::remote_source::providers::audible::diagnostics::AudibleAcquisitionError;
use crate::remote_source::providers::audible::license::{AudibleTitleDetails, LicenseLane};
use crate::remote_source::providers::audible::supplemental_pdf::{
    download_supplemental_pdf, log_supplemental_pdf_failed, supplemental_pdf_failure_message,
    SupplementalPdfFailure, SupplementalPdfRequest,
};
use crate::remote_source::{MaterializedSourceFile, RemoteSourceDiagnostic, SupplementalAsset};
use abb_audible_core::supplemental_pdf_display_file_name;
use audible_api::auth::Auth;

pub(super) struct SupplementalPdfAcquisitionRequest<'a> {
    pub(super) auth: &'a Auth,
    pub(super) file: &'a MaterializedSourceFile,
    pub(super) title_name: Option<&'a str>,
    pub(super) include_pdf: bool,
    pub(super) api_pdf_hint_present: bool,
    pub(super) ctx: TitleAcquisitionCtx<'a>,
}

pub(super) fn hint_present_for_acquisition(
    include_pdf: bool,
    title_details: &AudibleTitleDetails,
    lane: &LicenseLane,
) -> bool {
    include_pdf
        && (title_details.supplemental_pdf_url.is_some() || lane.supplemental_pdf_url.is_some())
}

fn requested_pdf_is_required(include_pdf: bool, api_pdf_hint_present: bool) -> bool {
    include_pdf && api_pdf_hint_present
}

pub(super) async fn download_if_requested(
    request: SupplementalPdfAcquisitionRequest<'_>,
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<(Vec<SupplementalAsset>, Vec<RemoteSourceDiagnostic>)> {
    let SupplementalPdfAcquisitionRequest {
        auth,
        file,
        title_name,
        include_pdf,
        api_pdf_hint_present,
        ctx,
    } = request;
    let TitleAcquisitionCtx {
        job_id,
        title_id,
        item_dir: job_dir,
        ..
    } = ctx;
    let mut assets = Vec::new();
    let diagnostics = Vec::new();
    if !include_pdf {
        ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
        return Ok((assets, diagnostics));
    }

    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    if !requested_pdf_is_required(include_pdf, api_pdf_hint_present) {
        return Ok((assets, diagnostics));
    }

    let supplemental_file_name = supplemental_pdf_display_file_name(title_name, title_id);
    let committed_pdf = match download_supplemental_pdf(
        SupplementalPdfRequest {
            auth,
            title_id,
            job_id,
            input_id: &file.input_id,
            file_name: &supplemental_file_name,
            api_pdf_hint_present,
            job_dir,
        },
        is_cancelled,
    )
    .await
    {
        Ok((asset, committed)) => {
            assets.push(asset);
            Some(committed)
        }
        Err(failure) if failure.category == "cancelled" => {
            return Err(AudibleAcquisitionError::cancellation(
                remote_acquisition_cancelled(),
            ));
        }
        Err(failure) => {
            log_supplemental_pdf_failed(job_id, title_id, failure);
            return Err(AudibleAcquisitionError::supplemental_pdf(
                AppError::General(required_failure_message(failure)),
            ));
        }
    };
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    if let Some(committed) = committed_pdf {
        committed.permanent();
    }
    Ok((assets, diagnostics))
}

fn required_failure_message(failure: SupplementalPdfFailure) -> String {
    format!(
        "{} The audiobook was not imported because the requested Supplemental PDF is required for this Audible title.",
        supplemental_pdf_failure_message(failure)
    )
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn requested_supplemental_pdf_is_required_only_when_audible_advertises_one() {
        assert!(requested_pdf_is_required(true, true));
        assert!(!requested_pdf_is_required(true, false));
        assert!(!requested_pdf_is_required(false, true));
        assert!(!requested_pdf_is_required(false, false));
    }

    #[test]
    fn required_supplemental_pdf_failure_message_keeps_provider_details_redacted() {
        let failure = SupplementalPdfFailure {
            category: "status",
            status: Some(reqwest::StatusCode::FORBIDDEN),
        };
        let message = required_failure_message(failure);

        assert!(message.contains("requested Supplemental PDF is required"));
        assert!(!message.contains("B000000001"));
        assert!(!message.contains("https://"));
        assert!(!message.contains("403"));
    }

    #[tokio::test]
    async fn requested_advertised_supplemental_pdf_failure_blocks_audio_handoff() {
        use crate::remote_source::scoped_output::ProvisionalCommittedFile;

        let root = tempfile::TempDir::new().expect("temp root");
        let auth = fixture_auth_without_pdf_cookies();
        let audio_path = root.path().join("Book.m4b");
        std::fs::write(&audio_path, b"audio-bytes").expect("write audio");
        let ctx = test_title_ctx(root.path(), test_progress_context());
        let error = {
            let committed_audio = ProvisionalCommittedFile::new(audio_path.clone());
            let file = materialized_source_file(committed_audio.path().to_path_buf());
            download_if_requested(
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
            .expect_err("advertised requested Supplemental PDF failure should fail title")
        };

        assert_eq!(
            error.kind(),
            RemoteAcquisitionFailureKind::SupplementalPdfFailed
        );
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
        assert!(
            !audio_path.exists(),
            "provisional audio guard should clean committed audio when required PDF fails"
        );
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
        assert!(diagnostics.is_empty());
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
    fn supplemental_pdf_hint_uses_title_or_license_presence_without_exposing_url() {
        let details = AudibleTitleDetails {
            title: Some("Remote Book".to_string()),
            supplemental_pdf_url: None,
        };
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/book.aax".to_string(),
            strategy: abb_remote_source_core::AcquisitionStrategy::DownloadThenDecryptAax,
            decryption_material: None,
            supplemental_pdf_url: Some("https://cdn.example.test/book.pdf".to_string()),
        };

        assert!(hint_present_for_acquisition(true, &details, &lane));
        assert!(!hint_present_for_acquisition(false, &details, &lane));
    }

    #[test]
    fn supplemental_pdf_hint_treats_api_pdf_url_as_presence_not_download_candidate() {
        let details = AudibleTitleDetails {
            title: Some("Remote Book".to_string()),
            supplemental_pdf_url: Some("https://metadata.example.test/book.pdf".to_string()),
        };
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/book.aax".to_string(),
            strategy: abb_remote_source_core::AcquisitionStrategy::DownloadThenDecryptAax,
            decryption_material: None,
            supplemental_pdf_url: Some("https://license.example.test/book.pdf".to_string()),
        };

        assert!(hint_present_for_acquisition(true, &details, &lane));
    }
}
