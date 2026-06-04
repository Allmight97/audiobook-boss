use std::fs;
use std::path::Path;

use abb_remote_source_core::{
    acquisition_progress, acquisition_progress_for_current_title, choose_acquisition_strategy,
    license_facts_from_value, AcquisitionProgress, AcquisitionStage, AcquisitionStrategy,
    LicenseFacts, MaterializedSourceKind,
};
use aes::Aes128;
use audible_api::api::Client as AudibleClient;
use audible_api::auth::oauth::{build_oauth_url, extract_auth_code};
use audible_api::auth::register::register;
use audible_api::auth::{localization, Auth};
use base64::{
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, STANDARD_NO_PAD as BASE64_STANDARD_NO_PAD, URL_SAFE,
        URL_SAFE_NO_PAD,
    },
    Engine as _,
};
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use reqwest::header::{CONTENT_RANGE, RANGE, USER_AGENT};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

mod library;

use library::parse_library_titles;

use crate::audio;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::remote_source::materializer::{
    AaxcleanLane, AaxcleanMaterializer, AaxcleanSecret, MaterializationRequest,
};
use crate::remote_source::staging;
use crate::remote_source::vault::SecretVault;
use crate::remote_source::{
    AccountRef, AcquisitionJob, AcquisitionPlan, MaterializedSourceFile, ProviderId,
    RemoteAccountStatus, RemoteAcquisitionFailureKind, RemoteAcquisitionStatus, RemoteAuthFlow,
    RemoteLibraryResponse, RemoteSourceAccountState, RemoteSourceDiagnostic,
    RemoteSourceProviderCapabilities, SupplementalAsset,
};

const COUNTRY_CODE: &str = "us";
const DOMAIN: &str = "com";
const MARKETPLACE_ID: &str = "ATVPDKIKX0DER";
const ACCOUNT_ID: &str = "audible-us";
const AUTH_SECRET_KEY: &str = "audible.us.auth";
const MAX_SUPPLEMENTAL_PDF_BYTES: u64 = 100 * 1024 * 1024;
const MAX_DOWNLOAD_REDIRECTS: usize = 5;
const MAX_DOWNLOAD_ATTEMPTS: usize = 4;
const AUDIBLE_DOWNLOAD_USER_AGENT: &str = "Audible/671 CFNetwork/1240.0.4 Darwin/20.6.0";
const AUDIBLE_IOS_DEVICE_TYPE: &str = "A2CZJZGLK2JJVM";
type Aes128CbcDec = cbc::Decryptor<Aes128>;

struct AcquiredTitle {
    file: Option<MaterializedSourceFile>,
    assets: Vec<SupplementalAsset>,
    diagnostics: Vec<RemoteSourceDiagnostic>,
}

struct LicenseLane {
    content_url: String,
    strategy: AcquisitionStrategy,
    decryption_material: Option<AudibleDecryptionMaterial>,
}

struct AudibleLicenseDecryptContext {
    device_type: String,
    device_serial: String,
    amazon_account_id: String,
}

#[derive(Clone, Copy)]
struct TitleProgressContext<'a> {
    title_id: &'a str,
    item_index: u32,
    total_items: u32,
}

#[derive(Clone, Copy)]
struct DownloadLogContext<'a> {
    job_id: &'a str,
    title_id: &'a str,
    extension: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedContentRange {
    start: u64,
    end: u64,
    total: Option<u64>,
}

enum AudibleDecryptionMaterial {
    Aax {
        activation_bytes_hex: SecretString,
    },
    Aaxc {
        key_hex: SecretString,
        iv_hex: SecretString,
    },
}

fn provider_private_failure(stage: &str) -> AppError {
    AppError::General(format!(
        "Audible {stage} failed. Provider-private details were withheld from UI and logs."
    ))
}

fn download_failure(stage: &str) -> AppError {
    AppError::General(format!(
        "Audible download {stage} failed. Provider-private details were withheld from UI and logs."
    ))
}

fn download_status_failure(status: reqwest::StatusCode) -> AppError {
    AppError::General(format!(
        "Remote source download returned HTTP {}. Check application logs for sanitized acquisition facts.",
        status.as_u16()
    ))
}

fn remote_acquisition_cancelled() -> AppError {
    AppError::Cancellation("Remote source acquisition was cancelled.".to_string())
}

fn ensure_not_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<()> {
    if is_cancelled() {
        return Err(remote_acquisition_cancelled());
    }
    Ok(())
}

