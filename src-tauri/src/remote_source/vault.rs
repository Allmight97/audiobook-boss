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
        keyring::use_native_store(false).map_err(|error| {
            AppError::ResourceCleanup(format!("Secure storage is unavailable: {error}"))
        })?;
        keyring_core::Entry::new(SERVICE, key)
            .map_err(|error| AppError::ResourceCleanup(format!("Keychain access failed: {error}")))
    }
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
