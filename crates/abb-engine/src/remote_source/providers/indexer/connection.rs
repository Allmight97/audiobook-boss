use secrecy::SecretString;

use crate::app_settings::{normalize_base_url, IndexerConnection};
use crate::errors::{AppError, Result};
use crate::remote_source::types::RemoteIndexerConnectionUpdate;
use crate::remote_source::vault::SecretVault;

pub(super) fn api_key_vault_key(base_url: &str) -> String {
    format!("indexer.api_key:{base_url}")
}

pub(super) fn api_key_configured(
    connection: &IndexerConnection,
    vault: &dyn SecretVault,
) -> Result<bool> {
    Ok(read_api_key(vault, connection.base_url.as_deref())?.is_some())
}

pub(super) fn apply_key_change(
    vault: &dyn SecretVault,
    base_url: &str,
    key_change: Option<String>,
) -> Result<bool> {
    if let Some(key) = key_change {
        vault.set_secret(&api_key_vault_key(base_url), SecretString::from(key))?;
    }
    Ok(read_api_key(vault, Some(base_url))?.is_some())
}

pub(in crate::remote_source) struct ConfiguredIndexerConnection {
    pub base_url: String,
    pub category_ids: Vec<u32>,
    pub api_key: SecretString,
}

pub(super) fn configured_connection(
    connection: IndexerConnection,
    vault: &dyn SecretVault,
) -> Result<ConfiguredIndexerConnection> {
    let base_url = connection.base_url.ok_or_else(|| {
        AppError::InvalidInput("Configure Indexer URL in Settings before continuing.".to_string())
    })?;
    let api_key = read_api_key(vault, Some(&base_url))?.ok_or_else(|| {
        AppError::InvalidInput(
            "Configure Indexer API key in Settings before continuing.".to_string(),
        )
    })?;
    Ok(ConfiguredIndexerConnection {
        base_url,
        category_ids: connection.category_ids,
        api_key,
    })
}

fn read_api_key(vault: &dyn SecretVault, base_url: Option<&str>) -> Result<Option<SecretString>> {
    match base_url {
        Some(base_url) => vault.get_secret(&api_key_vault_key(base_url)),
        None => Ok(None),
    }
}

pub(super) fn draft_credentials(
    vault: &dyn SecretVault,
    update: RemoteIndexerConnectionUpdate,
) -> Result<(String, SecretString)> {
    let base_url = update
        .base_url
        .map(normalize_base_url)
        .transpose()?
        .flatten()
        .ok_or_else(|| {
            AppError::InvalidInput("Configure Indexer URL before testing.".to_string())
        })?;
    let api_key = match update.api_key {
        Some(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| SecretString::from(value.to_string()))
        }
        None => read_api_key(vault, Some(&base_url))?,
    }
    .ok_or_else(|| {
        AppError::InvalidInput(
            "Configure an API key for this Indexer URL before testing.".to_string(),
        )
    })?;
    Ok((base_url, api_key))
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