fn title_progress(
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

fn with_title_progress(
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

#[derive(Debug, Clone)]
pub(in crate::remote_source) struct PendingAudibleAuth {
    pub(super) code_verifier: String,
    pub(super) device_serial: String,
}

#[derive(Debug, Clone)]
pub(in crate::remote_source) struct AudibleProvider;

impl AudibleProvider {
    pub(in crate::remote_source) fn capabilities() -> RemoteSourceProviderCapabilities {
        RemoteSourceProviderCapabilities {
            provider_id: ProviderId::Audible,
            label: "Audible".to_string(),
            auth_flow: RemoteAuthFlow::ExternalBrowserHandoff,
            supports_library_scan: true,
            supports_paged_scan: true,
            supports_typeahead_filter: true,
            supports_supplemental_pdf: true,
            supports_materialized_audio: true,
            supports_refresh: true,
            requires_live_session: true,
            known_unsupported_reasons: vec![
                RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
                RemoteAcquisitionFailureKind::ProtectedUnsupported,
            ],
        }
    }

    pub(in crate::remote_source) fn start_auth() -> Result<(String, PendingAudibleAuth)> {
        let (url, code_verifier, device_serial) =
            build_oauth_url(COUNTRY_CODE, DOMAIN, MARKETPLACE_ID, None, false).map_err(
                |error| AppError::General(format!("Failed to start Audible auth flow: {error}")),
            )?;
        Ok((
            url,
            PendingAudibleAuth {
                code_verifier,
                device_serial,
            },
        ))
    }

    pub(in crate::remote_source) async fn complete_auth(
        vault: &dyn SecretVault,
        pending: PendingAudibleAuth,
        response_url: &str,
    ) -> Result<RemoteSourceAccountState> {
        let authorization_code = extract_audible_auth_code(response_url)?;
        let registration = register(
            &authorization_code,
            &pending.code_verifier,
            DOMAIN,
            &pending.device_serial,
            false,
        )
        .await
        .map_err(|_| provider_private_failure("device registration"))?;
        let locale = localization::find_by_country_code(COUNTRY_CODE)
            .ok_or_else(|| AppError::General("Audible locale is unavailable".to_string()))?;
        let auth = Auth {
            locale,
            device_registration: registration,
            authorization_code,
            code_verifier: pending.code_verifier,
        };
        let serialized = serde_json::to_string(&auth).map_err(|error| {
            AppError::General(format!("Failed to serialize Audible auth: {error}"))
        })?;
        vault.set_secret(AUTH_SECRET_KEY, SecretString::from(serialized))?;
        Self::account_state(vault)
    }

    pub(in crate::remote_source) fn account_state(
        vault: &dyn SecretVault,
    ) -> Result<RemoteSourceAccountState> {
        let Some(secret) = vault.get_secret(AUTH_SECRET_KEY)? else {
            return Ok(RemoteSourceAccountState {
                provider_id: ProviderId::Audible,
                status: RemoteAccountStatus::NeedsAuth,
                account: None,
                message: Some("Connect Audible to load your library.".to_string()),
            });
        };

        let auth: Auth = serde_json::from_str(secret.expose_secret()).map_err(|error| {
            AppError::ResourceCleanup(format!("Stored Audible auth is invalid: {error}"))
        })?;
        let display_name = auth
            .device_registration
            .customer_info
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Audible account")
            .to_string();

        Ok(RemoteSourceAccountState {
            provider_id: ProviderId::Audible,
            status: RemoteAccountStatus::Connected,
            account: Some(AccountRef {
                provider_id: ProviderId::Audible,
                account_id: ACCOUNT_ID.to_string(),
                display_name,
            }),
            message: None,
        })
    }

    pub(in crate::remote_source) fn logout(vault: &dyn SecretVault) -> Result<()> {
        vault.delete_secret(AUTH_SECRET_KEY)
    }

    pub(in crate::remote_source) async fn load_library(
        vault: &dyn SecretVault,
    ) -> Result<RemoteLibraryResponse> {
        let client = client_from_vault(vault)?;
        let payload = client
            .get_library(Some(json!({
                "num_results": 100,
                "response_groups": "product_desc,product_attrs,contributors,media,pdf_url,product_details",
                "sort_by": "-PurchaseDate"
            })))
            .await
            .map_err(|_| provider_private_failure("library request"))?;
        let titles = parse_library_titles(&payload);
        Ok(RemoteLibraryResponse {
            provider_id: ProviderId::Audible,
            titles,
            diagnostics: Vec::new(),
        })
    }

    pub(in crate::remote_source) async fn acquire(
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
        let client = client_from_auth(auth)?;
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
            match acquire_one(
                &client,
                materializer,
                &selection.title_id,
                selection.include_supplemental_pdf,
                job_id,
                job_dir,
                license_decrypt_context.as_ref(),
                TitleProgressContext {
                    title_id: &selection.title_id,
                    item_index: u32::try_from(selection_index + 1).unwrap_or(u32::MAX),
                    total_items,
                },
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
                    if matches!(error, AppError::Cancellation(_)) {
                        return Err(error);
                    }
                    let kind = diagnostic_kind_for_acquire_error(&error);
                    job.diagnostics.push(RemoteSourceDiagnostic {
                        kind,
                        title_id: Some(selection.title_id.clone()),
                        message: error.to_string(),
                    });
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
}

fn diagnostic_kind_for_acquire_error(error: &AppError) -> RemoteAcquisitionFailureKind {
    let message = error.to_string();
    if matches!(error, AppError::FileValidation(_)) {
        return RemoteAcquisitionFailureKind::ValidationFailed;
    }
    if message.contains("Remote source download") || message.contains("Audible download") {
        return RemoteAcquisitionFailureKind::DownloadFailed;
    }
    if message.contains("Audible license") || message.contains("license response") {
        return RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed;
    }
    if message.contains("AAXClean") {
        return RemoteAcquisitionFailureKind::MaterializationFailed;
    }
    RemoteAcquisitionFailureKind::MaterializationFailed
}

fn client_from_vault(vault: &dyn SecretVault) -> Result<AudibleClient> {
    client_from_auth(auth_from_vault(vault)?)
}

fn auth_from_vault(vault: &dyn SecretVault) -> Result<Auth> {
    let secret = vault
        .get_secret(AUTH_SECRET_KEY)?
        .ok_or_else(|| AppError::InvalidInput("Audible account is not connected.".to_string()))?;
    serde_json::from_str(secret.expose_secret()).map_err(|error| {
        AppError::ResourceCleanup(format!("Stored Audible auth is invalid: {error}"))
    })
}

fn client_from_auth(auth: Auth) -> Result<AudibleClient> {
    AudibleClient::new(auth)
        .map_err(|error| AppError::General(format!("Failed to create Audible client: {error}")))
}

fn license_decrypt_context_from_auth(auth: &Auth) -> Option<AudibleLicenseDecryptContext> {
    let device_type =
        find_first_string_for_key(&auth.device_registration.device_info, "device_type")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| AUDIBLE_IOS_DEVICE_TYPE.to_string());
    let device_serial = find_first_string_for_key(
        &auth.device_registration.device_info,
        "device_serial_number",
    )
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| auth.device_registration.device_serial.clone());
    let amazon_account_id =
        find_first_string_for_key(&auth.device_registration.customer_info, "user_id")
            .filter(|value| !value.is_empty())?;

    Some(AudibleLicenseDecryptContext {
        device_type,
        device_serial,
        amazon_account_id,
    })
}

fn extract_audible_auth_code(response_url: &str) -> Result<String> {
    extract_auth_code(response_url).map_err(|_| {
        AppError::InvalidInput(
            "Audible auth response URL was not accepted. Provide the final Amazon redirect URL saved by the auth handoff."
                .to_string(),
        )
    })
}

async fn acquire_one(
    client: &AudibleClient,
    materializer: &AaxcleanMaterializer,
    title_id: &str,
    include_pdf: bool,
    job_id: &str,
    job_dir: &Path,
    license_decrypt_context: Option<&AudibleLicenseDecryptContext>,
    progress_context: TitleProgressContext<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<AcquiredTitle> {
    ensure_not_cancelled(is_cancelled)?;
    let item_id = uuid::Uuid::new_v4().to_string();
    let item_dir = staging::create_item_dir(job_dir, &item_id)?;
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
    let supplemental_pdf_url =
        lookup_supplemental_pdf_url(client, title_id, include_pdf, is_cancelled).await?;
    let lane = request_license_lane(
        client,
        title_id,
        job_id,
        license_decrypt_context,
        progress_context,
        progress,
        is_cancelled,
    )
    .await?;
    if let Some(unsupported) = unsupported_result_for_unmaterializable_lane(title_id, job_id, &lane)
    {
        return Ok(unsupported);
    }

    let downloaded_path = if lane.strategy == AcquisitionStrategy::DownloadImportReady {
        staged_materialized_path(&item_dir)
    } else {
        staged_protected_source_path(&item_dir, lane.strategy)
    };
    download_audio(
        &lane.content_url,
        &downloaded_path,
        job_id,
        title_id,
        progress_context,
        progress,
        is_cancelled,
    )
    .await?;
    ensure_not_cancelled(is_cancelled)?;
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
            title_id,
            job_id,
            &item_id,
            &downloaded_path,
            &item_dir,
            &lane,
            progress_context,
            progress,
            is_cancelled,
        )
        .await?
    };
    let file =
        validate_materialized_audio(title_id, &materialized_path, progress_context, progress)?;
    let (assets, diagnostics) = download_supplemental_pdf_if_requested(
        title_id,
        &file,
        supplemental_pdf_url,
        &item_dir,
        is_cancelled,
    )
    .await?;
    Ok(AcquiredTitle {
        file: Some(file),
        assets,
        diagnostics,
    })
}

async fn lookup_supplemental_pdf_url(
    client: &AudibleClient,
    title_id: &str,
    include_pdf: bool,
    is_cancelled: &impl Fn() -> bool,
) -> Result<Option<String>> {
    ensure_not_cancelled(is_cancelled)?;
    let metadata = client
        .get_library_item_by_asin(
            title_id,
            Some(json!({
                "response_groups": "product_desc,product_attrs,contributors,media,pdf_url,product_details"
            })),
        )
        .await
        .map_err(|_| provider_private_failure("title lookup"))?;
    ensure_not_cancelled(is_cancelled)?;
    Ok(include_pdf
        .then(|| {
            find_first_string_for_key(&metadata, "pdf_url")
                .or_else(|| find_first_string_for_key(&metadata, "pdfUrl"))
        })
        .flatten())
}

async fn request_license_lane(
    client: &AudibleClient,
    title_id: &str,
    job_id: &str,
    license_decrypt_context: Option<&AudibleLicenseDecryptContext>,
    progress_context: TitleProgressContext<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<LicenseLane> {
    ensure_not_cancelled(is_cancelled)?;
    progress(title_progress(
        progress_context,
        AcquisitionStage::License,
        Some(0.45),
        None,
        None,
    ));
    let license_payload = license_request_payload();
    let license = post_license_request_json(client, title_id, &license_payload, job_id).await?;
    ensure_not_cancelled(is_cancelled)?;
    let facts = license_facts_from_value(&license);
    let strategy = choose_acquisition_strategy(&facts);
    log_license_facts(job_id, title_id, &facts, strategy);
    let Some(content_url) = facts.content_url.as_deref() else {
        log::warn!(
            "remote_source audible stage=license_classification job_id={} title_ref={} lane=provider_protocol_failed reason=missing_download_url",
            job_id,
            title_ref(title_id)
        );
        return Ok(LicenseLane {
            content_url: String::new(),
            strategy: AcquisitionStrategy::ProviderProtocolFailed,
            decryption_material: None,
        });
    };

    Ok(LicenseLane {
        content_url: content_url.to_string(),
        strategy,
        decryption_material: {
            let material = audible_decryption_material_from_license(
                &license,
                strategy,
                title_id,
                license_decrypt_context,
            );
            log_missing_license_material(
                job_id,
                title_id,
                &license,
                &facts,
                strategy,
                license_decrypt_context.is_some(),
                material.is_some(),
            );
            material
        },
    })
}

fn unsupported_result_for_unmaterializable_lane(
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

fn provider_protocol_lane_message(strategy: AcquisitionStrategy) -> String {
    if strategy == AcquisitionStrategy::ProviderProtocolFailed {
        return "Audible license response did not include a downloadable audio URL.".to_string();
    }
    format!(
        "Audible returned {}; ABB cannot materialize this lane in the current build.",
        strategy_label(strategy)
    )
}

async fn materialize_protected_download(
    materializer: &AaxcleanMaterializer,
    title_id: &str,
    job_id: &str,
    item_id: &str,
    downloaded_path: &Path,
    item_dir: &Path,
    lane: &LicenseLane,
    progress_context: TitleProgressContext<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<std::path::PathBuf> {
    let Some((helper_lane, secret)) = helper_material_from_audible_material(lane) else {
        log::warn!(
            "remote_source audible stage=materializer_failed job_id={} title_ref={} item_id={} category=decryption_material",
            job_id,
            title_ref(title_id),
            item_id
        );
        cleanup_download_artifacts(downloaded_path)?;
        return Err(provider_private_failure("AAXClean decryption material"));
    };
    progress(title_progress(
        progress_context,
        AcquisitionStage::Decryption,
        Some(0.0),
        None,
        None,
    ));
    let output_path = staged_materialized_path(item_dir);
    let output_temp_path = materializer_output_temp_path(&output_path);
    let mut materializer_progress = |progress_event: AcquisitionProgress| {
        progress(with_title_progress(progress_event, progress_context));
    };
    let result = materializer
        .materialize(
            MaterializationRequest {
                job_id: job_id.to_string(),
                operation_id: item_id.to_string(),
                lane: helper_lane,
                input_path: downloaded_path.to_path_buf(),
                output_temp_path,
                output_path: output_path.clone(),
                secret,
            },
            &mut materializer_progress,
            is_cancelled,
        )
        .await;
    let protected_cleanup = cleanup_download_artifacts(downloaded_path);
    match result {
        Ok(path) => {
            if protected_cleanup.is_err() {
                let _ = cleanup_download_artifacts(&output_path);
                return Err(provider_private_failure("staged protected cleanup"));
            }
            Ok(path)
        }
        Err(error) => {
            let _ = protected_cleanup;
            let _ = cleanup_download_artifacts(&output_path);
            if matches!(error, AppError::Cancellation(_)) {
                return Err(error);
            }
            log::warn!(
                "remote_source audible stage=materialization_failed job_id={} title_ref={} lane={}",
                job_id,
                title_ref(title_id),
                strategy_label(lane.strategy)
            );
            Err(error)
        }
    }
}

fn validate_materialized_audio(
    title_id: &str,
    materialized_path: &Path,
    progress_context: TitleProgressContext<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
) -> Result<MaterializedSourceFile> {
    let file = match materialized_file_from_path(title_id, materialized_path) {
        Ok(file) => file,
        Err(error) => {
            cleanup_download_artifacts(materialized_path)?;
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

async fn download_supplemental_pdf_if_requested(
    title_id: &str,
    file: &MaterializedSourceFile,
    supplemental_pdf_url: Option<String>,
    job_dir: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> Result<(Vec<SupplementalAsset>, Vec<RemoteSourceDiagnostic>)> {
    let mut assets = Vec::new();
    let mut diagnostics = Vec::new();
    if let Some(pdf_url) = supplemental_pdf_url {
        ensure_not_cancelled(is_cancelled)?;
        match download_pdf(title_id, &file.input_id, &pdf_url, job_dir, is_cancelled).await {
            Ok(asset) => assets.push(asset),
            Err(AppError::Cancellation(message)) => {
                return Err(AppError::Cancellation(message));
            }
            Err(error) => {
                diagnostics.push(RemoteSourceDiagnostic {
                    kind: RemoteAcquisitionFailureKind::SupplementalPdfFailed,
                    title_id: Some(title_id.to_string()),
                    message: error.to_string(),
                });
            }
        }
    }
    ensure_not_cancelled(is_cancelled)?;
    Ok((assets, diagnostics))
}

async fn download_audio(
    content_url: &str,
    path: &Path,
    job_id: &str,
    title_id: &str,
    progress_context: TitleProgressContext<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<()> {
    ensure_not_cancelled(is_cancelled)?;
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| std::borrow::Cow::Borrowed("bin"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    log::info!(
        "remote_source audible stage=download_start job_id={} title_ref={} extension={}",
        job_id,
        title_ref(title_id),
        extension
    );
    let mut download_progress = |progress_event: AcquisitionProgress| {
        progress(with_title_progress(progress_event, progress_context));
    };
    let bytes = download_to_path(
        content_url,
        path,
        Some(DownloadLogContext {
            job_id,
            title_id,
            extension: &extension,
        }),
        &mut download_progress,
        is_cancelled,
    )
    .await?;
    if let Err(error @ AppError::Cancellation(_)) = ensure_not_cancelled(is_cancelled) {
        cleanup_download_artifacts(path)?;
        return Err(error);
    }
    log::info!(
        "remote_source audible stage=download_complete job_id={} title_ref={} extension={} bytes={}",
        job_id,
        title_ref(title_id),
        extension,
        bytes
    );
    Ok(())
}

#[cfg(test)]
fn materialized_file_from_downloaded_path(
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

fn download_extension_for_strategy(strategy: AcquisitionStrategy) -> &'static str {
    match strategy {
        AcquisitionStrategy::DownloadImportReady => "m4b",
        AcquisitionStrategy::DownloadThenDecryptAax => "aax",
        AcquisitionStrategy::DownloadThenDecryptAaxc => "aaxc",
        AcquisitionStrategy::DownloadThenDecryptDash => "mpd",
        AcquisitionStrategy::ProtectedUnsupported | AcquisitionStrategy::ProviderProtocolFailed => {
            "bin"
        }
    }
}

fn staged_protected_source_path(
    job_dir: &Path,
    strategy: AcquisitionStrategy,
) -> std::path::PathBuf {
    job_dir.join(format!(
        "source.{}",
        download_extension_for_strategy(strategy)
    ))
}

fn staged_materialized_path(job_dir: &Path) -> std::path::PathBuf {
    job_dir.join("materialized.m4b")
}

fn materializer_output_temp_path(path: &Path) -> std::path::PathBuf {
    path.with_extension("m4b.partial")
}

fn helper_material_from_audible_material(
    lane: &LicenseLane,
) -> Option<(AaxcleanLane, AaxcleanSecret)> {
    match lane.decryption_material.as_ref()? {
        AudibleDecryptionMaterial::Aax {
            activation_bytes_hex,
        } => Some((
            AaxcleanLane::Aax,
            AaxcleanSecret::Aax {
                activation_bytes_hex: activation_bytes_hex.clone(),
            },
        )),
        AudibleDecryptionMaterial::Aaxc { key_hex, iv_hex } => Some((
            AaxcleanLane::Aaxc,
            AaxcleanSecret::Aaxc {
                key_hex: key_hex.clone(),
                iv_hex: iv_hex.clone(),
            },
        )),
    }
}

fn audible_decryption_material_from_license(
    license: &Value,
    strategy: AcquisitionStrategy,
    title_id: &str,
    decrypt_context: Option<&AudibleLicenseDecryptContext>,
) -> Option<AudibleDecryptionMaterial> {
    if let Some(material) =
        decrypt_license_response_material(license, strategy, title_id, decrypt_context)
    {
        return Some(material);
    }

    let key_material = find_voucher_key_material(license);
    if let Some(material) = audible_decryption_material_from_key_material(key_material.as_ref()) {
        return Some(material);
    }

    match strategy {
        AcquisitionStrategy::DownloadThenDecryptAax => {
            find_activation_bytes(license).map(|activation_bytes| AudibleDecryptionMaterial::Aax {
                activation_bytes_hex: SecretString::from(hex_encode(&activation_bytes)),
            })
        }
        AcquisitionStrategy::DownloadThenDecryptAaxc => {
            find_key_iv_pair(license).map(|(key, iv)| AudibleDecryptionMaterial::Aaxc {
                key_hex: SecretString::from(hex_encode(&key)),
                iv_hex: SecretString::from(hex_encode(&iv)),
            })
        }
        _ => None,
    }
}

fn decrypt_license_response_material(
    license: &Value,
    strategy: AcquisitionStrategy,
    title_id: &str,
    decrypt_context: Option<&AudibleLicenseDecryptContext>,
) -> Option<AudibleDecryptionMaterial> {
    match strategy {
        AcquisitionStrategy::DownloadThenDecryptAax
        | AcquisitionStrategy::DownloadThenDecryptAaxc => {}
        _ => return None,
    }

    let context = decrypt_context?;
    let license_response =
        find_first_string_for_keys(license, &["license_response", "licenseResponse"])?;
    let asin =
        find_first_string_for_keys(license, &["asin", "Asin"]).unwrap_or_else(|| title_id.into());
    let decrypted = decrypt_license_response(&license_response, &asin, context)?;
    let key_material = find_voucher_key_material(&decrypted);
    audible_decryption_material_from_key_material(key_material.as_ref())
}

fn decrypt_license_response(
    license_response: &str,
    asin: &str,
    context: &AudibleLicenseDecryptContext,
) -> Option<Value> {
    let mut ciphertext = decode_base64_any(license_response)?;
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return None;
    }

    let key_components = format!(
        "{}{}{}{}",
        context.device_type, context.device_serial, context.amazon_account_id, asin
    );
    let hash = Sha256::digest(key_components.as_bytes());
    let (key, iv) = hash.split_at(16);
    let plaintext = Aes128CbcDec::new_from_slices(key, iv)
        .ok()?
        .decrypt_padded_mut::<NoPadding>(&mut ciphertext)
        .ok()?;
    let json_bytes = plaintext
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or(plaintext);
    serde_json::from_slice(json_bytes).ok()
}

fn audible_decryption_material_from_key_material(
    key_material: Option<&VoucherKeyMaterial>,
) -> Option<AudibleDecryptionMaterial> {
    let material = key_material?;
    match (material.key.as_slice(), material.iv.as_deref()) {
        (activation_bytes, None) if activation_bytes.len() == 4 => {
            Some(AudibleDecryptionMaterial::Aax {
                activation_bytes_hex: SecretString::from(hex_encode(activation_bytes)),
            })
        }
        (key, Some(iv)) if key.len() == 16 && iv.len() == 16 => {
            Some(AudibleDecryptionMaterial::Aaxc {
                key_hex: SecretString::from(hex_encode(key)),
                iv_hex: SecretString::from(hex_encode(iv)),
            })
        }
        _ => None,
    }
}

fn log_missing_license_material(
    job_id: &str,
    title_id: &str,
    license: &Value,
    facts: &LicenseFacts,
    strategy: AcquisitionStrategy,
    decrypt_context_present: bool,
    material_present: bool,
) {
    if material_present {
        return;
    }
    if !matches!(
        strategy,
        AcquisitionStrategy::DownloadThenDecryptAax | AcquisitionStrategy::DownloadThenDecryptAaxc
    ) {
        return;
    }
    log::warn!(
        "remote_source audible stage=license_material_extraction job_id={} title_ref={} strategy={} provider_material_hint={} license_response_present={} content_license_asin_present={} decrypt_context_present={} material_extracted=false",
        job_id,
        title_ref(title_id),
        strategy_label(strategy),
        facts.decryption_material_present,
        find_first_string_for_keys(license, &["license_response", "licenseResponse"]).is_some(),
        find_first_string_for_keys(license, &["asin", "Asin"]).is_some(),
        decrypt_context_present
    );
}

struct VoucherKeyMaterial {
    key: Vec<u8>,
    iv: Option<Vec<u8>>,
}

fn find_voucher_key_material(value: &Value) -> Option<VoucherKeyMaterial> {
    match value {
        Value::Object(map) => {
            if let Some(voucher) = map
                .get("voucher")
                .or_else(|| map.get("Voucher"))
                .or_else(|| map.get("content_license"))
                .or_else(|| map.get("contentLicense"))
                .and_then(find_voucher_key_material)
            {
                return Some(voucher);
            }
            let key = find_bytes_for_keys_in_object(
                map,
                &[
                    "key",
                    "Key",
                    "keyHex",
                    "key_hex",
                    "activation_bytes",
                    "activationBytes",
                    "activationBytesHex",
                    "activation_bytes_hex",
                ],
                &[4, 16],
            );
            if let Some(key) = key {
                let iv =
                    find_bytes_for_keys_in_object(map, &["iv", "Iv", "ivHex", "iv_hex"], &[16]);
                return Some(VoucherKeyMaterial { key, iv });
            }
            map.values().find_map(find_voucher_key_material)
        }
        Value::Array(values) => values.iter().find_map(find_voucher_key_material),
        _ => None,
    }
}

fn find_activation_bytes(value: &Value) -> Option<Vec<u8>> {
    find_bytes_for_keys(
        value,
        &[
            "activation_bytes",
            "activationBytes",
            "activationBytesHex",
            "activation_bytes_hex",
            "activation",
            "key",
            "Key",
        ],
        &[4],
    )
}

fn find_key_iv_pair(value: &Value) -> Option<(Vec<u8>, Vec<u8>)> {
    match value {
        Value::Object(map) => {
            let key =
                find_bytes_for_keys_in_object(map, &["key", "Key", "keyHex", "key_hex"], &[16]);
            let iv = find_bytes_for_keys_in_object(map, &["iv", "Iv", "ivHex", "iv_hex"], &[16]);
            if let (Some(key), Some(iv)) = (key, iv) {
                return Some((key, iv));
            }
            map.values().find_map(find_key_iv_pair)
        }
        Value::Array(values) => values.iter().find_map(find_key_iv_pair),
        _ => None,
    }
}

fn find_bytes_for_keys(value: &Value, keys: &[&str], expected_lens: &[usize]) -> Option<Vec<u8>> {
    match value {
        Value::Object(map) => {
            if let Some(bytes) = find_bytes_for_keys_in_object(map, keys, expected_lens) {
                return Some(bytes);
            }
            map.values()
                .find_map(|entry| find_bytes_for_keys(entry, keys, expected_lens))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_bytes_for_keys(entry, keys, expected_lens)),
        _ => None,
    }
}

fn find_bytes_for_keys_in_object(
    map: &serde_json::Map<String, Value>,
    keys: &[&str],
    expected_lens: &[usize],
) -> Option<Vec<u8>> {
    keys.iter()
        .filter_map(|key| map.get(*key))
        .find_map(|value| decode_secret_bytes(value, expected_lens))
}

fn decode_secret_bytes(value: &Value, expected_lens: &[usize]) -> Option<Vec<u8>> {
    match value {
        Value::String(raw) => decode_secret_string(raw, expected_lens),
        Value::Array(values) => {
            let bytes = values
                .iter()
                .map(|value| value.as_u64().and_then(|byte| u8::try_from(byte).ok()))
                .collect::<Option<Vec<_>>>()?;
            expected_lens.contains(&bytes.len()).then_some(bytes)
        }
        _ => None,
    }
}

fn decode_secret_string(raw: &str, expected_lens: &[usize]) -> Option<Vec<u8>> {
    let trimmed = raw.trim();
    let hex = trimmed
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>();
    if expected_lens
        .iter()
        .any(|expected_len| hex.len() == expected_len * 2)
        && trimmed.chars().all(|character| {
            character.is_ascii_hexdigit()
                || character == '-'
                || character == ':'
                || character == ' '
        })
    {
        return decode_hex(&hex);
    }

    [
        &BASE64_STANDARD,
        &BASE64_STANDARD_NO_PAD,
        &URL_SAFE,
        &URL_SAFE_NO_PAD,
    ]
    .iter()
    .find_map(|engine| {
        engine
            .decode(trimmed)
            .ok()
            .filter(|bytes| expected_lens.contains(&bytes.len()))
    })
}

fn decode_base64_any(raw: &str) -> Option<Vec<u8>> {
    let trimmed = raw.trim();
    [
        &BASE64_STANDARD,
        &BASE64_STANDARD_NO_PAD,
        &URL_SAFE,
        &URL_SAFE_NO_PAD,
    ]
    .iter()
    .find_map(|engine| engine.decode(trimmed).ok())
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).ok())
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn cleanup_download_artifacts(path: &Path) -> Result<()> {
    let partial_path = partial_download_path(path);
    for candidate in [partial_path.as_path(), path] {
        if candidate.exists() {
            fs::remove_file(candidate)?;
        }
    }
    Ok(())
}

async fn download_pdf(
    title_id: &str,
    input_id: &str,
    url: &str,
    job_dir: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> Result<SupplementalAsset> {
    ensure_not_cancelled(is_cancelled)?;
    fs::create_dir_all(job_dir)?;
    let path = generated_staging_path(job_dir, "pdf");
    let mut ignore_progress = |_progress: AcquisitionProgress| {};
    download_to_path(url, &path, None, &mut ignore_progress, is_cancelled).await?;
    if let Err(error @ AppError::Cancellation(_)) = ensure_not_cancelled(is_cancelled) {
        cleanup_download_artifacts(&path)?;
        return Err(error);
    }
    let bytes = fs::read(&path)?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(AppError::FileValidation(
            "Downloaded Supplemental PDF did not pass PDF magic-byte validation.".to_string(),
        ));
    }
    let metadata = fs::metadata(&path)?;
    if metadata.len() > MAX_SUPPLEMENTAL_PDF_BYTES {
        return Err(AppError::FileValidation(
            "Downloaded Supplemental PDF exceeds the 100 MiB size limit.".to_string(),
        ));
    }
    let path = path.canonicalize().map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot canonicalize Supplemental PDF source '{}': {}",
            sanitize_path_for_display(&path),
            error
        ))
    })?;
    Ok(SupplementalAsset {
        asset_id: uuid::Uuid::new_v4().to_string(),
        input_id: input_id.to_string(),
        title_id: title_id.to_string(),
        path,
        file_name: "Supplemental PDF.pdf".to_string(),
        size_bytes: metadata.len(),
        sha256: sha256_bytes(&bytes),
    })
}

async fn download_to_path(
    url: &str,
    path: &Path,
    log_context: Option<DownloadLogContext<'_>>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<u64> {
    ensure_not_cancelled(is_cancelled)?;
    let parsed = reqwest::Url::parse(url).map_err(|_| provider_private_failure("download URL"))?;
    if parsed.scheme() != "https" {
        return Err(AppError::InvalidInput(
            "Remote source download URL must use https.".to_string(),
        ));
    }

    let partial_path = partial_download_path(path);
    let _ = tokio::fs::remove_file(&partial_path).await;
    let bytes =
        match download_to_partial_path(parsed, &partial_path, log_context, progress, is_cancelled)
            .await
        {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = tokio::fs::remove_file(&partial_path).await;
                return Err(error);
            }
        };
    if let Err(error @ AppError::Cancellation(_)) = ensure_not_cancelled(is_cancelled) {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(error);
    }
    tokio::fs::rename(&partial_path, path).await?;
    Ok(bytes)
}

fn license_request_payload() -> Value {
    json!({
        "quality": "High",
        "response_groups": "last_position_heard,pdf_url,content_reference,chapter_info",
        "consumption_type": "Download",
        "tenant_id": "Audible",
        "spatial": false,
        "supported_media_features": {
            "drm_types": ["Adrm", "Mpeg"],
            "codecs": ["mp4a.40.2"],
            "chapter_titles_type": "Tree",
            "previews": false,
            "catalog_samples": false
        }
    })
}

struct LicenseRequestSpec<'a> {
    method: &'static str,
    url: String,
    body: &'a Value,
}

fn license_request_spec<'a>(title_id: &str, payload: &'a Value) -> LicenseRequestSpec<'a> {
    LicenseRequestSpec {
        method: "POST",
        url: format!("https://api.audible.{DOMAIN}/1.0/content/{title_id}/licenserequest"),
        body: payload,
    }
}

