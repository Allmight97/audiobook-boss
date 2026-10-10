use std::sync::OnceLock;

use secrecy::{ExposeSecret, SecretString};

use crate::errors::{AppError, Result};

const PRODUCTION_SERVICE: &str = "audiobook-boss.remote-source";

pub(super) trait SecretVault: Send + Sync {
    fn get_secret(&self, key: &str) -> Result<Option<SecretString>>;
    fn set_secret(&self, key: &str, value: SecretString) -> Result<()>;
    fn delete_secret(&self, key: &str) -> Result<()>;

    /// Whether a secret written now is still readable after an OS restart or a
    /// WSL VM stop. Test doubles keep secrets for the test, so they report true.
    /// The OS vault reports the registered store's persistence.
    fn survives_restart(&self) -> bool {
        true
    }
}

/// Shown on the account view when the registered store will not keep a sign-in.
pub(super) const SIGN_IN_NOT_REMEMBERED: &str = "Sign-in won't be remembered after a restart.";

pub(super) fn account_message(base: Option<&str>, survives_restart: bool) -> Option<String> {
    match (base, survives_restart) {
        (Some(base), true) => Some(base.to_string()),
        (None, true) => None,
        (Some(base), false) => Some(format!("{base} {SIGN_IN_NOT_REMEMBERED}")),
        (None, false) => Some(SIGN_IN_NOT_REMEMBERED.to_string()),
    }
}

/// `UntilDelete` is on-disk storage. Anything shorter, including keyutils'
/// `UntilReboot`, is gone after a restart, so the account view must say so.
fn sign_in_survives_restart(persistence: keyring_core::CredentialPersistence) -> bool {
    matches!(
        persistence,
        keyring_core::CredentialPersistence::UntilDelete
    )
}

#[derive(Debug)]
pub(super) struct KeyringSecretVault {
    service: String,
}

impl KeyringSecretVault {
    pub(super) fn for_app_identifier(identifier: &str) -> Self {
        Self {
            // Keep shipped credentials reachable; other app identities own their slots.
            service: if identifier == "com.audiobook-boss" {
                PRODUCTION_SERVICE.to_string()
            } else {
                format!("{identifier}.remote-source")
            },
        }
    }

    fn entry(&self, key: &str) -> Result<keyring_core::Entry> {
        ensure_native_store()?;
        keyring_core::Entry::new(&self.service, key)
            .map_err(|error| AppError::ResourceCleanup(format!("Keychain access failed: {error}")))
    }
}

/// Register the platform-native OS credential store as keyring-core's default,
/// exactly once per process.
///
/// Replaces `keyring::use_native_store`, dropped because the `keyring` umbrella
/// crate unconditionally pulled in its `db-keystore` fallback backend
/// (turso + tantivy + memmap2) that ABB never selected. macOS uses the login
/// Keychain and Windows uses Credential Manager. Linux uses the Secret Service
/// when one answers on the session bus, and kernel keyutils otherwise. Keyutils
/// keeps secrets only until restart, so the account view says they will not be
/// remembered. No plaintext file, and no copy of an old keyutils secret.
fn ensure_native_store() -> Result<()> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    if REGISTERED.get().is_some() {
        return Ok(());
    }
    register_native_store()?;
    let _ = REGISTERED.set(());
    Ok(())
}

/// The legacy file Keychain: the Data Protection Keychain needs Developer ID
/// signing and a `keychain-access-groups` entitlement the unsigned build lacks.
/// Moving stores waits for that signing; a vault error stays a typed error,
/// never `NeedsAuth` or an empty result.
#[cfg(target_os = "macos")]
fn register_native_store() -> Result<()> {
    let store = apple_native_keyring_store::keychain::Store::new().map_err(store_unavailable)?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "windows")]
