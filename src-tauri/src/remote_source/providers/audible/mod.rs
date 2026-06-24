use std::collections::HashSet;

use audible_api::api::Client as AudibleClient;
use audible_api::auth::oauth::{build_oauth_url, extract_auth_code};
use audible_api::auth::register::register;
use audible_api::auth::{localization, Auth};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};

pub(super) mod acquisition;
mod audio_download;
mod diagnostics;
mod http;
mod library;
mod license;
mod materialization;
mod supplemental_pdf;

use acquisition::acquire;
use library::parse_library_titles;

use crate::errors::{AppError, Result};
use crate::remote_source::vault::SecretVault;
use crate::remote_source::{
    AccountRef, ProviderId, RemoteAccountStatus, RemoteAcquisitionFailureKind, RemoteAuthFlow,
    RemoteLibraryResponse, RemoteSourceAccountState, RemoteSourceProviderCapabilities, RemoteTitle,
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
        Self::persist_auth(vault, &auth)
    }

    /// Serialize the Audible auth and persist it to the secure vault, then
    /// report the resulting account state. Separated from `complete_auth` so
    /// the persistence boundary is testable without the live OAuth `register`
    /// exchange.
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
        materializer: &crate::remote_source::materializer::AaxcleanMaterializer,
        plan: &crate::remote_source::AcquisitionPlan,
        job_id: &str,
        job_dir: &std::path::Path,
        progress: impl FnMut(abb_remote_source_core::AcquisitionProgress),
        is_cancelled: impl Fn() -> bool,
    ) -> Result<crate::remote_source::AcquisitionJob> {
        acquire(
            vault,
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

#[cfg(test)]
mod library_probe;

#[cfg(test)]
mod probe;

#[cfg(test)]
mod tests {
    use super::audio_download::{build_download_request, download_to_path};
    use crate::remote_source::scoped_output::partial_sibling;
    use super::library_probe::library_probe_summary;
    use super::license::{license_request_payload, license_request_spec};
    use super::*;
    use abb_remote_source_core::AcquisitionProgress;
    use reqwest::header::{RANGE, USER_AGENT};
    use std::path::Path;

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
    fn partial_sibling_keeps_final_extension_visible() {
        let path = Path::new("/tmp/book.m4b");

        assert_eq!(partial_sibling(path), Path::new("/tmp/book.m4b.partial"));
    }

    #[cfg(unix)]
    #[test]
    fn partial_sibling_handles_non_utf8_extension_lossily() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let root = tempfile::TempDir::new().expect("temp root");
        let path = root
            .path()
            .join(OsString::from_vec(b"book.\xFFm4b".to_vec()));

        assert!(partial_sibling(&path)
            .to_string_lossy()
            .ends_with(".partial"));
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
        assert!(!partial_sibling(&target).exists());
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

    // -- Audible auth -> keychain persistence chain (mock vault, no network) --

    use crate::errors::{AppError, Result as AppResult};
    use crate::remote_source::vault::SecretVault;
    use audible_api::auth::register::Registration;
    use audible_api::auth::{localization, Auth};
    use secrecy::{ExposeSecret, SecretString};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn fixture_auth() -> Auth {
        Auth {
            locale: localization::find_by_country_code(COUNTRY_CODE).expect("locale"),
            device_registration: Registration {
                device_serial: "device-serial".to_string(),
                client_id: "client-id".to_string(),
                adp_token: "adp-token".to_string(),
                device_private_key: "device-private-key".to_string(),
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                expires: 0,
                website_cookies: HashMap::new(),
                store_authentication_cookie: "store-cookie".to_string(),
                device_info: json!({ "device_type": AUDIBLE_IOS_DEVICE_TYPE }),
                customer_info: json!({ "user_id": "account-1", "name": "Fixture Listener" }),
            },
            authorization_code: "authorization-code".to_string(),
            code_verifier: "code-verifier".to_string(),
        }
    }

    #[derive(Default)]
    struct MockSecretVault {
        secrets: Mutex<HashMap<String, String>>,
        get_error: Mutex<Option<AppError>>,
        set_error: Mutex<Option<AppError>>,
        delete_error: Mutex<Option<AppError>>,
        set_calls: Mutex<Vec<String>>,
        delete_calls: Mutex<Vec<String>>,
    }

    impl MockSecretVault {
        fn seed_raw(&self, key: &str, value: &str) {
            self.secrets
                .lock()
                .expect("lock")
                .insert(key.to_string(), value.to_string());
        }
        fn seed_auth(&self, key: &str, auth: &Auth) {
            let serialized = serde_json::to_string(auth).expect("serialize");
            self.seed_raw(key, &serialized);
        }
        fn set_get_error(&self, error: AppError) {
            *self.get_error.lock().expect("lock") = Some(error);
        }
        fn set_set_error(&self, error: AppError) {
            *self.set_error.lock().expect("lock") = Some(error);
        }
        fn set_delete_error(&self, error: AppError) {
            *self.delete_error.lock().expect("lock") = Some(error);
        }
        fn set_calls(&self) -> Vec<String> {
            self.set_calls.lock().expect("lock").clone()
        }
        fn delete_calls(&self) -> Vec<String> {
            self.delete_calls.lock().expect("lock").clone()
        }
    }

    impl SecretVault for MockSecretVault {
        fn get_secret(&self, key: &str) -> AppResult<Option<SecretString>> {
            if let Some(error) = self.get_error.lock().expect("lock").take() {
                return Err(error);
            }
            Ok(self
                .secrets
                .lock()
                .expect("lock")
                .get(key)
                .map(|value| SecretString::from(value.clone())))
        }
        fn set_secret(&self, key: &str, value: SecretString) -> AppResult<()> {
            self.set_calls.lock().expect("lock").push(key.to_string());
            if let Some(error) = self.set_error.lock().expect("lock").take() {
                return Err(error);
            }
            self.secrets
                .lock()
                .expect("lock")
                .insert(key.to_string(), value.expose_secret().to_string());
            Ok(())
        }
        fn delete_secret(&self, key: &str) -> AppResult<()> {
            self.delete_calls
                .lock()
                .expect("lock")
                .push(key.to_string());
            if let Some(error) = self.delete_error.lock().expect("lock").take() {
                return Err(error);
            }
            self.secrets.lock().expect("lock").remove(key);
            Ok(())
        }
    }

    #[test]
    fn persist_auth_stores_serialized_auth_and_reports_connected() {
        let vault = MockSecretVault::default();
        let auth = fixture_auth();

        let state = AudibleProvider::persist_auth(&vault, &auth).expect("persist");

        assert_eq!(state.status, RemoteAccountStatus::Connected);
        assert_eq!(state.account.expect("account").account_id, ACCOUNT_ID);
        assert_eq!(vault.set_calls(), vec![AUTH_SECRET_KEY.to_string()]);

        let stored = vault
            .get_secret(AUTH_SECRET_KEY)
            .expect("read back")
            .expect("secret present");
        let serialized = stored.expose_secret();
        assert!(!serialized.is_empty());
        let round_trip: Auth = serde_json::from_str(serialized).expect("valid json round-trip");
        assert_eq!(round_trip.authorization_code, auth.authorization_code);
    }

    #[test]
    fn persist_auth_propagates_vault_write_error() {
        let vault = MockSecretVault::default();
        vault.set_set_error(AppError::ResourceCleanup("keychain locked".to_string()));
        let auth = fixture_auth();

        let error = AudibleProvider::persist_auth(&vault, &auth).expect_err("propagated");

        assert!(matches!(error, AppError::ResourceCleanup(_)));
        assert_eq!(vault.set_calls(), vec![AUTH_SECRET_KEY.to_string()]);
    }

    #[test]
    fn account_state_needs_auth_when_vault_has_no_secret() {
        let vault = MockSecretVault::default();

        let state = AudibleProvider::account_state(&vault).expect("state");

        assert_eq!(state.status, RemoteAccountStatus::NeedsAuth);
        assert!(state.account.is_none());
    }

    #[test]
    fn account_state_connected_when_vault_holds_valid_auth() {
        let vault = MockSecretVault::default();
        vault.seed_auth(AUTH_SECRET_KEY, &fixture_auth());

        let state = AudibleProvider::account_state(&vault).expect("state");

        assert_eq!(state.status, RemoteAccountStatus::Connected);
        let account = state.account.expect("account");
        assert_eq!(account.account_id, ACCOUNT_ID);
        assert_eq!(account.display_name, "Fixture Listener");
    }

    #[test]
    fn account_state_propagates_vault_read_error_instead_of_needs_auth() {
        let vault = MockSecretVault::default();
        vault.set_get_error(AppError::ResourceCleanup("keychain locked".to_string()));

        let error = AudibleProvider::account_state(&vault).expect_err("propagated");

        assert!(matches!(error, AppError::ResourceCleanup(_)));
    }

    #[test]
    fn logout_deletes_secret_and_subsequent_account_state_needs_auth() {
        let vault = MockSecretVault::default();
        vault.seed_auth(AUTH_SECRET_KEY, &fixture_auth());

        AudibleProvider::logout(&vault).expect("logout");

        assert_eq!(vault.delete_calls(), vec![AUTH_SECRET_KEY.to_string()]);
        let state = AudibleProvider::account_state(&vault).expect("state");
        assert_eq!(state.status, RemoteAccountStatus::NeedsAuth);
    }

    #[test]
    fn logout_propagates_vault_delete_error() {
        let vault = MockSecretVault::default();
        vault.seed_auth(AUTH_SECRET_KEY, &fixture_auth());
        vault.set_delete_error(AppError::ResourceCleanup("keychain locked".to_string()));

        let error = AudibleProvider::logout(&vault).expect_err("propagated");

        assert!(matches!(error, AppError::ResourceCleanup(_)));
        assert_eq!(vault.delete_calls(), vec![AUTH_SECRET_KEY.to_string()]);
    }

    #[test]
    fn auth_from_vault_returns_invalid_input_when_not_connected() {
        let vault = MockSecretVault::default();

        let error = auth_from_vault(&vault).expect_err("not connected");

        assert!(matches!(error, AppError::InvalidInput(_)));
    }

    #[test]
    fn auth_from_vault_propagates_vault_read_error() {
        let vault = MockSecretVault::default();
        vault.set_get_error(AppError::ResourceCleanup("keychain locked".to_string()));

        let error = auth_from_vault(&vault).expect_err("propagated");

        assert!(matches!(error, AppError::ResourceCleanup(_)));
    }

    #[test]
    fn corrupt_stored_auth_error_does_not_leak_secret_bytes() {
        let vault = MockSecretVault::default();
        let secret_payload = "not-valid-json{sensitive-token-value}";
        vault.seed_raw(AUTH_SECRET_KEY, secret_payload);

        let account_error = AudibleProvider::account_state(&vault).expect_err("corrupt auth");
        assert!(!account_error.to_string().contains("sensitive-token-value"));
        assert!(!account_error.to_string().contains(secret_payload));

        let auth_error = auth_from_vault(&vault).expect_err("corrupt auth");
        assert!(!auth_error.to_string().contains("sensitive-token-value"));
        assert!(!auth_error.to_string().contains(secret_payload));
    }
}