fn build_license_request(title_id: &str, payload: &Value) -> Result<audible_reqwest::Request> {
    let spec = license_request_spec(title_id, payload);
    let request = match spec.method {
        "POST" => audible_reqwest::Client::new()
            .post(spec.url)
            .json(spec.body),
        _ => return Err(provider_private_failure("license request method")),
    };
    request
        .build()
        .map_err(|_| provider_private_failure("license request construction"))
}

async fn post_license_request_json(
    client: &AudibleClient,
    title_id: &str,
    payload: &Value,
    job_id: &str,
) -> Result<Value> {
    log::info!(
        "remote_source audible stage=license_request_start job_id={} title_ref={} body=json",
        job_id,
        title_ref(title_id)
    );
    let request = build_license_request(title_id, payload)?;
    let response = client.send_request(request).await.map_err(|_| {
        log::warn!(
            "remote_source audible stage=license_request_failed job_id={} title_ref={} failure=send_request",
            job_id,
            title_ref(title_id)
        );
        provider_private_failure("license request")
    })?;
    let status = response.status();
    log::info!(
        "remote_source audible stage=license_request_status job_id={} title_ref={} http_status={}",
        job_id,
        title_ref(title_id),
        status.as_u16()
    );
    if !status.is_success() {
        return Err(AppError::General(format!(
            "Audible license request returned HTTP {}. Check application logs for sanitized acquisition facts.",
            status.as_u16()
        )));
    }
    response.json().await.map_err(|_| {
        log::warn!(
            "remote_source audible stage=license_response_failed job_id={} title_ref={} failure=json_parse",
            job_id,
            title_ref(title_id)
        );
        provider_private_failure("license response parse")
    })
}

