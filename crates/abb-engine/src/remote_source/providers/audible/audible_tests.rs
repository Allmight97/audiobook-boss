use super::license::{license_request_path, license_request_payload};
use super::*;

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
fn sign_in_starts_a_device_with_a_current_iphone_serial() {
    let (url, pending) = AudibleProvider::start_auth().expect("auth url");
    let (_, other) = AudibleProvider::start_auth().expect("second auth url");

    assert_ne!(pending.device_serial, other.device_serial);
    assert_eq!(pending.device_serial.len(), 40);
    assert!(pending
        .device_serial
        .chars()
        .all(|c| c.is_ascii_alphanumeric()));
    let client_id = audible_api::auth::oauth::build_client_id(&pending.device_serial);
    assert!(url.contains(&format!("device%3A{client_id}")), "{url}");
}

#[test]
fn license_request_matches_aaxc_adaptive_license_shape() {
    let payload = license_request_payload();

    assert_eq!(
        license_request_path("B000000001"),
        "/1.0/content/B000000001/licenserequest"
    );
    assert_eq!(payload["quality"], "High");
    assert_eq!(payload["consumption_type"], "Download");
    assert_eq!(payload["tenant_id"], "Audible");
    assert_eq!(
        payload["response_groups"],
        "last_position_heard,pdf_url,content_reference,chapter_info"
    );
    assert_eq!(payload["supported_media_features"]["drm_types"][0], "Adrm");
    assert_eq!(payload["supported_media_features"]["drm_types"][1], "Mpeg");
    assert_eq!(
        payload["supported_media_features"]["codecs"][0],
        "mp4a.40.2"
    );
    assert_eq!(
        payload["supported_media_features"]["chapter_titles_type"],
        "Tree"
    );
}

use crate::errors::{AppError, Result as AppResult};
use crate::remote_source::vault::SecretVault;
use audible_api::auth::register::Registration;
use audible_api::auth::{localization, Auth};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;

pub(in crate::remote_source) fn fixture_auth() -> Auth {
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
    set_calls: Mutex<Vec<String>>,
    delete_calls: Mutex<Vec<String>>,
    ephemeral: std::sync::atomic::AtomicBool,
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
    fn set_calls(&self) -> Vec<String> {
        self.set_calls.lock().expect("lock").clone()
    }
    fn delete_calls(&self) -> Vec<String> {
        self.delete_calls.lock().expect("lock").clone()
    }
    fn set_ephemeral(&self) {
        self.ephemeral
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl SecretVault for MockSecretVault {
    fn survives_restart(&self) -> bool {
        !self.ephemeral.load(std::sync::atomic::Ordering::Relaxed)
    }

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
    assert_eq!(
        state.message.as_deref(),
        Some("Connect Audible to load your library.")
    );
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
    assert!(state.message.is_none());
}

#[test]
fn account_state_says_sign_in_will_not_be_remembered_when_the_store_does_not_survive_restart() {
    let vault = MockSecretVault::default();
    vault.set_ephemeral();

    let needs_auth = AudibleProvider::account_state(&vault).expect("state");
    assert_eq!(needs_auth.status, RemoteAccountStatus::NeedsAuth);
    assert_eq!(
        needs_auth.message.as_deref(),
        Some(concat!(
            "Connect Audible to load your library. ",
            "Sign-in won't be remembered after a restart."
        ))
    );

    vault.seed_auth(AUTH_SECRET_KEY, &fixture_auth());
    let connected = AudibleProvider::account_state(&vault).expect("state");
    assert_eq!(connected.status, RemoteAccountStatus::Connected);
    assert_eq!(
        connected.message.as_deref(),
        Some("Sign-in won't be remembered after a restart.")
    );
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
