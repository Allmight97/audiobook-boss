use std::collections::HashSet;

use audible_api::auth::oauth::{build_oauth_url, extract_auth_code};
use audible_api::auth::{localization, Auth};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};

pub(super) mod acquisition;
mod audio_download;
mod diagnostics;
mod http;
mod library;
mod license;
mod license_reply_log;
mod materialization;
mod registration;
mod supplemental_pdf;

#[cfg(test)]
pub(crate) use acquisition::{run_protected_materialization, ProtectedMaterializationRun};

use acquisition::acquire;
use http::{ApiCall, AudibleApi};
pub(in crate::remote_source) use http::{AudibleRequestError, SIGN_IN_REJECTED_MESSAGE};
use library::parse_library_titles;

use crate::errors::{AppError, Result};
use crate::remote_source::vault::{account_message, SecretVault};
use crate::remote_source::{
    AccountRef, ProviderId, RemoteAccountStatus, RemoteLibraryResponse, RemoteSourceAccountState,
    RemoteSourceProviderCapabilities, RemoteTitle,
};

const COUNTRY_CODE: &str = "us";
const DOMAIN: &str = "com";
const MARKETPLACE_ID: &str = "ATVPDKIKX0DER";
const ACCOUNT_ID: &str = "audible-us";
const AUTH_SECRET_KEY: &str = "audible.us.auth";
const AUDIBLE_LIBRARY_PAGE_SIZE: u16 = 1000;
const MAX_AUDIBLE_LIBRARY_PAGES: u16 = 100;
const MAX_DOWNLOAD_REDIRECTS: usize = 5;
const MAX_DOWNLOAD_ATTEMPTS: u32 = 5;
const AUDIBLE_DOWNLOAD_USER_AGENT: &str = "Audible/671 CFNetwork/1240.0.4 Darwin/20.6.0";
const AUDIBLE_IOS_DEVICE_TYPE: &str = "A2CZJZGLK2JJVM";

fn provider_private_failure(stage: &str) -> AppError {
    AppError::General(format!(
        "Audible {stage} failed. Provider-private details were withheld from UI and logs."
    ))
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
        }
    }

    pub(in crate::remote_source) fn start_auth() -> Result<(String, PendingAudibleAuth)> {
        let (url, code_verifier, device_serial) = build_oauth_url(
            COUNTRY_CODE,
            DOMAIN,
            MARKETPLACE_ID,
            Some(registration::new_device_serial()),
            false,
        )
        .map_err(|error| {
            AppError::General(format!("Failed to start Audible auth flow: {error}"))
        })?;
        Ok((
            url,
            PendingAudibleAuth {
                code_verifier,
                device_serial,
            },
        ))
    }

    pub(in crate::remote_source) async fn register_auth(
        pending: PendingAudibleAuth,
        response_url: &str,
    ) -> Result<Auth> {
        let authorization_code = extract_audible_auth_code(response_url)?;
        let registration = registration::register_device(
            &authorization_code,
            &pending.code_verifier,
            DOMAIN,
            &pending.device_serial,
        )
        .await?;
        let locale = localization::find_by_country_code(COUNTRY_CODE)
            .ok_or_else(|| AppError::General("Audible locale is unavailable".to_string()))?;
        let auth = Auth {
            locale,
            device_registration: registration,
            authorization_code,
            code_verifier: pending.code_verifier,
        };
        Ok(auth)
    }

    pub(in crate::remote_source) fn persist_auth(
        vault: &dyn SecretVault,
        auth: &Auth,
    ) -> Result<RemoteSourceAccountState> {
        let serialized = serde_json::to_string(auth).map_err(|error| {
            AppError::General(format!("Failed to serialize Audible auth: {error}"))
        })?;
        vault.set_secret(AUTH_SECRET_KEY, SecretString::from(serialized))?;
        Self::account_state(vault)
    }

    pub(in crate::remote_source) fn account_state(
        vault: &dyn SecretVault,
    ) -> Result<RemoteSourceAccountState> {
        let survives = vault.survives_restart();
        let Some(secret) = vault.get_secret(AUTH_SECRET_KEY)? else {
            return Ok(RemoteSourceAccountState {
                provider_id: ProviderId::Audible,
                status: RemoteAccountStatus::NeedsAuth,
                account: None,
                message: account_message(Some("Connect Audible to load your library."), survives),
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
            message: account_message(None, survives),
        })
    }

    pub(in crate::remote_source) fn logout(vault: &dyn SecretVault) -> Result<()> {
        vault.delete_secret(AUTH_SECRET_KEY)
    }

    pub(in crate::remote_source) fn stored_auth(vault: &dyn SecretVault) -> Result<Auth> {
        auth_from_vault(vault)
    }

    pub(in crate::remote_source) async fn load_library(
        auth: Auth,
    ) -> std::result::Result<RemoteLibraryResponse, AudibleRequestError> {
        let titles = load_all_library_titles(&AudibleApi::new(auth)?).await?;
        Ok(RemoteLibraryResponse {
            provider_id: ProviderId::Audible,
            titles,
            diagnostics: Vec::new(),
        })
    }

    pub(in crate::remote_source) async fn acquire(
        auth: Auth,
        materializer: &crate::remote_source::materializer::AaxcleanMaterializer,
        plan: &crate::remote_source::AcquisitionPlan,
        job_id: &str,
        job_dir: &std::path::Path,
        progress: impl FnMut(abb_remote_source_core::AcquisitionProgress),
        is_cancelled: impl Fn() -> bool,
    ) -> Result<crate::remote_source::AcquisitionJob> {
        acquire(
            auth,
            materializer,
            plan,
            job_id,
            job_dir,
            progress,
            is_cancelled,
        )
        .await
    }
}

async fn load_all_library_titles(
    api: &AudibleApi,
) -> std::result::Result<Vec<RemoteTitle>, AudibleRequestError> {
    let mut titles = Vec::new();
    let mut seen_title_ids = HashSet::new();

    let first_payload = fetch_library_page(api, None).await?;
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
        let payload = fetch_library_page(api, Some(page)).await?;
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
    ))
    .into())
}

async fn fetch_library_page(
    api: &AudibleApi,
    page: Option<u16>,
) -> std::result::Result<Value, AudibleRequestError> {
    api.get(
        "/1.0/library",
        &library_request_params(page),
        ApiCall::account("library request"),
    )
    .await
    .map_err(|error| match error {
        AudibleRequestError::Failed(_) => AppError::General(
            "An Audible library page could not be loaded. No partial library was loaded."
                .to_string(),
        )
        .into(),
        rejected => rejected,
    })
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

pub(super) fn library_request_params(page: Option<u16>) -> Value {
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

pub(super) fn auth_from_vault(vault: &dyn SecretVault) -> Result<Auth> {
    let secret = vault
        .get_secret(AUTH_SECRET_KEY)?
        .ok_or_else(|| AppError::InvalidInput("Audible account is not connected.".to_string()))?;
    serde_json::from_str(secret.expose_secret()).map_err(|error| {
        AppError::ResourceCleanup(format!("Stored Audible auth is invalid: {error}"))
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

#[cfg(test)]
mod library_probe;

#[cfg(test)]
#[path = "probe_tests.rs"]
mod probe;

#[cfg(test)]
#[path = "audible_tests.rs"]
mod tests;