fn log_license_facts(
    job_id: &str,
    title_id: &str,
    facts: &LicenseFacts,
    strategy: AcquisitionStrategy,
) {
    log::info!(
        "remote_source audible stage=license_classification job_id={} title_ref={} content_url_present={} container={:?} protection={:?} drm={} decryption_material_present={} supplemental_pdf_present={} strategy={}",
        job_id,
        title_ref(title_id),
        facts.content_url.is_some(),
        facts.media_container,
        facts.media_protection,
        drm_log_label(facts.drm_kind.as_deref()),
        facts.decryption_material_present,
        facts.supplemental_pdf_url.is_some(),
        strategy_label(strategy)
    );
}

fn drm_log_label(drm_kind: Option<&str>) -> &'static str {
    match drm_kind {
        Some(kind) if kind.eq_ignore_ascii_case("widevine") => "widevine",
        Some(_) => "present",
        None => "absent",
    }
}

fn strategy_label(strategy: AcquisitionStrategy) -> &'static str {
    match strategy {
        AcquisitionStrategy::DownloadImportReady => "import-ready-m4b",
        AcquisitionStrategy::DownloadThenDecryptAax => "aax-requires-materializer",
        AcquisitionStrategy::DownloadThenDecryptAaxc => "aaxc-requires-materializer",
        AcquisitionStrategy::DownloadThenDecryptDash => "dash-requires-materializer",
        AcquisitionStrategy::ProtectedUnsupported => "protected-unsupported",
        AcquisitionStrategy::ProviderProtocolFailed => "provider-protocol-failed",
    }
}

