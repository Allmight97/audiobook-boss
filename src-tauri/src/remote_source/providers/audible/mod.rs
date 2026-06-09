use std::collections::HashSet;
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
use audible_api::auth::oauth::{build_oauth_url, extract_auth_code};
use audible_api::auth::register::register;
use audible_api::auth::{localization, Auth};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

mod audio_download;
mod diagnostics;
mod library;
mod license;
mod materialization;
mod supplemental_pdf;

use audio_download::{cleanup_download_artifacts, download_audio};
use diagnostics::AudibleAcquisitionError;
use library::parse_library_titles;
use license::{
    license_decrypt_context_from_auth, lookup_title_details, provider_protocol_lane_message,
    request_license_lane, strategy_label, AudibleTitleDetails, LicenseLane,
};
use materialization::materialize_protected_download;
use supplemental_pdf::{
    download_supplemental_pdf, log_supplemental_pdf_failed, supplemental_pdf_failure_message,
    SupplementalPdfRequest,
};

use crate::audio;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::remote_source::materializer::AaxcleanMaterializer;
use crate::remote_source::staging;
use crate::remote_source::vault::SecretVault;
use crate::remote_source::{
    AccountRef, AcquisitionJob, AcquisitionPlan, MaterializedSourceFile, ProviderId,
    RemoteAccountStatus, RemoteAcquisitionFailureKind, RemoteAcquisitionStatus, RemoteAuthFlow,
    RemoteLibraryResponse, RemoteSourceAccountState, RemoteSourceDiagnostic,
    RemoteSourceProviderCapabilities, RemoteTitle, SupplementalAsset,
};

const COUNTRY_CODE: &str = "us";
const DOMAIN: &str = "com";
const MARKETPLACE_ID: &str = "ATVPDKIKX0DER";
const ACCOUNT_ID: &str = "audible-us";
const AUTH_SECRET_KEY: &str = "audible.us.auth";
const AUDIBLE_LIBRARY_PAGE_SIZE: u16 = 1000;
const MAX_AUDIBLE_LIBRARY_PAGES: u16 = 100;
const MAX_DOWNLOAD_REDIRECTS: usize = 5;
const MAX_DOWNLOAD_ATTEMPTS: usize = 4;
const AUDIBLE_DOWNLOAD_USER_AGENT: &str = "Audible/671 CFNetwork/1240.0.4 Darwin/20.6.0";
const AUDIBLE_IOS_DEVICE_TYPE: &str = "A2CZJZGLK2JJVM";

struct AcquiredTitle {
    file: Option<MaterializedSourceFile>,
    assets: Vec<SupplementalAsset>,
    diagnostics: Vec<RemoteSourceDiagnostic>,
}

type AudibleAcquisitionResult<T> = std::result::Result<T, AudibleAcquisitionError>;

#[derive(Clone, Copy)]
struct TitleProgressContext<'a> {
    title_id: &'a str,
    item_index: u32,
    total_items: u32,
}

/// Per-title request handed to `acquire_one`: the identity and policy inputs for
/// one selection, bundled so the acquisition entry point stays under the param
/// budget.
struct TitleAcquisitionRequest<'a> {
    title_id: &'a str,
    include_pdf: bool,
    job_id: &'a str,
    job_dir: &'a Path,
    license_decrypt_context: Option<&'a AudibleLicenseDecryptContext>,
    progress_context: TitleProgressContext<'a>,
}

/// Identity/location threaded through the per-title acquisition helpers
/// (license, download, materialize, validate, supplemental PDF). Carries no
/// effect channels; `progress` and `is_cancelled` stay explicit parameters.
#[derive(Clone, Copy)]
struct TitleAcquisitionCtx<'a> {
    job_id: &'a str,
    title_id: &'a str,
    item_id: &'a str,
    item_dir: &'a Path,
    progress_context: TitleProgressContext<'a>,
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
        let titles = load_all_library_titles(&client).await?;
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
}

