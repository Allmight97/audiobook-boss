use std::fs;
use std::io::Read;
use std::path::Path;

use abb_audible_core::{
    download_extension_for_strategy, remote_materialized_filename_stem,
    supplemental_pdf_display_file_name, title_ref, AudibleLicenseDecryptContext,
};
use abb_remote_source_core::{
    acquisition_progress, acquisition_progress_for_current_title, AcquisitionProgress,
    AcquisitionStage, AcquisitionStrategy, MaterializedSourceKind,
};
use audible_api::api::Client as AudibleClient;
use audible_api::auth::Auth;
use sha2::{Digest, Sha256};

use super::audio_download::{cleanup_download_artifacts, download_audio};
use super::diagnostics::AudibleAcquisitionError;
use super::license::{
    license_decrypt_context_from_auth, lookup_title_details, provider_protocol_lane_message,
    request_license_lane, strategy_label, AudibleTitleDetails, LicenseLane,
};
use super::materialization::materialize_protected_download;
use super::supplemental_pdf::{
    download_supplemental_pdf, log_supplemental_pdf_failed, supplemental_pdf_failure_message,
    SupplementalPdfRequest,
};
use super::{auth_from_vault, client_from_auth};
use crate::audio;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::remote_source::materializer::AaxcleanMaterializer;
use crate::remote_source::staging;
use crate::remote_source::vault::SecretVault;
use crate::remote_source::{
    AcquisitionJob, AcquisitionPlan, MaterializedSourceFile, ProviderId,
    RemoteAcquisitionFailureKind, RemoteAcquisitionStatus, RemoteSourceDiagnostic,
    SupplementalAsset,
};

pub(super) struct AcquiredTitle {
    pub(super) file: Option<MaterializedSourceFile>,
    pub(super) assets: Vec<SupplementalAsset>,
    pub(super) diagnostics: Vec<RemoteSourceDiagnostic>,
}

type AudibleAcquisitionResult<T> = std::result::Result<T, AudibleAcquisitionError>;

#[derive(Clone, Copy)]
pub(super) struct TitleProgressContext<'a> {
    pub(super) title_id: &'a str,
    pub(super) item_index: u32,
    pub(super) total_items: u32,
}

struct TitleAcquisitionRequest<'a> {
    title_id: &'a str,
    include_pdf: bool,
    job_id: &'a str,
    job_dir: &'a Path,
    license_decrypt_context: Option<&'a AudibleLicenseDecryptContext>,
    progress_context: TitleProgressContext<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct TitleAcquisitionCtx<'a> {
    pub(super) job_id: &'a str,
    pub(super) title_id: &'a str,
    pub(super) item_id: &'a str,
    pub(super) item_dir: &'a Path,
    pub(super) progress_context: TitleProgressContext<'a>,
}

pub(super) async fn acquire(
    vault: &dyn SecretVault,
    materializer: &AaxcleanMaterializer,
    plan: &AcquisitionPlan,
    job_id: &str,
    job_dir: &Path,
    mut progress: impl FnMut(AcquisitionProgress),
    is_cancelled: impl Fn() -> bool,
) -> Result<AcquisitionJob> {
    ensure_not_cancelled(&is_cancelled)?;
    let auth = auth_from_vault(vault)?;
    let license_decrypt_context = license_decrypt_context_from_auth(&auth);
    let client = client_from_auth(auth.clone())?;
    let mut job = AcquisitionJob {
        job_id: job_id.to_string(),
        provider_id: ProviderId::Audible,
        status: RemoteAcquisitionStatus::Acquiring,
        progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
    };

    let total_items = u32::try_from(plan.selections.len()).unwrap_or(u32::MAX);
    for (selection_index, selection) in plan.selections.iter().enumerate() {
        ensure_not_cancelled(&is_cancelled)?;
        let request = TitleAcquisitionRequest {
            title_id: &selection.title_id,
            include_pdf: selection.include_supplemental_pdf,
            job_id,
            job_dir,
            license_decrypt_context: license_decrypt_context.as_ref(),
            progress_context: TitleProgressContext {
                title_id: &selection.title_id,
                item_index: u32::try_from(selection_index + 1).unwrap_or(u32::MAX),
                total_items,
            },
        };
        match acquire_one(
            &client,
            &auth,
            materializer,
            request,
            &mut progress,
            &is_cancelled,
        )
        .await
        {
            Ok(AcquiredTitle {
                file: Some(file),
                assets,
                diagnostics,
            }) => {
                job.materialized_files.push(file);
                job.supplemental_assets.extend(assets);
                job.diagnostics.extend(diagnostics);
            }
            Ok(AcquiredTitle {
                file: None,
                assets,
                diagnostics,
            }) => {
                job.supplemental_assets.extend(assets);
                job.diagnostics.extend(diagnostics);
                job.diagnostics.push(RemoteSourceDiagnostic {
                    kind: RemoteAcquisitionFailureKind::ProtectedUnsupported,
                    title_id: Some(selection.title_id.clone()),
                    message: "Audible returned provider data, but no import-compatible audio was materialized.".to_string(),
                });
            }
            Err(error) => {
                if error.is_cancellation() {
                    return Err(error.into_app_error());
                }
                job.diagnostics
                    .push(error.into_diagnostic(Some(selection.title_id.clone())));
            }
        }
    }

    job.status = if job.materialized_files.is_empty() {
        RemoteAcquisitionStatus::Failed
    } else {
        RemoteAcquisitionStatus::Validated
    };
    job.progress = if job.status == RemoteAcquisitionStatus::Failed {
        acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None)
    } else {
        acquisition_progress(AcquisitionStage::ImportHandoff, Some(1.0), None, None)
    };
    Ok(job)
}