fn title_ref(title_id: &str) -> String {
    sha256_bytes(title_id.as_bytes()).chars().take(12).collect()
}

async fn download_to_partial_path(
    url: reqwest::Url,
    path: &Path,
    log_context: Option<DownloadLogContext<'_>>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<u64> {
    ensure_not_cancelled(is_cancelled)?;
    let client = remote_download_client()?;
    let mut bytes_downloaded = 0_u64;
    let mut bytes_total = None;
    let mut first_bytes_logged = false;
    let can_resume = log_context.is_some();

    for attempt in 0..MAX_DOWNLOAD_ATTEMPTS {
        ensure_not_cancelled(is_cancelled)?;
        log_download_request_start(log_context, bytes_downloaded);
        let request = if log_context.is_some() {
            build_download_request(&client, url.clone(), bytes_downloaded)
        } else {
            client.get(url.clone())
        };
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(_) => {
                log_download_failed(log_context, "request", bytes_downloaded, bytes_total, None);
                return Err(download_failure("request"));
            }
        };
        ensure_not_cancelled(is_cancelled)?;

        let status = response.status();
        let content_range = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok());
        let final_url_is_https = response.url().scheme() == "https";
        let response_total = match classify_download_response_for_mode(
            log_context.is_some(),
            status,
            final_url_is_https,
            bytes_downloaded,
            response.content_length(),
            content_range,
        ) {
            Ok(total) => total,
            Err(error) => {
                log_download_failed(
                    log_context,
                    "status",
                    bytes_downloaded,
                    bytes_total,
                    Some(status),
                );
                return Err(error);
            }
        };
        bytes_total = response_total.or(bytes_total);
        log_download_request_status(log_context, status, bytes_downloaded, bytes_total);

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let mut read_failed = false;
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    read_failed = true;
                    break;
                }
            };
            ensure_not_cancelled(is_cancelled)?;
            if chunk.is_empty() {
                continue;
            }
            bytes_downloaded += chunk.len() as u64;
            if !first_bytes_logged {
                first_bytes_logged = true;
                log_download_progress_first_bytes(log_context, bytes_downloaded, bytes_total);
            }
            file.write_all(&chunk).await?;
            let fraction = bytes_total
                .filter(|total| *total > 0)
                .map(|total| bytes_downloaded as f32 / total as f32)
                .unwrap_or(0.2);
            progress(acquisition_progress(
                AcquisitionStage::Download,
                Some(fraction),
                Some(bytes_downloaded),
                bytes_total,
            ));
        }
        file.sync_all().await?;

        if read_failed {
            if can_resume && attempt + 1 < MAX_DOWNLOAD_ATTEMPTS {
                continue;
            }
            log_download_failed(log_context, "read", bytes_downloaded, bytes_total, None);
            return Err(download_failure("read"));
        }
        ensure_not_cancelled(is_cancelled)?;
        if bytes_total.is_none_or(|total| bytes_downloaded >= total) && bytes_downloaded > 0 {
            return Ok(bytes_downloaded);
        }
        if can_resume && attempt + 1 < MAX_DOWNLOAD_ATTEMPTS {
            continue;
        }
        break;
    }

    log_download_failed(
        log_context,
        "incomplete",
        bytes_downloaded,
        bytes_total,
        None,
    );
    Err(download_failure("incomplete"))
}