pub(super) async fn load_all_library_titles(client: &AudibleClient) -> Result<Vec<RemoteTitle>> {
    let mut titles = Vec::new();
    let mut seen_title_ids = HashSet::new();

    let first_payload = fetch_library_page(client, None).await?;
    let (first_count, first_new_count) =
        append_unique_library_titles(&mut titles, &mut seen_title_ids, &first_payload);
    log::info!(
        "remote_source audible stage=library_page page=default titles={} new_titles={}",
        first_count,
        first_new_count
    );
    if first_count < AUDIBLE_LIBRARY_PAGE_SIZE as usize {
        return Ok(titles);
    }

    for page in 1..=MAX_AUDIBLE_LIBRARY_PAGES {
        let payload = fetch_library_page(client, Some(page)).await?;
        let (page_count, new_count) =
            append_unique_library_titles(&mut titles, &mut seen_title_ids, &payload);
        log::info!(
            "remote_source audible stage=library_page page={} titles={} new_titles={}",
            page,
            page_count,
            new_count
        );

        if !should_continue_library_pagination(page, page_count, new_count) {
            return Ok(titles);
        }
    }

    Err(AppError::General(format!(
        "Audible library scan exceeded ABB's pagination safety limit of {MAX_AUDIBLE_LIBRARY_PAGES} pages. No partial library was loaded."
    )))
}

async fn fetch_library_page(client: &AudibleClient, page: Option<u16>) -> Result<Value> {
    client
        .get_library(Some(library_request_params(page)))
        .await
        .map_err(|_| provider_private_failure("library request"))
}

fn append_unique_library_titles(
    titles: &mut Vec<RemoteTitle>,
    seen_title_ids: &mut HashSet<String>,
    payload: &Value,
) -> (usize, usize) {
    let page_titles = parse_library_titles(payload);
    let page_count = page_titles.len();
    let mut new_count = 0;

    for title in page_titles {
        if seen_title_ids.insert(title.title_id.clone()) {
            titles.push(title);
            new_count += 1;
        }
    }

    (page_count, new_count)
}

fn should_continue_library_pagination(page: u16, page_count: usize, new_count: usize) -> bool {
    if page_count < AUDIBLE_LIBRARY_PAGE_SIZE as usize {
        return false;
    }

    page == 1 || new_count > 0
}

fn library_request_params(page: Option<u16>) -> Value {
    let mut params = json!({
        "num_results": AUDIBLE_LIBRARY_PAGE_SIZE,
        "response_groups": "product_desc,product_attrs,contributors,media,pdf_url,product_details,customer_rights,is_visible,is_playable,is_downloaded,is_finished,is_archived,is_returnable,origin_asin",
        "sort_by": "-PurchaseDate"
    });
    if let Some(page) = page {
        params["page"] = json!(page);
    }
    params
}

fn client_from_vault(vault: &dyn SecretVault) -> Result<AudibleClient> {
    client_from_auth(auth_from_vault(vault)?)
}

pub(super) fn auth_from_vault(vault: &dyn SecretVault) -> Result<Auth> {
    let secret = vault
        .get_secret(AUTH_SECRET_KEY)?
        .ok_or_else(|| AppError::InvalidInput("Audible account is not connected.".to_string()))?;
    serde_json::from_str(secret.expose_secret()).map_err(|error| {
        AppError::ResourceCleanup(format!("Stored Audible auth is invalid: {error}"))
    })
}

