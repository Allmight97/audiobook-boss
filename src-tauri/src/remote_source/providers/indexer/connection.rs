use std::io;
use std::path::{Path, PathBuf};

use secrecy::SecretString;

use crate::errors::{AppError, Result};
use crate::remote_source::types::{RemoteIndexerConnection, RemoteIndexerConnectionUpdate};
use crate::remote_source::vault::SecretVault;

pub(super) const API_KEY_VAULT_KEY: &str = "indexer.api_key";
pub(super) const DEFAULT_CATEGORY_ID: u32 = 3030;

const CONNECTION_FILE_NAME: &str = "remote-source-indexer-connection.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredIndexerConnection {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default = "default_category_id")]
    category_id: u32,
}

impl Default for StoredIndexerConnection {
    fn default() -> Self {
        Self {
            base_url: None,
            category_id: DEFAULT_CATEGORY_ID,
        }
    }
}

fn default_category_id() -> u32 {
    DEFAULT_CATEGORY_ID
}

pub(super) fn get_connection(
    config_dir: &Path,
    vault: &dyn SecretVault,
) -> Result<RemoteIndexerConnection> {
    let stored = load_stored_connection(config_dir)?;
    Ok(RemoteIndexerConnection {
        base_url: stored.base_url,
        category_id: stored.category_id,
        api_key_configured: vault.get_secret(API_KEY_VAULT_KEY)?.is_some(),
    })
}

pub(super) fn update_connection(
    config_dir: &Path,
    vault: &dyn SecretVault,
    update: RemoteIndexerConnectionUpdate,
) -> Result<RemoteIndexerConnection> {
    let mut stored = load_stored_connection(config_dir)?;

    if let Some(base_url) = update.base_url {
        stored.base_url = normalize_base_url(base_url)?;
    }
    if let Some(category_id) = update.category_id {
        stored.category_id = category_id;
    }
    if update.clear_api_key == Some(true) {
        vault.delete_secret(API_KEY_VAULT_KEY)?;
    }
    if let Some(api_key) = update.api_key {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            vault.delete_secret(API_KEY_VAULT_KEY)?;
        } else {
            vault.set_secret(API_KEY_VAULT_KEY, SecretString::from(trimmed.to_string()))?;
        }
    }

    save_stored_connection(config_dir, &stored)?;
    get_connection(config_dir, vault)
}

fn load_stored_connection(config_dir: &Path) -> Result<StoredIndexerConnection> {
    let path = connection_path(config_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(StoredIndexerConnection::default());
        }
        Err(error) => return Err(AppError::Io(error)),
    };

    serde_json::from_str(&content).map_err(|error| {
        AppError::InvalidInput(format!(
            "Indexer connection settings are invalid. Update Indexer settings to restore defaults. ({error})"
        ))
    })
}

fn save_stored_connection(config_dir: &Path, stored: &StoredIndexerConnection) -> Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let path = connection_path(config_dir);
    let temp_path = config_dir.join(format!(".indexer-connection-{}.tmp", uuid::Uuid::new_v4()));
    let content = serde_json::to_string_pretty(stored).map_err(|error| {
        AppError::General(format!("Failed to serialize indexer connection: {error}"))
    })?;

    let write_and_replace = || -> Result<()> {
        std::fs::write(&temp_path, &content)?;
        if path.exists() {
            crate::file_replace::replace_file(&temp_path, &path)?;
        } else {
            std::fs::rename(&temp_path, &path)?;
        }
        Ok(())
    };

    let result = write_and_replace();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

fn connection_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CONNECTION_FILE_NAME)
}

fn normalize_base_url(base_url: String) -> Result<Option<String>> {
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
    let mut normalized = format!("{}://{}", parsed.scheme(), parsed.authority());
    let path = parsed.path().trim_end_matches('/');
    if !path.is_empty() && path != "/" {
        normalized.push_str(path);
    }
    Ok(Some(normalized))
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;
    use tempfile::TempDir;

    #[derive(Default)]
    struct TestVault {
        secret: Option<SecretString>,
    }

    impl SecretVault for TestVault {
        fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
            assert_eq!(key, API_KEY_VAULT_KEY);
            Ok(self.secret.clone())
        }

        fn set_secret(&self, _key: &str, _value: SecretString) -> Result<()> {
            Ok(())
        }

        fn delete_secret(&self, _key: &str) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn missing_connection_file_returns_defaults() {
        let temp = TempDir::new().expect("temp dir");
        let vault = TestVault::default();

        let connection = get_connection(temp.path(), &vault).expect("connection");

        assert_eq!(connection.base_url, None);
        assert_eq!(connection.category_id, DEFAULT_CATEGORY_ID);
        assert!(!connection.api_key_configured);
    }

    #[test]
    fn normalize_base_url_trims_trailing_slash_and_path_prefix() {
        let normalized =
            normalize_base_url("  http://192.168.0.20:9696/prowlarr/  ".to_string()).expect("url");

        assert_eq!(
            normalized.as_deref(),
            Some("http://192.168.0.20:9696/prowlarr")
        );
    }
}