fn build_download_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    offset: u64,
) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header(USER_AGENT, AUDIBLE_DOWNLOAD_USER_AGENT)
        .header(RANGE, format!("bytes={offset}-"))
}

fn classify_download_response(
    status: reqwest::StatusCode,
    final_url_is_https: bool,
    offset: u64,
    content_length: Option<u64>,
    content_range: Option<&str>,
) -> Result<Option<u64>> {
    if !final_url_is_https {
        return Err(AppError::InvalidInput(
            "Remote source download redirect must use https.".to_string(),
        ));
    }
    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        if let Some(range) = content_range.and_then(parse_content_range) {
            if range.start != offset || range.end < range.start {
                return Err(download_failure("content range"));
            }
            return Ok(range
                .total
                .or_else(|| content_length.map(|length| offset + length)));
        }
        return Ok(content_length.map(|length| offset + length));
    }
    if status == reqwest::StatusCode::OK && offset == 0 && content_length.is_some() {
        return Ok(content_length);
    }
    Err(download_status_failure(status))
}

fn classify_download_response_for_mode(
    licensed_audio: bool,
    status: reqwest::StatusCode,
    final_url_is_https: bool,
    offset: u64,
    content_length: Option<u64>,
    content_range: Option<&str>,
) -> Result<Option<u64>> {
    if licensed_audio {
        return classify_download_response(
            status,
            final_url_is_https,
            offset,
            content_length,
            content_range,
        );
    }
    if !final_url_is_https {
        return Err(AppError::InvalidInput(
            "Remote source download redirect must use https.".to_string(),
        ));
    }
    if status.is_success() {
        return Ok(content_length.map(|length| offset + length));
    }
    Err(download_status_failure(status))
}

