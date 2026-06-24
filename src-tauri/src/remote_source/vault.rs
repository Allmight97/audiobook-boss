use std::sync::OnceLock;

use secrecy::{ExposeSecret, SecretString};

use crate::errors::{AppError, Result};

const SERVICE: &str = "audiobook-boss.remote-source";

pub(super) trait SecretVault: Send + Sync {
    fn get_secret(&self, key: &str) -> Result<Option<SecretString>>;
    fn set_secret(&self, key: &str, value: SecretString) -> Result<()>;
    fn delete_secret(&self, key: &str) -> Result<()>;
}

#[derive(Debug, Default)]
pub(super) struct KeyringSecretVault;

impl KeyringSecretVault {
    fn entry(key: &str) -> Result<keyring_core::Entry> {
        ensure_native_store()?;
        keyring_core::Entry::new(SERVICE, key)
            .map_err(|error| AppError::ResourceCleanup(format!("Keychain access failed: {error}")))
    }
}

/// Register the platform-native OS credential store as keyring-core's default,
/// exactly once per process.
///
/// Replaces `keyring::use_native_store`, dropped because the `keyring` umbrella
/// crate unconditionally pulled in its `db-keystore` fallback backend
/// (turso + tantivy + memmap2) that ABB never selected. The platform mapping
/// mirrors `keyring::use_native_store(false)`: macOS → login Keychain,
/// Windows → Credential Manager, Linux → kernel keyutils.
fn ensure_native_store() -> Result<()> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    if REGISTERED.get().is_some() {
        return Ok(());
    }
    register_native_store()?;
    let _ = REGISTERED.set(());
    Ok(())
}

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
fn register_native_store() -> Result<()> {
    let store = linux_keyutils_keyring_store::Store::new().map_err(store_unavailable)?;
    keyring_core::set_default_store(store);
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

impl SecretVault for KeyringSecretVault {
    fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
        let entry = Self::entry(key)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(SecretString::from(value))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(AppError::ResourceCleanup(format!(
                "Failed to read provider secret from secure storage: {error}"
            ))),
        }
    }

    fn set_secret(&self, key: &str, value: SecretString) -> Result<()> {
        let entry = Self::entry(key)?;
        entry.set_password(value.expose_secret()).map_err(|error| {
            AppError::ResourceCleanup(format!(
                "Failed to write provider secret to secure storage: {error}"
            ))
        })
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        let entry = Self::entry(key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(AppError::ResourceCleanup(format!(
                "Failed to delete provider secret from secure storage: {error}"
            ))),
        }
    }
}