pub(super) fn client_from_auth(auth: Auth) -> Result<AudibleClient> {
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

fn supplemental_pdf_hint_present_for_acquisition(
    include_pdf: bool,
    title_details: &AudibleTitleDetails,
    lane: &LicenseLane,
) -> bool {
    include_pdf
        && (title_details.supplemental_pdf_url.is_some() || lane.supplemental_pdf_url.is_some())
}

fn requested_supplemental_pdf_is_required(include_pdf: bool, api_pdf_hint_present: bool) -> bool {
    include_pdf && api_pdf_hint_present
}

fn validate_materialized_audio(
    materialized_path: &Path,
    ctx: TitleAcquisitionCtx<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
) -> Result<MaterializedSourceFile> {
    let TitleAcquisitionCtx {
        title_id,
        progress_context,
        ..
    } = ctx;
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

fn required_supplemental_pdf_failure_message(
    failure: supplemental_pdf::SupplementalPdfFailure,
) -> String {
    format!(
        "{} The audiobook was not imported because the requested Supplemental PDF is required for this Audible title.",
        supplemental_pdf_failure_message(failure)
    )
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

fn staged_protected_source_path(
    job_dir: &Path,
    strategy: AcquisitionStrategy,
) -> std::path::PathBuf {
    job_dir.join(format!(
        "source.{}",
        download_extension_for_strategy(strategy)
    ))
}

fn staged_materialized_path(
    job_dir: &Path,
    title_name: Option<&str>,
    title_id: &str,
) -> std::path::PathBuf {
    job_dir.join(format!(
        "{}.m4b",
        remote_materialized_filename_stem(title_name, title_id)
    ))
}

fn generated_staging_path(job_dir: &Path, extension: &str) -> std::path::PathBuf {
    job_dir.join(format!("{}.{}", uuid::Uuid::new_v4(), extension))
}

fn sha256_file(path: &Path) -> Result<String> {
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
pub(super) struct LibraryProbeSummary {
    pub raw_items: usize,
    pub parsed_titles: usize,
    pub supplemental_pdf_available: usize,
    pub total_hint: Option<u64>,
    pub state_token_present: bool,
}

#[cfg(test)]
pub(super) fn library_probe_params(
    page: Option<u16>,
    status: Option<&str>,
    include_pending: Option<bool>,
) -> Value {
    let mut params = library_request_params(page);
    if let Some(status) = status {
        params["status"] = json!(status);
    }
    if let Some(include_pending) = include_pending {
        params["include_pending"] = json!(include_pending);
    }
    params
}

#[cfg(test)]
pub(super) fn library_probe_summary(payload: &Value) -> LibraryProbeSummary {
    let titles = parse_library_titles(payload);
    LibraryProbeSummary {
        raw_items: first_array_len_for_keys(payload, &["items", "products"]).unwrap_or(0),
        parsed_titles: titles.len(),
        supplemental_pdf_available: titles
            .iter()
            .filter(|title| title.supplemental_pdf_available)
            .count(),
        total_hint: first_u64_for_keys(
            payload,
            &[
                "total_results",
                "totalResults",
                "total_count",
                "totalCount",
                "num_results",
                "numResults",
                "count",
            ],
        ),
        state_token_present: first_string_for_keys(payload, &["state_token", "stateToken"])
            .is_some(),
    }
}

#[cfg(test)]
pub(super) fn first_array_len_for_keys(value: &Value, keys: &[&str]) -> Option<usize> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(array) = map.get(*key).and_then(Value::as_array) {
                    return Some(array.len());
                }
            }
            map.values()
                .find_map(|entry| first_array_len_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| first_array_len_for_keys(entry, keys)),
        _ => None,
    }
}

#[cfg(test)]
pub(super) fn first_u64_for_keys(value: &Value, keys: &[&str]) -> Option<u64> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(Value::as_u64) {
                    return Some(found);
                }
            }
            map.values()
                .find_map(|entry| first_u64_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| first_u64_for_keys(entry, keys)),
        _ => None,
    }
}

#[cfg(test)]
pub(super) fn first_string_for_keys(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(Value::as_str) {
                    return Some(found.to_string());
                }
            }
            map.values()
                .find_map(|entry| first_string_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| first_string_for_keys(entry, keys)),
        _ => None,
    }
}

#[cfg(test)]
mod probe;

#[cfg(test)]
mod tests {
    use super::audio_download::{
        build_download_request, download_status_failure, download_to_path, partial_download_path,
    };
    use super::license::{license_request_payload, license_request_spec};
    use super::materialization::helper_material_from_audible_material;
    use super::*;
    use crate::remote_source::materializer::{AaxcleanLane, AaxcleanSecret};
    use abb_audible_core::audible_decryption_material_from_license;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use reqwest::header::{RANGE, USER_AGENT};
    use std::collections::HashMap;