fn parse_content_range(header: &str) -> Option<ParsedContentRange> {
    let range = header.trim().strip_prefix("bytes ")?;
    let (bounds, total_raw) = range.split_once('/')?;
    let (start_raw, end_raw) = bounds.split_once('-')?;
    let start = start_raw.parse().ok()?;
    let end = end_raw.parse().ok()?;
    let total = if total_raw == "*" {
        None
    } else {
        Some(total_raw.parse().ok()?)
    };
    Some(ParsedContentRange { start, end, total })
}

fn log_download_request_start(context: Option<DownloadLogContext<'_>>, offset: u64) {
    let Some(context) = context else {
        return;
    };
    log::info!(
        "remote_source audible stage=download_request_start job_id={} title_ref={} extension={} bytes={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        offset
    );
}

fn log_download_request_status(
    context: Option<DownloadLogContext<'_>>,
    status: reqwest::StatusCode,
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
) {
    let Some(context) = context else {
        return;
    };
    log::info!(
        "remote_source audible stage=download_request_status job_id={} title_ref={} extension={} http_status={} bytes={} bytes_total={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        status.as_u16(),
        bytes_downloaded,
        bytes_total.unwrap_or(0)
    );
}

fn log_download_progress_first_bytes(
    context: Option<DownloadLogContext<'_>>,
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
) {
    let Some(context) = context else {
        return;
    };
    log::info!(
        "remote_source audible stage=download_progress_first_bytes job_id={} title_ref={} extension={} bytes={} bytes_total={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        bytes_downloaded,
        bytes_total.unwrap_or(0)
    );
}

fn log_download_failed(
    context: Option<DownloadLogContext<'_>>,
    category: &str,
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
    status: Option<reqwest::StatusCode>,
) {
    let Some(context) = context else {
        return;
    };
    log::warn!(
        "remote_source audible stage=download_failed job_id={} title_ref={} extension={} category={} http_status={} bytes={} bytes_total={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        category,
        status.map(|status| status.as_u16()).unwrap_or(0),
        bytes_downloaded,
        bytes_total.unwrap_or(0)
    );
}

fn remote_download_client() -> Result<reqwest::Client> {
    let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_DOWNLOAD_REDIRECTS {
            return attempt.error("remote source download exceeded redirect limit");
        }
        if attempt.url().scheme() != "https" {
            return attempt.error("remote source download redirect must use https");
        }
        attempt.follow()
    });
    reqwest::Client::builder()
        .redirect(redirect_policy)
        .build()
        .map_err(|_| provider_private_failure("download client"))
}

fn generated_staging_path(job_dir: &Path, extension: &str) -> std::path::PathBuf {
    job_dir.join(format!("{}.{}", uuid::Uuid::new_v4(), extension))
}

fn partial_download_path(path: &Path) -> std::path::PathBuf {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| format!("{extension}.partial"))
        .unwrap_or_else(|| "partial".to_string());
    path.with_extension(extension)
}

fn find_first_string_for_key(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(Value::as_str) {
                return Some(found.to_string());
            }
            map.values()
                .find_map(|entry| find_first_string_for_key(entry, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_first_string_for_key(entry, key)),
        _ => None,
    }
}