fn register_native_store() -> Result<()> {
    let store = windows_native_keyring_store::Store::new().map_err(store_unavailable)?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_secret_service() -> Option<std::sync::Arc<zbus_secret_service_keyring_store::Store>> {
    // `Store::new` connects to the session bus. No answer means keyutils.
    zbus_secret_service_keyring_store::Store::new().ok()
}

#[cfg(target_os = "linux")]
fn register_native_store() -> Result<()> {
    if let Some(store) = open_secret_service() {
        keyring_core::set_default_store(store);
    } else {
        let store = linux_keyutils_keyring_store::Store::new().map_err(store_unavailable)?;
        keyring_core::set_default_store(store);
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn register_native_store() -> Result<()> {
    Err(AppError::ResourceCleanup(
        "Secure storage is not supported on this platform".to_string(),
    ))
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn store_unavailable(error: keyring_core::Error) -> AppError {
    AppError::ResourceCleanup(format!("Secure storage is unavailable: {error}"))
}

fn delete_entry(entry: keyring_core::Entry) -> Result<()> {
    match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(AppError::ResourceCleanup(format!(
            "Failed to delete provider secret from secure storage: {error}"
        ))),
    }
}

/// A launch that finds no Secret Service falls back to keyutils, so a
/// Disconnect also removes any keyutils copy left from such a launch.
#[cfg(target_os = "linux")]
fn keyutils_entry(service: &str, key: &str) -> Result<keyring_core::Entry> {
    use keyring_core::api::CredentialStoreApi;
    linux_keyutils_keyring_store::Store::new()
        .and_then(|store| store.build(service, key, None))
        .map_err(store_unavailable)
}

impl SecretVault for KeyringSecretVault {
    fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(SecretString::from(value))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(AppError::ResourceCleanup(format!(
                "Failed to read provider secret from secure storage: {error}"
            ))),
        }
    }

    fn set_secret(&self, key: &str, value: SecretString) -> Result<()> {
        let entry = self.entry(key)?;
        entry.set_password(value.expose_secret()).map_err(|error| {
            AppError::ResourceCleanup(format!(
                "Failed to write provider secret to secure storage: {error}"
            ))
        })
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        delete_entry(self.entry(key)?)?;
        #[cfg(target_os = "linux")]
        delete_entry(keyutils_entry(&self.service, key)?)?;
        Ok(())
    }

    fn survives_restart(&self) -> bool {
        let Ok(()) = ensure_native_store() else {
            return false;
        };
        keyring_core::get_default_store()
            .is_some_and(|store| sign_in_survives_restart(store.persistence()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_identity_keeps_shipped_credentials_and_isolates_development_vaults() {
        let production = KeyringSecretVault::for_app_identifier("com.audiobook-boss");
        let first = KeyringSecretVault::for_app_identifier("com.audiobook-boss.dev.first");
        let second = KeyringSecretVault::for_app_identifier("com.audiobook-boss.dev.second");
        assert_eq!(production.service, "audiobook-boss.remote-source");
        assert_ne!(first.service, production.service);
        assert_ne!(first.service, second.service);
        assert_eq!(
            first.service,
            KeyringSecretVault::for_app_identifier("com.audiobook-boss.dev.first").service
        );
    }

    #[test]
    fn sign_in_survives_restart_only_for_until_delete() {
        use keyring_core::CredentialPersistence;
        assert!(sign_in_survives_restart(CredentialPersistence::UntilDelete));
        assert!(!sign_in_survives_restart(
            CredentialPersistence::UntilReboot
        ));
        assert!(!sign_in_survives_restart(
            CredentialPersistence::UntilLogout
        ));
        assert!(!sign_in_survives_restart(
            CredentialPersistence::Unspecified
        ));
    }

    #[test]
    fn ephemeral_store_tells_the_user_sign_in_will_not_be_remembered() {
        assert_eq!(
            account_message(Some("Connect Audible to load your library."), false).as_deref(),
            Some(concat!(
                "Connect Audible to load your library. ",
                "Sign-in won't be remembered after a restart."
            ))
        );
        assert_eq!(
            account_message(None, false).as_deref(),
            Some(SIGN_IN_NOT_REMEMBERED)
        );
        assert_eq!(
            account_message(Some("Connect Audible to load your library."), true).as_deref(),
            Some("Connect Audible to load your library.")
        );
        assert_eq!(account_message(None, true), None);
    }

    /// Headless `abb-dev` and the engine tests have no Secret Service. The probe
    /// must fail closed so registration can use keyutils instead of hanging.
    #[cfg(target_os = "linux")]
    #[test]
    fn secret_service_probe_fails_closed_without_a_session_bus() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
            return;
        }
        assert!(open_secret_service().is_none());
    }
}