    fn fixture_auth_without_pdf_cookies() -> Auth {
        Auth {
            locale: localization::find_by_country_code(COUNTRY_CODE).expect("locale"),
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
                device_info: json!({ "device_type": AUDIBLE_IOS_DEVICE_TYPE }),
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
    fn library_request_uses_audible_max_page_size() {
        let params = library_request_params(None);

        assert_eq!(params["num_results"], AUDIBLE_LIBRARY_PAGE_SIZE);
        assert!(params.get("page").is_none());
        assert_eq!(params["sort_by"], "-PurchaseDate");
        assert!(params["response_groups"]
            .as_str()
            .expect("response groups")
            .contains("pdf_url"));
    }

    #[test]
    fn library_request_includes_availability_fields_used_for_title_status() {
        let params = library_request_params(None);
        let response_groups = params["response_groups"].as_str().expect("response groups");

        for required_group in ["is_playable", "is_visible", "is_downloaded"] {
            assert!(
                response_groups
                    .split(',')
                    .any(|group| group == required_group),
                "library request must ask Audible for {required_group}"
            );
        }
    }

    #[test]
    fn library_request_can_include_explicit_page() {
        let params = library_request_params(Some(2));

        assert_eq!(params["num_results"], AUDIBLE_LIBRARY_PAGE_SIZE);
        assert_eq!(params["page"], 2);
    }

    #[test]
    fn library_pagination_dedupes_titles_by_provider_id() {
        let mut titles = Vec::new();
        let mut seen_title_ids = HashSet::new();

        let first = library_page_payload(&["B000000001", "B000000002"]);
        let second = library_page_payload(&["B000000002", "B000000003"]);

        assert_eq!(
            append_unique_library_titles(&mut titles, &mut seen_title_ids, &first),
            (2, 2)
        );
        assert_eq!(
            append_unique_library_titles(&mut titles, &mut seen_title_ids, &second),
            (2, 1)
        );
        assert_eq!(
            titles
                .iter()
                .map(|title| title.title_id.as_str())
                .collect::<Vec<_>>(),
            vec!["B000000001", "B000000002", "B000000003"]
        );
    }

    #[test]
    fn library_pagination_handles_ambiguous_first_explicit_page() {
        assert!(
            should_continue_library_pagination(1, AUDIBLE_LIBRARY_PAGE_SIZE as usize, 0),
            "explicit page 1 may duplicate the default first page on one-based APIs"
        );
        assert!(should_continue_library_pagination(
            2,
            AUDIBLE_LIBRARY_PAGE_SIZE as usize,
            1
        ));
        assert!(!should_continue_library_pagination(
            2,
            AUDIBLE_LIBRARY_PAGE_SIZE as usize,
            0
        ));
        assert!(!should_continue_library_pagination(
            1,
            AUDIBLE_LIBRARY_PAGE_SIZE as usize - 1,
            AUDIBLE_LIBRARY_PAGE_SIZE as usize - 1
        ));
    }

    fn library_page_payload(title_ids: &[&str]) -> Value {
        json!({
            "items": title_ids
                .iter()
                .map(|title_id| json!({
                    "asin": title_id,
                    "title": format!("Remote Book {title_id}")
                }))
                .collect::<Vec<_>>()
        })
    }

    #[test]
    fn library_probe_summary_reports_sanitized_counts() {
        let payload = json!({
            "count": 2,
            "state_token": "provider-private-token",
            "items": [
                {
                    "asin": "B000000001",
                    "title": "Remote Book",
                    "details": {"pdf_url": "https://example.test/book.pdf"}
                },
                {
                    "asin": "B000000002",
                    "title": "Remote Book Without PDF"
                }
            ]
        });

        let summary = library_probe_summary(&payload);

        assert_eq!(summary.raw_items, 2);
        assert_eq!(summary.parsed_titles, 2);
        assert_eq!(summary.supplemental_pdf_available, 1);
        assert_eq!(summary.total_hint, Some(2));
        assert!(summary.state_token_present);
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
    fn partial_download_path_keeps_final_extension_visible() {
        let path = Path::new("/tmp/book.m4b");

        assert_eq!(
            partial_download_path(path),
            Path::new("/tmp/book.m4b.partial")
        );
    }

    #[cfg(unix)]
    #[test]
    fn partial_download_path_handles_non_utf8_extension_lossily() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let root = tempfile::TempDir::new().expect("temp root");
        let path = root
            .path()
            .join(OsString::from_vec(b"book.\xFFm4b".to_vec()));

        assert!(partial_download_path(&path)
            .to_string_lossy()
            .ends_with(".partial"));
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
    fn requested_supplemental_pdf_is_required_only_when_audible_advertises_one() {
        assert!(super::requested_supplemental_pdf_is_required(true, true));
        assert!(!super::requested_supplemental_pdf_is_required(true, false));
        assert!(!super::requested_supplemental_pdf_is_required(false, true));
        assert!(!super::requested_supplemental_pdf_is_required(false, false));
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
}