fn find_first_string_for_keys(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| find_first_string_for_key(value, key))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::BlockEncryptMut;

    type Aes128CbcEnc = cbc::Encryptor<Aes128>;

    fn fixture_decrypt_context() -> AudibleLicenseDecryptContext {
        AudibleLicenseDecryptContext {
            device_type: AUDIBLE_IOS_DEVICE_TYPE.to_string(),
            device_serial: "device-serial".to_string(),
            amazon_account_id: "account-1".to_string(),
        }
    }

    fn encrypted_license_response(
        voucher: Value,
        asin: &str,
        context: &AudibleLicenseDecryptContext,
    ) -> String {
        let mut plaintext = serde_json::to_vec(&voucher).expect("voucher json");
        let padded_len = plaintext.len().next_multiple_of(16);
        plaintext.resize(padded_len, 0);
        let key_components = format!(
            "{}{}{}{}",
            context.device_type, context.device_serial, context.amazon_account_id, asin
        );
        let hash = Sha256::digest(key_components.as_bytes());
        let (key, iv) = hash.split_at(16);
        let ciphertext = Aes128CbcEnc::new_from_slices(key, iv)
            .expect("cipher")
            .encrypt_padded_mut::<NoPadding>(&mut plaintext, padded_len)
            .expect("encrypt");
        BASE64_STANDARD.encode(ciphertext)
    }

    #[test]
    fn capabilities_stay_provider_neutral() {
        let capabilities = AudibleProvider::capabilities();

        assert_eq!(capabilities.provider_id, ProviderId::Audible);
        assert_eq!(
            capabilities.auth_flow,
            RemoteAuthFlow::ExternalBrowserHandoff
        );
        assert!(capabilities.supports_library_scan);
        assert!(capabilities.supports_supplemental_pdf);
        assert!(capabilities.supports_materialized_audio);
        assert!(capabilities
            .known_unsupported_reasons
            .contains(&RemoteAcquisitionFailureKind::ProtectedUnsupported));
    }

    #[test]
    fn pdf_hash_uses_sha256_hex() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
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
        let materialized_path = staged_materialized_path(&item_dir);

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
            Some("materialized.m4b")
        );
    }

    #[test]
    fn partial_download_path_keeps_final_extension_visible() {
        let path = Path::new("/tmp/book.m4b");

        assert_eq!(
            partial_download_path(path),
            Path::new("/tmp/book.m4b.partial")
        );
    }

    #[tokio::test]
    async fn download_to_path_rejects_cleartext_urls_without_fetching() {
        let root = tempfile::TempDir::new().expect("temp root");
        let target = root.path().join("book.m4b");
        let mut ignore_progress = |_progress: AcquisitionProgress| {};

        let error = download_to_path(
            "http://provider.example/book.m4b?token=fake-secret",
            &target,
            None,
            &mut ignore_progress,
            &|| false,
        )
        .await
        .expect_err("cleartext URL rejected");

        assert!(error.to_string().contains("must use https"));
        assert!(!error.to_string().contains("fake-secret"));
        assert!(!target.exists());
        assert!(!partial_download_path(&target).exists());
    }

    #[test]
    fn licensed_audio_download_request_uses_range_and_audible_user_agent() {
        let client = reqwest::Client::new();
        let request = build_download_request(
            &client,
            reqwest::Url::parse("https://cdn.example.test/book.aax").expect("url"),
            4096,
        )
        .build()
        .expect("request");

        assert_eq!(
            request
                .headers()
                .get(RANGE)
                .and_then(|value| value.to_str().ok()),
            Some("bytes=4096-")
        );
        assert_eq!(
            request
                .headers()
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some(AUDIBLE_DOWNLOAD_USER_AGENT)
        );
    }

    #[test]
    fn partial_content_status_uses_content_range_total() {
        let total = classify_download_response(
            reqwest::StatusCode::PARTIAL_CONTENT,
            true,
            4096,
            Some(2048),
            Some("bytes 4096-6143/8192"),
        )
        .expect("partial content");

        assert_eq!(total, Some(8192));
    }

    #[test]
    fn initial_ok_status_requires_usable_content_length() {
        let total = classify_download_response(reqwest::StatusCode::OK, true, 0, Some(42), None)
            .expect("initial ok");

        assert_eq!(total, Some(42));
        assert!(
            classify_download_response(reqwest::StatusCode::OK, true, 7, Some(42), None).is_err()
        );
        assert!(classify_download_response(reqwest::StatusCode::OK, true, 0, None, None).is_err());
    }

    #[test]
    fn download_status_errors_map_to_download_failure_kind() {
        let error = download_status_failure(reqwest::StatusCode::FORBIDDEN);

        assert_eq!(
            diagnostic_kind_for_acquire_error(&error),
            RemoteAcquisitionFailureKind::DownloadFailed
        );
        assert!(!error.to_string().contains("token"));
        assert!(!error.to_string().contains("license"));
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
    fn auth_code_extraction_error_does_not_expose_response_url() {
        let response_url = "not-a-url token=fake-secret license=fake-license";

        let error = extract_audible_auth_code(response_url).expect_err("invalid response URL");
        let message = error.to_string();

        assert!(message.contains("Audible auth response URL was not accepted"));
        assert!(!message.contains("fake-secret"));
        assert!(!message.contains("fake-license"));
        assert!(!message.contains(response_url));
    }

    #[test]
    fn download_extension_matches_core_strategy() {
        assert_eq!(
            download_extension_for_strategy(
                abb_remote_source_core::AcquisitionStrategy::DownloadImportReady
            ),
            "m4b"
        );
        assert_eq!(
            download_extension_for_strategy(
                abb_remote_source_core::AcquisitionStrategy::DownloadThenDecryptAax
            ),
            "aax"
        );
        assert_eq!(
            download_extension_for_strategy(
                abb_remote_source_core::AcquisitionStrategy::DownloadThenDecryptAaxc
            ),
            "aaxc"
        );
        assert_eq!(
            download_extension_for_strategy(
                abb_remote_source_core::AcquisitionStrategy::DownloadThenDecryptDash
            ),
            "mpd"
        );
    }

    #[test]
    fn license_request_matches_aaxc_adaptive_license_shape() {
        let payload = license_request_payload();
        let spec = license_request_spec("B000000001", &payload);

        assert_eq!(spec.method, "POST");
        assert_eq!(
            spec.url,
            format!("https://api.audible.{DOMAIN}/1.0/content/B000000001/licenserequest")
        );
        assert!(!spec.url.contains('?'));
        assert_eq!(spec.body["quality"], "High");
        assert_eq!(spec.body["consumption_type"], "Download");
        assert_eq!(spec.body["tenant_id"], "Audible");
        assert_eq!(
            spec.body["response_groups"],
            "last_position_heard,pdf_url,content_reference,chapter_info"
        );
        assert_eq!(
            spec.body["supported_media_features"]["drm_types"][0],
            "Adrm"
        );
        assert_eq!(
            spec.body["supported_media_features"]["drm_types"][1],
            "Mpeg"
        );
        assert_eq!(
            spec.body["supported_media_features"]["codecs"][0],
            "mp4a.40.2"
        );
        assert_eq!(
            spec.body["supported_media_features"]["chapter_titles_type"],
            "Tree"
        );
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
    fn aax_license_response_decrypts_adrm_voucher_activation_bytes() {
        let context = fixture_decrypt_context();
        let asin = "B000000001";
        let response = json!({
            "content_license": {
                "asin": asin,
                "content_url": "https://cdn.example.test/book.aax",
                "drm_type": "Adrm",
                "license_response": encrypted_license_response(
                    json!({ "key": [10, 27, 44, 61] }),
                    asin,
                    &context
                )
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAax,
            asin,
            Some(&context),
        )
        .expect("aax material");

        match material {
            AudibleDecryptionMaterial::Aax {
                activation_bytes_hex,
            } => assert_eq!(activation_bytes_hex.expose_secret(), "0a1b2c3d"),
            AudibleDecryptionMaterial::Aaxc { .. } => panic!("unexpected aaxc material"),
        }
    }

    #[test]
    fn aaxc_license_response_decrypts_adrm_voucher_key_iv() {
        let context = fixture_decrypt_context();
        let asin = "B000000002";
        let response = json!({
            "content_license": {
                "Asin": asin,
                "content_url": "https://cdn.example.test/book.aaxc",
                "drm_type": "Adrm",
                "licenseResponse": encrypted_license_response(
                    json!({
                        "key": "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d",
                        "iv": BASE64_STANDARD.encode([
                            0x2e, 0x2f, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
                            0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f, 0x5a, 0x5b
                        ])
                    }),
                    asin,
                    &context
                )
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAaxc,
            asin,
            Some(&context),
        )
        .expect("aaxc material");

        match material {
            AudibleDecryptionMaterial::Aaxc { key_hex, iv_hex } => {
                assert_eq!(key_hex.expose_secret(), "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d");
                assert_eq!(iv_hex.expose_secret(), "2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b");
            }
            AudibleDecryptionMaterial::Aax { .. } => panic!("unexpected aax material"),
        }
    }

    #[test]
    fn aax_license_material_extracts_activation_bytes_without_public_surface() {
        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/book.aax",
                "drm_type": "Mpeg",
                "voucher": {
                    "key": [10, 27, 44, 61]
                }
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAax,
            "B000000001",
            None,
        )
        .expect("aax material");

        match material {
            AudibleDecryptionMaterial::Aax {
                activation_bytes_hex,
            } => assert_eq!(activation_bytes_hex.expose_secret(), "0a1b2c3d"),
            AudibleDecryptionMaterial::Aaxc { .. } => panic!("unexpected aaxc material"),
        }
    }

    #[test]
    fn aax_license_material_extracts_url_safe_unpadded_voucher_key() {
        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/book.aax",
                "drm_type": "Mpeg",
                "voucher": {
                    "Key": URL_SAFE_NO_PAD.encode([0xfb, 0xff, 0xee, 0xdd])
                }
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAax,
            "B000000001",
            None,
        )
        .expect("aax material");

        match material {
            AudibleDecryptionMaterial::Aax {
                activation_bytes_hex,
            } => assert_eq!(activation_bytes_hex.expose_secret(), "fbffeedd"),
            AudibleDecryptionMaterial::Aaxc { .. } => panic!("unexpected aaxc material"),
        }
    }

    #[test]
    fn aaxc_license_material_extracts_key_and_iv_from_hex_or_base64() {
        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/book.aaxc",
                "drm_type": "Mpeg",
                "voucher": {
                    "key": "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d",
                    "iv": BASE64_STANDARD.encode([
                        0x2e, 0x2f, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
                        0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f, 0x5a, 0x5b
                    ])
                }
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAaxc,
            "B000000001",
            None,
        )
        .expect("aaxc material");

        match material {
            AudibleDecryptionMaterial::Aaxc { key_hex, iv_hex } => {
                assert_eq!(key_hex.expose_secret(), "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d");
                assert_eq!(iv_hex.expose_secret(), "2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b");
            }
            AudibleDecryptionMaterial::Aax { .. } => panic!("unexpected aax material"),
        }
    }

    #[test]
    fn helper_lane_follows_voucher_key_shape_not_content_url_extension() {
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

    #[test]
    fn dash_lane_is_reported_unsupported_not_deferred() {
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/manifest.mpd".to_string(),
            strategy: AcquisitionStrategy::DownloadThenDecryptDash,
            decryption_material: None,
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
}
