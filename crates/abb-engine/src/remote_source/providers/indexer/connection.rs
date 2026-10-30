use std::io;
use std::path::{Path, PathBuf};

use secrecy::SecretString;

use crate::errors::{AppError, Result};
use crate::remote_source::types::{RemoteIndexerConnection, RemoteIndexerConnectionUpdate};
use crate::remote_source::vault::SecretVault;

pub(super) fn api_key_vault_key(base_url: &str) -> String {
    format!("indexer.api_key:{base_url}")
}
pub(super) const DEFAULT_CATEGORY_IDS: &[u32] = &[3000, 3030];

const CONNECTION_FILE_NAME: &str = "indexer.toml";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct StoredIndexerConnection {
    #[serde(rename = "url")]
    base_url: Option<String>,
    #[serde(rename = "categories")]
    category_ids: Vec<u32>,
}

impl Default for StoredIndexerConnection {
    fn default() -> Self {
        Self {
            base_url: None,
            category_ids: default_category_ids(),
        }
    }
}

fn default_category_ids() -> Vec<u32> {
    DEFAULT_CATEGORY_IDS.to_vec()
}

fn chosen_category_ids(ids: Vec<u32>) -> Result<Vec<u32>> {
    let mut chosen: Vec<u32> = ids.into_iter().filter(|id| *id > 0).collect();
    chosen.sort_unstable();
    chosen.dedup();
    if chosen.is_empty() {
        return Err(AppError::InvalidInput(
            "Choose at least one Indexer category.".to_string(),
        ));
    }
    Ok(chosen)
}

pub(super) fn get_connection(
    config_dir: &Path,
    vault: &dyn SecretVault,
) -> Result<RemoteIndexerConnection> {
    let stored = load_stored_connection(config_dir);
    Ok(RemoteIndexerConnection {
        base_url: stored.base_url.clone(),
        category_ids: stored.category_ids,
        api_key_configured: read_api_key(vault, stored.base_url.as_deref())?.is_some(),
    })
}

pub(super) fn update_connection(
    config_dir: &Path,
    vault: &dyn SecretVault,
    update: RemoteIndexerConnectionUpdate,
) -> Result<RemoteIndexerConnection> {
    let previous = load_stored_connection(config_dir);
    let mut stored = previous.clone();
    if let Some(base_url) = update.base_url {
        stored.base_url = normalize_base_url(base_url)?;
    }
    if let Some(category_ids) = update.category_ids {
        stored.category_ids = chosen_category_ids(category_ids)?;
    }
    let key_change = update.api_key.map(|key| key.trim().to_string());
    if key_change.as_ref().is_some_and(|key| !key.is_empty()) && stored.base_url.is_none() {
        return Err(AppError::InvalidInput(
            "Configure Indexer URL before saving an API key.".to_string(),
        ));
    }
    let settings_changed = stored != previous;
    if settings_changed {
        save_stored_connection(config_dir, &stored)?;
    }
    if let Some(base_url) = stored.base_url.as_deref() {
        let result = update_api_key(
            vault,
            base_url,
            key_change,
            update.clear_api_key == Some(true),
        );
        result.map_err(|error| {
            if settings_changed {
                AppError::General(format!("Indexer connection settings were saved, but the API key could not be updated: {error}"))
            } else {
                error
            }
        })?;
    }
    get_connection(config_dir, vault)
}

fn update_api_key(
    vault: &dyn SecretVault,
    base_url: &str,
    key_change: Option<String>,
    clear: bool,
) -> Result<()> {
    let key = api_key_vault_key(base_url);
    match key_change {
        Some(value) if !value.is_empty() => vault.set_secret(&key, SecretString::from(value)),
        Some(_) => vault.delete_secret(&key),
        None if clear => vault.delete_secret(&key),
        None => Ok(()),
    }
}

pub(in crate::remote_source) struct ConfiguredIndexerConnection {
    pub base_url: String,
    pub category_ids: Vec<u32>,
    pub api_key: SecretString,
}

pub(super) fn configured_connection(
    config_dir: &Path,
    vault: &dyn SecretVault,
) -> Result<ConfiguredIndexerConnection> {
    let stored = load_stored_connection(config_dir);
    let base_url = stored.base_url.ok_or_else(|| {
        AppError::InvalidInput("Configure Indexer URL in Settings before continuing.".to_string())
    })?;
    let api_key = read_api_key(vault, Some(&base_url))?.ok_or_else(|| {
        AppError::InvalidInput(
            "Configure Indexer API key in Settings before continuing.".to_string(),
        )
    })?;
    Ok(ConfiguredIndexerConnection {
        base_url,
        category_ids: stored.category_ids,
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
    config_dir: &Path,
    vault: &dyn SecretVault,
    update: RemoteIndexerConnectionUpdate,
) -> Result<(String, SecretString)> {
    let base_url = match update.base_url {
        Some(url) => normalize_base_url(url)?,
        None => load_stored_connection(config_dir).base_url,
    }
    .ok_or_else(|| AppError::InvalidInput("Configure Indexer URL before testing.".to_string()))?;
    let api_key = match update.api_key {
        Some(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| SecretString::from(value.to_string()))
        }
        None if update.clear_api_key == Some(true) => None,
        None => read_api_key(vault, Some(&base_url))?,
    }
    .ok_or_else(|| {
        AppError::InvalidInput(
            "Configure an API key for this Indexer URL before testing.".to_string(),
        )
    })?;
    Ok((base_url, api_key))
}

fn load_stored_connection(config_dir: &Path) -> StoredIndexerConnection {
    let content = match std::fs::read_to_string(connection_path(config_dir)) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return StoredIndexerConnection::default();
        }
        Err(error) => {
            log::warn!("Indexer settings could not be read; using none: {error}");
            return StoredIndexerConnection::default();
        }
    };
    let stored = toml::from_str::<StoredIndexerConnection>(&content)
        .map_err(|error| error.to_string())
        .and_then(|stored| {
            let base_url = stored
                .base_url
                .map(normalize_base_url)
                .transpose()
                .map_err(|error| error.to_string())?
                .flatten();
            Ok(StoredIndexerConnection {
                base_url,
                category_ids: chosen_category_ids(stored.category_ids)
                    .unwrap_or_else(|_| default_category_ids()),
            })
        });
    stored.unwrap_or_else(|error| {
        log::warn!("Indexer settings were not usable; using none: {error}");
        StoredIndexerConnection::default()
    })
}

fn save_stored_connection(config_dir: &Path, stored: &StoredIndexerConnection) -> Result<()> {
    let content = toml::to_string(stored)
        .map_err(|error| AppError::General(format!("Failed to write Indexer settings: {error}")))?;
    crate::file_replace::write_atomically(&connection_path(config_dir), content.as_bytes())
        .inspect_err(|error| log::warn!("Indexer settings were not saved: {error}"))?;
    Ok(())
}

fn connection_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CONNECTION_FILE_NAME)
}

pub(in crate::remote_source) fn normalize_base_url(base_url: String) -> Result<Option<String>> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = reqwest::Url::parse(trimmed).map_err(|_| {
        AppError::InvalidInput("Indexer URL must be a valid http or https URL.".to_string())
    })?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(AppError::InvalidInput(
            "Indexer URL must use http or https.".to_string(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::InvalidInput(
            "Indexer URL must not contain credentials. Use the API key field instead.".to_string(),
        ));
    }
    let mut normalized = format!("{}://{}", parsed.scheme(), parsed.authority());
    let path = parsed.path().trim_end_matches('/');
    if !path.is_empty() && path != "/" {
        normalized.push_str(path);
    }
    Ok(Some(normalized))
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
