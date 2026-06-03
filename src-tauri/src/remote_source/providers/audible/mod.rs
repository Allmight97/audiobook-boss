use std::fs;
use std::path::Path;

use abb_remote_source_core::{
    acquisition_progress, choose_acquisition_strategy, license_facts_from_value,
    strategy_allows_import_handoff, AcquisitionProgress, AcquisitionStage, AcquisitionStrategy,
    LicenseFacts, MaterializedSourceKind,
};
use audible_api::api::Client as AudibleClient;
use audible_api::auth::oauth::{build_oauth_url, extract_auth_code};
use audible_api::auth::register::register;
use audible_api::auth::{localization, Auth};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

mod library;

use library::parse_library_titles;

use crate::audio;
use crate::errors::{sanitize_path_for_display, AppError, Result};
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

struct AcquiredTitle {
    file: Option<MaterializedSourceFile>,
    assets: Vec<SupplementalAsset>,
    diagnostics: Vec<RemoteSourceDiagnostic>,
}

struct LicenseLane {
    content_url: String,
    strategy: AcquisitionStrategy,
}

fn provider_private_failure(stage: &str) -> AppError {
    AppError::General(format!(
        "Audible {stage} failed. Provider-private details were withheld from UI and logs."
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
        plan: &AcquisitionPlan,
        job_id: &str,
        job_dir: &Path,
        mut progress: impl FnMut(AcquisitionProgress),
        is_cancelled: impl Fn() -> bool,
    ) -> Result<AcquisitionJob> {
        ensure_not_cancelled(&is_cancelled)?;
        let client = client_from_vault(vault)?;
        let mut job = AcquisitionJob {
            job_id: job_id.to_string(),
            provider_id: ProviderId::Audible,
            status: RemoteAcquisitionStatus::Acquiring,
            progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
            materialized_files: Vec::new(),
            supplemental_assets: Vec::new(),
            diagnostics: Vec::new(),
        };

        for selection in &plan.selections {
            ensure_not_cancelled(&is_cancelled)?;
            match acquire_one(
                &client,
                &selection.title_id,
                selection.include_supplemental_pdf,
                job_id,
                job_dir,
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
                    job.diagnostics.push(RemoteSourceDiagnostic {
                        kind: RemoteAcquisitionFailureKind::MaterializationFailed,
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

fn client_from_vault(vault: &dyn SecretVault) -> Result<AudibleClient> {
    let secret = vault
        .get_secret(AUTH_SECRET_KEY)?
        .ok_or_else(|| AppError::InvalidInput("Audible account is not connected.".to_string()))?;
    let auth: Auth = serde_json::from_str(secret.expose_secret()).map_err(|error| {
        AppError::ResourceCleanup(format!("Stored Audible auth is invalid: {error}"))
    })?;
    AudibleClient::new(auth)
        .map_err(|error| AppError::General(format!("Failed to create Audible client: {error}")))
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
    title_id: &str,
    include_pdf: bool,
    job_id: &str,
    job_dir: &Path,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<AcquiredTitle> {
    ensure_not_cancelled(is_cancelled)?;
    log::info!(
        "remote_source audible stage=title_start job_id={} title_ref={} include_pdf={}",
        job_id,
        title_ref(title_id),
        include_pdf
    );
    progress(acquisition_progress(
        AcquisitionStage::License,
        Some(0.05),
        None,
        None,
    ));
    let supplemental_pdf_url =
        lookup_supplemental_pdf_url(client, title_id, include_pdf, is_cancelled).await?;
    let lane = request_license_lane(client, title_id, job_id, progress, is_cancelled).await?;
    if let Some(deferred) = deferred_result_for_non_import_ready_lane(title_id, job_id, &lane) {
        return Ok(deferred);
    }

    let downloaded_path = download_audio(
        &lane.content_url,
        download_extension_for_strategy(lane.strategy),
        job_id,
        title_id,
        job_dir,
        progress,
        is_cancelled,
    )
    .await?;
    ensure_not_cancelled(is_cancelled)?;
    progress(acquisition_progress(
        AcquisitionStage::Validation,
        Some(0.1),
        None,
        None,
    ));
    let file = validate_import_ready_download(title_id, &downloaded_path, lane.strategy, progress)?;
    let (assets, diagnostics) = download_supplemental_pdf_if_requested(
        title_id,
        &file,
        supplemental_pdf_url,
        job_dir,
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
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<LicenseLane> {
    ensure_not_cancelled(is_cancelled)?;
    progress(acquisition_progress(
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
        });
    };

    Ok(LicenseLane {
        content_url: content_url.to_string(),
        strategy,
    })
}

fn deferred_result_for_non_import_ready_lane(
    title_id: &str,
    job_id: &str,
    lane: &LicenseLane,
) -> Option<AcquiredTitle> {
    let strategy = lane.strategy;
    match strategy {
        AcquisitionStrategy::DownloadImportReady => None,
        AcquisitionStrategy::DownloadThenDecryptAax
        | AcquisitionStrategy::DownloadThenDecryptAaxc
        | AcquisitionStrategy::DownloadThenDecryptDash => {
            let lane = strategy_label(strategy);
            log::info!(
                "remote_source audible stage=materializer_selection job_id={} title_ref={} lane={} action=deferred",
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
                        "Audible returned {lane}; ABB has not selected the materializer implementation yet."
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

fn validate_import_ready_download(
    title_id: &str,
    downloaded_path: &Path,
    strategy: AcquisitionStrategy,
    progress: &mut impl FnMut(AcquisitionProgress),
) -> Result<MaterializedSourceFile> {
    let file = match materialized_file_from_downloaded_path(title_id, downloaded_path, strategy) {
        Ok(file) => file,
        Err(error) => {
            cleanup_download_artifacts(downloaded_path)?;
            progress(acquisition_progress(
                AcquisitionStage::Failed,
                Some(1.0),
                None,
                None,
            ));
            return Err(error);
        }
    };
    progress(acquisition_progress(
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
    extension: &str,
    job_id: &str,
    title_id: &str,
    job_dir: &Path,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<std::path::PathBuf> {
    ensure_not_cancelled(is_cancelled)?;
    fs::create_dir_all(job_dir)?;
    let path = generated_staging_path(job_dir, extension);
    log::info!(
        "remote_source audible stage=download_start job_id={} title_ref={} extension={}",
        job_id,
        title_ref(title_id),
        extension
    );
    download_to_path(content_url, &path, progress, is_cancelled).await?;
    if let Err(error @ AppError::Cancellation(_)) = ensure_not_cancelled(is_cancelled) {
        cleanup_download_artifacts(&path)?;
        return Err(error);
    }
    log::info!(
        "remote_source audible stage=download_complete job_id={} title_ref={} extension={}",
        job_id,
        title_ref(title_id),
        extension
    );
    Ok(path)
}

fn materialized_file_from_downloaded_path(
    title_id: &str,
    path: &Path,
    strategy: AcquisitionStrategy,
) -> Result<MaterializedSourceFile> {
    let source_kind = abb_remote_source_core::classify_materialized_source_path(path);
    if !strategy_allows_import_handoff(strategy, source_kind) {
        return Err(AppError::FileValidation(format!(
            "Downloaded Audible {} requires Audible decryption before ABB import handoff.",
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
    download_to_path(url, &path, &mut ignore_progress, is_cancelled).await?;
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
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<()> {
    ensure_not_cancelled(is_cancelled)?;
    let parsed = reqwest::Url::parse(url).map_err(|_| provider_private_failure("download URL"))?;
    if parsed.scheme() != "https" {
        return Err(AppError::InvalidInput(
            "Remote source download URL must use https.".to_string(),
        ));
    }

    let partial_path = partial_download_path(path);
    let result = download_to_partial_path(parsed, &partial_path, progress, is_cancelled).await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(error);
    }
    if let Err(error @ AppError::Cancellation(_)) = ensure_not_cancelled(is_cancelled) {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(error);
    }
    tokio::fs::rename(&partial_path, path).await?;
    Ok(())
}

fn license_request_payload() -> Value {
    json!({
        "quality": "High",
        "response_groups": "chapter_info,content_reference,last_position_heard,pdf_url,ad_insertion,certificate",
        "consumption_type": "Download",
        "supported_media_features": {
            "codecs": ["mp4a.40.2", "mp4a.40.42"],
            "drm_types": ["Mpeg"]
        },
        "spatial": false
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
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<()> {
    ensure_not_cancelled(is_cancelled)?;
    let client = remote_download_client()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| provider_private_failure("download request"))?;
    ensure_not_cancelled(is_cancelled)?;
    if !response.status().is_success() {
        return Err(AppError::General(format!(
            "Remote source download returned HTTP {}",
            response.status()
        )));
    }
    let bytes_total = response.content_length();
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    let mut response = response;
    let mut bytes_downloaded = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| provider_private_failure("download read"))?
    {
        ensure_not_cancelled(is_cancelled)?;
        bytes_downloaded += chunk.len() as u64;
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
    ensure_not_cancelled(is_cancelled)?;
    file.sync_all().await?;
    Ok(())
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
    fn license_request_uses_json_body_not_query_payload() {
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
        assert!(spec.body["supported_media_features"].is_object());
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