pub(super) fn remote_acquisition_cancelled() -> AppError {
    AppError::Cancellation("Remote source acquisition was cancelled.".to_string())
}

pub(super) fn ensure_not_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<()> {
    if is_cancelled() {
        return Err(remote_acquisition_cancelled());
    }
    Ok(())
}

pub(super) fn title_progress(
    context: TitleProgressContext<'_>,
    stage: AcquisitionStage,
    fraction: Option<f32>,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
) -> AcquisitionProgress {
    with_title_progress(
        acquisition_progress(stage, fraction, bytes_downloaded, bytes_total),
        context,
    )
}

pub(super) fn with_title_progress(
    progress: AcquisitionProgress,
    context: TitleProgressContext<'_>,
) -> AcquisitionProgress {
    acquisition_progress_for_current_title(
        progress,
        context.title_id,
        context.item_index,
        context.total_items,
    )
}

async fn acquire_one(
    client: &AudibleClient,
    auth: &Auth,
    materializer: &AaxcleanMaterializer,
    request: TitleAcquisitionRequest<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<AcquiredTitle> {
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    let TitleAcquisitionRequest {
        title_id,
        include_pdf,
        job_id,
        job_dir,
        license_decrypt_context,
        progress_context,
    } = request;
    let item_id = uuid::Uuid::new_v4().to_string();
    let item_dir = staging::create_item_dir(job_dir, &item_id)
        .map_err(AudibleAcquisitionError::materialization)?;
    let ctx = TitleAcquisitionCtx {
        job_id,
        title_id,
        item_id: &item_id,
        item_dir: &item_dir,
        progress_context,
    };
    log::info!(
        "remote_source audible stage=title_start job_id={} title_ref={} include_pdf={}",
        job_id,
        title_ref(title_id),
        include_pdf
    );
    progress(title_progress(
        progress_context,
        AcquisitionStage::License,
        Some(0.05),
        None,
        None,
    ));
    let title_details = lookup_title_details(client, ctx, include_pdf, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::provider_protocol)?;
    let title_name = title_details.title.as_deref();
    let lane = request_license_lane(client, ctx, license_decrypt_context, progress, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::provider_protocol)?;
    if let Some(unsupported) = unsupported_result_for_unmaterializable_lane(title_id, job_id, &lane)
    {
        return Ok(unsupported);
    }

    let downloaded_path = if lane.strategy == AcquisitionStrategy::DownloadImportReady {
        staged_materialized_path(&item_dir, title_name, title_id)
    } else {
        staged_protected_source_path(&item_dir, lane.strategy)
    };
    download_audio(
        &lane.content_url,
        &downloaded_path,
        ctx,
        progress,
        is_cancelled,
    )
    .await
    .map_err(AudibleAcquisitionError::download)?;
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    progress(title_progress(
        progress_context,
        AcquisitionStage::Validation,
        Some(0.1),
        None,
        None,
    ));
    let materialized_path = if lane.strategy == AcquisitionStrategy::DownloadImportReady {
        downloaded_path
    } else {
        materialize_protected_download(
            materializer,
            &downloaded_path,
            title_name,
            &lane,
            ctx,
            progress,
            is_cancelled,
        )
        .await
        .map_err(AudibleAcquisitionError::materialization)?
    };
    let file = validate_materialized_audio(&materialized_path, ctx, progress)
        .await
        .map_err(AudibleAcquisitionError::validation)?;
    let supplemental_pdf_hint_present =
        supplemental_pdf_hint_present_for_acquisition(include_pdf, &title_details, &lane);
    let (assets, diagnostics) = download_supplemental_pdf_if_requested(
        auth,
        &file,
        title_name,
        include_pdf,
        supplemental_pdf_hint_present,
        ctx,
        is_cancelled,
    )
    .await?;
    Ok(AcquiredTitle {
        file: Some(file),
        assets,
        diagnostics,
    })
}

pub(super) fn unsupported_result_for_unmaterializable_lane(
    title_id: &str,
    job_id: &str,
    lane: &LicenseLane,
) -> Option<AcquiredTitle> {
    let strategy = lane.strategy;
    match strategy {
        AcquisitionStrategy::DownloadImportReady
        | AcquisitionStrategy::DownloadThenDecryptAax
        | AcquisitionStrategy::DownloadThenDecryptAaxc => None,
        AcquisitionStrategy::DownloadThenDecryptDash => {
            let lane = strategy_label(strategy);
            log::info!(
                "remote_source audible stage=materializer_selection job_id={} title_ref={} lane={} action=unsupported",
                job_id,
                title_ref(title_id),
                lane
            );
            Some(AcquiredTitle {
                file: None,
                assets: Vec::new(),
                diagnostics: vec![RemoteSourceDiagnostic {
                    kind: RemoteAcquisitionFailureKind::ProtectedUnsupported,
                    title_id: Some(title_id.to_string()),
                    message: format!(
                        "Audible returned {lane}; Dash/Widevine materialization is not supported in this build."
                    ),
                }],
            })
        }
        AcquisitionStrategy::ProviderProtocolFailed | AcquisitionStrategy::ProtectedUnsupported => {
            log::warn!(
                "remote_source audible stage=license_classification job_id={} title_ref={} lane={} action=failed",
                job_id,
                title_ref(title_id),
                strategy_label(strategy)
            );
            Some(AcquiredTitle {
                file: None,
                assets: Vec::new(),
                diagnostics: vec![RemoteSourceDiagnostic {
                    kind: if strategy == AcquisitionStrategy::ProviderProtocolFailed {
                        RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed
                    } else {
                        RemoteAcquisitionFailureKind::ProtectedUnsupported
                    },
                    title_id: Some(title_id.to_string()),
                    message: provider_protocol_lane_message(strategy),
                }],
            })
        }
    }
}

pub(super) fn supplemental_pdf_hint_present_for_acquisition(
    include_pdf: bool,
    title_details: &AudibleTitleDetails,
    lane: &LicenseLane,
) -> bool {
    include_pdf
        && (title_details.supplemental_pdf_url.is_some() || lane.supplemental_pdf_url.is_some())
}

pub(super) fn requested_supplemental_pdf_is_required(
    include_pdf: bool,
    api_pdf_hint_present: bool,
) -> bool {
    include_pdf && api_pdf_hint_present
}

async fn validate_materialized_audio(
    materialized_path: &Path,
    ctx: TitleAcquisitionCtx<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
) -> Result<MaterializedSourceFile> {
    let TitleAcquisitionCtx {
        title_id,
        progress_context,
        ..
    } = ctx;
    let title_id = title_id.to_string();
    let materialized_path = materialized_path.to_path_buf();
    let validation_result =
        tokio::task::spawn_blocking(move || {
            match materialized_file_from_path(&title_id, &materialized_path) {
                Ok(file) => Ok(file),
                Err(error) => {
                    cleanup_download_artifacts(&materialized_path)?;
                    Err(error)
                }
            }
        })
        .await
        .map_err(|error| {
            AppError::General(format!(
                "Materialized audio validation task failed: {error}"
            ))
        })?;

    let file = match validation_result {
        Ok(file) => file,
        Err(error) => {
            progress(title_progress(
                progress_context,
                AcquisitionStage::Failed,
                Some(1.0),
                None,
                None,
            ));
            return Err(error);
        }
    };
    progress(title_progress(
        progress_context,
        AcquisitionStage::Validation,
        Some(1.0),
        None,
        None,
    ));
    Ok(file)
}

pub(super) async fn download_supplemental_pdf_if_requested(
    auth: &Auth,
    file: &MaterializedSourceFile,
    title_name: Option<&str>,
    include_pdf: bool,
    api_pdf_hint_present: bool,
    ctx: TitleAcquisitionCtx<'_>,
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<(Vec<SupplementalAsset>, Vec<RemoteSourceDiagnostic>)> {
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
    if !requested_supplemental_pdf_is_required(include_pdf, api_pdf_hint_present) {
        return Ok((assets, diagnostics));
    }

    let supplemental_file_name = supplemental_pdf_display_file_name(title_name, title_id);
    match download_supplemental_pdf(
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
        Ok(asset) => assets.push(asset),
        Err(failure) if failure.category == "cancelled" => {
            return Err(AudibleAcquisitionError::cancellation(
                remote_acquisition_cancelled(),
            ))
        }
        Err(failure) => {
            log_supplemental_pdf_failed(job_id, title_id, failure);
            let _ = cleanup_download_artifacts(&file.path);
            return Err(AudibleAcquisitionError::supplemental_pdf(
                AppError::General(required_supplemental_pdf_failure_message(failure)),
            ));
        }
    }
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    Ok((assets, diagnostics))
}

pub(super) fn required_supplemental_pdf_failure_message(
    failure: super::supplemental_pdf::SupplementalPdfFailure,
) -> String {
    format!(
        "{} The audiobook was not imported because the requested Supplemental PDF is required for this Audible title.",
        supplemental_pdf_failure_message(failure)
    )
}

#[cfg(test)]
pub(super) fn materialized_file_from_downloaded_path(
    title_id: &str,
    path: &Path,
    strategy: AcquisitionStrategy,
) -> Result<MaterializedSourceFile> {
    let source_kind = abb_remote_source_core::classify_materialized_source_path(path);
    if !abb_remote_source_core::strategy_allows_import_handoff(strategy, source_kind) {
        return Err(AppError::FileValidation(format!(
            "Downloaded Audible {} requires Audible decryption before ABB import handoff.",
            kind_label(source_kind)
        )));
    }

    materialized_file_from_path(title_id, path)
}

fn materialized_file_from_path(title_id: &str, path: &Path) -> Result<MaterializedSourceFile> {
    let source_kind = abb_remote_source_core::classify_materialized_source_path(path);
    if !abb_remote_source_core::materialized_source_is_import_ready(source_kind) {
        return Err(AppError::FileValidation(format!(
            "Materialized Audible {} requires Audible decryption before ABB import handoff.",
            kind_label(source_kind)
        )));
    }

    let metadata = fs::metadata(path)?;
    let sha256 = sha256_file(path)?;

    match audio::get_file_list_info(std::slice::from_ref(&path)) {
        Ok(info) if info.valid_count == 1 => {
            let accepted_file = &info.files[0];
            Ok(MaterializedSourceFile {
                input_id: accepted_file.input_id.clone(),
                title_id: title_id.to_string(),
                path: accepted_file.path.clone(),
                size_bytes: metadata.len(),
                sha256,
            })
        }
        Ok(_) => Err(AppError::FileValidation(format!(
            "Materialized Audible file was not accepted as audio: {}",
            sanitize_path_for_display(path)
        ))),
        Err(error) => Err(error),
    }
}

fn kind_label(kind: MaterializedSourceKind) -> &'static str {
    match kind {
        MaterializedSourceKind::ImportReadyM4b => "M4B",
        MaterializedSourceKind::EncryptedAax => "AAX",
        MaterializedSourceKind::EncryptedAaxc => "AAXC",
        MaterializedSourceKind::SupplementalPdf => "PDF",
        MaterializedSourceKind::Unsupported => "file",
    }
}

pub(super) fn staged_protected_source_path(
    job_dir: &Path,
    strategy: AcquisitionStrategy,
) -> std::path::PathBuf {
    job_dir.join(format!(
        "source.{}",
        download_extension_for_strategy(strategy)
    ))
}

pub(super) fn staged_materialized_path(
    job_dir: &Path,
    title_name: Option<&str>,
    title_id: &str,
) -> std::path::PathBuf {
    job_dir.join(format!(
        "{}.m4b",
        remote_materialized_filename_stem(title_name, title_id)
    ))
}

pub(super) fn generated_staging_path(job_dir: &Path, extension: &str) -> std::path::PathBuf {
    job_dir.join(format!("{}.{}", uuid::Uuid::new_v4(), extension))
}

pub(super) fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_source::providers::audible::audio_download::{
        cleanup_download_artifacts, download_status_failure, partial_download_path,
    };
    use crate::remote_source::providers::audible::license::AudibleTitleDetails;
    use crate::remote_source::providers::audible::supplemental_pdf;
    use audible_api::auth::localization;
    use secrecy::ExposeSecret;
    use serde_json::json;
    use std::collections::HashMap;

    fn fixture_auth_without_pdf_cookies() -> Auth {
        Auth {
            locale: localization::find_by_country_code(super::super::COUNTRY_CODE).expect("locale"),
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
                device_info: json!({ "device_type": super::super::AUDIBLE_IOS_DEVICE_TYPE }),
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
            sha256_file(&path).expect("hash file"),
            abb_media_core::sha256_hex(bytes)
        );
    }

    #[test]
    fn requested_supplemental_pdf_is_required_only_when_audible_advertises_one() {
        assert!(requested_supplemental_pdf_is_required(true, true));
        assert!(!requested_supplemental_pdf_is_required(true, false));
        assert!(!requested_supplemental_pdf_is_required(false, true));
        assert!(!requested_supplemental_pdf_is_required(false, false));
    }

    #[test]
    fn required_supplemental_pdf_failure_message_keeps_provider_details_redacted() {
        let failure = supplemental_pdf::SupplementalPdfFailure {
            category: "status",
            status: Some(reqwest::StatusCode::FORBIDDEN),
        };
        let message = required_supplemental_pdf_failure_message(failure);

        assert!(message.contains("requested Supplemental PDF is required"));
        assert!(!message.contains("B000000001"));
        assert!(!message.contains("https://"));
        assert!(!message.contains("403"));
    }

    #[tokio::test]
    async fn requested_advertised_supplemental_pdf_failure_blocks_audio_handoff() {
        let root = tempfile::TempDir::new().expect("temp root");
        let auth = fixture_auth_without_pdf_cookies();
        let audio_path = root.path().join("Book.m4b");
        std::fs::write(&audio_path, b"audio-bytes").expect("write audio");
        let file = materialized_source_file(audio_path.clone());
        let ctx = test_title_ctx(root.path(), test_progress_context());

        let error = download_supplemental_pdf_if_requested(
            &auth,
            &file,
            Some("Book"),
            true,
            true,
            ctx,
            &|| false,
        )
        .await
        .expect_err("advertised requested Supplemental PDF failure should fail title");

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
            "audio handoff file should be cleaned when required PDF fails"
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

        let (assets, diagnostics) = download_supplemental_pdf_if_requested(
            &auth,
            &file,
            Some("Book"),
            true,
            false,
            ctx,
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

        let (assets, diagnostics) = download_supplemental_pdf_if_requested(
            &auth,
            &file,
            Some("Book"),
            false,
            true,
            ctx,
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

        let result = materialized_file_from_downloaded_path(
            "B000000001",
            &encrypted,
            abb_remote_source_core::AcquisitionStrategy::DownloadThenDecryptAax,
        );

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
    fn supplemental_pdf_hint_uses_title_or_license_presence_without_exposing_url() {
        let details = AudibleTitleDetails {
            title: Some("Remote Book".to_string()),
            supplemental_pdf_url: None,
        };
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/book.aax".to_string(),
            strategy: AcquisitionStrategy::DownloadThenDecryptAax,
            decryption_material: None,
            supplemental_pdf_url: Some("https://cdn.example.test/book.pdf".to_string()),
        };

        assert!(supplemental_pdf_hint_present_for_acquisition(
            true, &details, &lane
        ));
        assert!(!supplemental_pdf_hint_present_for_acquisition(
            false, &details, &lane
        ));
    }

    #[test]
    fn supplemental_pdf_hint_treats_api_pdf_url_as_presence_not_download_candidate() {
        let details = AudibleTitleDetails {
            title: Some("Remote Book".to_string()),
            supplemental_pdf_url: Some("https://metadata.example.test/book.pdf".to_string()),
        };
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/book.aax".to_string(),
            strategy: AcquisitionStrategy::DownloadThenDecryptAax,
            decryption_material: None,
            supplemental_pdf_url: Some("https://license.example.test/book.pdf".to_string()),
        };

        assert!(supplemental_pdf_hint_present_for_acquisition(
            true, &details, &lane
        ));
    }

    #[test]
    fn cleanup_download_artifacts_removes_partial_and_intermediate_files() {
        let root = tempfile::TempDir::new().expect("temp root");
        let final_path = root.path().join("book.aax");
        let partial_path = partial_download_path(&final_path);
        std::fs::write(&final_path, b"encrypted").expect("write final");
        std::fs::write(&partial_path, b"partial").expect("write partial");

        cleanup_download_artifacts(&final_path).expect("cleanup artifacts");

        assert!(!final_path.exists());
        assert!(!partial_path.exists());
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
}
