use std::io;
use std::path::{Path, PathBuf};

use secrecy::SecretString;

use crate::errors::{AppError, Result};
use crate::remote_source::types::{RemoteIndexerConnection, RemoteIndexerConnectionUpdate};
use crate::remote_source::vault::SecretVault;

// Keys are host-scoped: a failed settings save must never pair an old host with
// a new host's credential. Old slots remain available for deliberate reconnection.
pub(super) fn api_key_vault_key(base_url: &str) -> String {
    format!("indexer.api_key:{base_url}")
}
pub(super) const DEFAULT_CATEGORY_IDS: &[u32] = &[3000, 3030];

const CONNECTION_FILE_NAME: &str = "remote-source-indexer-connection.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredIndexerConnection {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default = "default_category_ids")]
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

fn normalize_category_ids(ids: Vec<u32>) -> Vec<u32> {
    let mut normalized: Vec<u32> = ids.into_iter().filter(|id| *id > 0).collect();
    normalized.sort_unstable();
    normalized.dedup();
    if normalized.is_empty() {
        default_category_ids()
    } else {
        normalized
    }
}

pub(super) fn get_connection(
    config_dir: &Path,
    vault: &dyn SecretVault,
) -> Result<RemoteIndexerConnection> {
    let stored = load_stored_connection(config_dir)?;
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
    let previous = load_stored_connection(config_dir)?;
    let mut stored = previous.clone();
    if let Some(base_url) = update.base_url {
        stored.base_url = normalize_base_url(base_url)?;
    }
    if let Some(category_ids) = update.category_ids {
        stored.category_ids = normalize_category_ids(category_ids);
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

pub(super) fn read_api_key(
    vault: &dyn SecretVault,
    base_url: Option<&str>,
) -> Result<Option<SecretString>> {
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
        None => load_stored_connection(config_dir)?.base_url,
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

fn load_stored_connection(config_dir: &Path) -> Result<StoredIndexerConnection> {
    let path = connection_path(config_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(StoredIndexerConnection::default());
        }
        Err(error) => return Err(AppError::Io(error)),
    };

    let mut stored: StoredIndexerConnection = serde_json::from_str(&content).map_err(|error| {
        AppError::InvalidInput(format!(
            "Indexer connection settings are invalid. Remove remote-source-indexer-connection.json from the app configuration folder, then configure Indexer again. ({error})"
        ))
    })?;
    stored.base_url = stored
        .base_url
        .map(normalize_base_url)
        .transpose()
        .map_err(|error| {
            AppError::InvalidInput(format!(
                "Stored Indexer URL is invalid: {error} Remove remote-source-indexer-connection.json from the app configuration folder, then configure Indexer again."
            ))
        })?
        .flatten();
    stored.category_ids = normalize_category_ids(stored.category_ids);
    Ok(stored)
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
mod tests {
    use super::*;
    use secrecy::SecretString;
    use tempfile::TempDir;

    use secrecy::ExposeSecret;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct TestVault {
        secrets: Mutex<HashMap<String, SecretString>>,
        fail_writes: bool,
    }

    impl SecretVault for TestVault {
        fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
            Ok(self
                .secrets
                .lock()
                .expect("test vault lock")
                .get(key)
                .cloned())
        }
        fn set_secret(&self, key: &str, value: SecretString) -> Result<()> {
            if self.fail_writes {
                return Err(AppError::General("vault unavailable".to_string()));
            }
            self.secrets
                .lock()
                .expect("test vault lock")
                .insert(key.to_string(), value);
            Ok(())
        }
        fn delete_secret(&self, key: &str) -> Result<()> {
            self.secrets.lock().expect("test vault lock").remove(key);
            Ok(())
        }
    }

    fn update(url: Option<&str>, key: Option<&str>) -> RemoteIndexerConnectionUpdate {
        RemoteIndexerConnectionUpdate {
            base_url: url.map(str::to_string),
            api_key: key.map(str::to_string),
            category_ids: None,
            clear_api_key: None,
        }
    }

    #[test]
    fn missing_connection_file_returns_defaults() {
        let temp = TempDir::new().expect("temp dir");
        let vault = TestVault::default();

        let connection = get_connection(temp.path(), &vault).expect("connection");

        assert_eq!(connection.base_url, None);
        assert_eq!(connection.category_ids, vec![3000, 3030]);
        assert!(!connection.api_key_configured);
    }

    #[test]
    fn rejects_url_credentials_before_saving_or_testing() {
        let temp = TempDir::new().expect("temp dir");
        let vault = TestVault::default();
        let saved = update_connection(
            temp.path(),
            &vault,
            update(Some("https://indexer.test"), Some("saved-key")),
        )
        .expect("save valid connection");
        for url in [
            "https://user:password@indexer.test",
            "http://user@indexer.test:9696",
            "https://:password@indexer.test",
        ] {
            let change = || update(Some(url), Some("replacement-key"));
            let error = update_connection(temp.path(), &vault, change())
                .expect_err("URL credentials must not be persisted");
            assert!(error.to_string().contains("must not contain credentials"));
            assert!(!error.to_string().contains(url));
            assert!(draft_credentials(temp.path(), &vault, change()).is_err());
            assert_eq!(get_connection(temp.path(), &vault).expect("reload"), saved);
            assert_eq!(
                read_api_key(&vault, saved.base_url.as_deref())
                    .expect("read key")
                    .expect("saved key")
                    .expose_secret(),
                "saved-key"
            );
        }
    }

    #[test]
    fn rejects_persisted_url_credentials_before_exposing_or_using_them() {
        let temp = TempDir::new().expect("temp dir");
        let vault = TestVault::default();
        std::fs::write(
            connection_path(temp.path()),
            r#"{"baseUrl":"https://user:old-password@indexer.test","categoryIds":[3030]}"#,
        )
        .expect("write previously accepted connection");

        let errors = [
            get_connection(temp.path(), &vault).expect_err("do not expose stored credentials"),
            draft_credentials(temp.path(), &vault, update(None, Some("api-key")))
                .expect_err("do not use stored credentials for a connection test"),
        ];
        for error in errors {
            let message = error.to_string();
            assert!(message.contains("must not contain credentials"));
            assert!(message.contains("remote-source-indexer-connection.json"));
            assert!(!message.contains("old-password"));
            assert!(!message.contains("indexer.test"));
        }
    }

    #[test]
    fn preserves_explicit_categories_and_defaults_empty_categories() {
        let temp = TempDir::new().expect("temp dir");
        let vault = TestVault::default();
        for (categories, expected) in [(vec![3030], vec![3030]), (vec![], vec![3000, 3030])] {
            let mut change = update(None, None);
            change.category_ids = Some(categories);
            update_connection(temp.path(), &vault, change).expect("save categories");
            assert_eq!(
                get_connection(temp.path(), &vault)
                    .expect("reload")
                    .category_ids,
                expected
            );
        }
    }

    #[test]
    fn persists_multiple_category_ids() {
        let temp = TempDir::new().expect("temp dir");
        let vault = TestVault::default();

        update_connection(
            temp.path(),
            &vault,
            RemoteIndexerConnectionUpdate {
                base_url: Some("http://192.168.0.20:9696".to_string()),
                category_ids: Some(vec![3000, 3030, 3030]),
                api_key: None,
                clear_api_key: None,
            },
        )
        .expect("update");

        let connection = get_connection(temp.path(), &vault).expect("connection");
        assert_eq!(connection.category_ids, vec![3000, 3030]);
    }

    #[test]
    fn keys_are_write_only_host_scoped_and_clearable() {
        let temp = TempDir::new().expect("temporary config directory");
        let vault = TestVault::default();
        let saved = update_connection(
            temp.path(),
            &vault,
            update(Some(" http://one.test/proxy/ "), Some(" secret-one ")),
        )
        .expect("save first host connection");
        assert_eq!(saved.base_url.as_deref(), Some("http://one.test/proxy"));
        assert!(saved.api_key_configured);
        let disk = std::fs::read_to_string(connection_path(temp.path()))
            .expect("read persisted connection JSON");
        assert!(!disk.contains("secret-one"));
        assert!(!serde_json::to_string(&saved)
            .expect("serialize public connection")
            .contains("secret-one"));
        let other = update_connection(temp.path(), &vault, update(Some("http://two.test"), None))
            .expect("save second host without key");
        assert!(!other.api_key_configured);
        let previous = update_connection(
            temp.path(),
            &vault,
            update(Some("http://one.test/proxy"), None),
        )
        .expect("restore first host connection");
        assert!(previous.api_key_configured);
        let mut clear = update(None, None);
        clear.clear_api_key = Some(true);
        assert!(
            !update_connection(temp.path(), &vault, clear)
                .expect("clear current host key")
                .api_key_configured
        );
    }

    #[test]
    fn draft_test_uses_current_host_key_without_writes() {
        let temp = TempDir::new().expect("temporary config directory");
        let vault = TestVault::default();
        update_connection(
            temp.path(),
            &vault,
            update(Some("http://saved.test"), Some("saved-key")),
        )
        .expect("save baseline connection");
        let before =
            std::fs::read(connection_path(temp.path())).expect("read persisted connection bytes");
        let (url, key) = draft_credentials(
            temp.path(),
            &vault,
            update(Some(" http://draft.test/proxy/ "), Some(" draft-key ")),
        )
        .expect("resolve supplied draft credentials");
        assert_eq!(url, "http://draft.test/proxy");
        assert_eq!(key.expose_secret(), "draft-key");
        assert!(
            draft_credentials(temp.path(), &vault, update(Some("http://draft.test"), None))
                .is_err()
        );
        let (_, saved_key) =
            draft_credentials(temp.path(), &vault, update(Some("http://saved.test"), None))
                .expect("resolve saved host credentials");
        assert_eq!(saved_key.expose_secret(), "saved-key");
        assert_eq!(
            std::fs::read(connection_path(temp.path())).expect("read persisted connection bytes"),
            before
        );
        assert_eq!(vault.secrets.lock().expect("test vault lock").len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn failed_settings_save_does_not_change_any_host_key() {
        let temp = TempDir::new().expect("temporary config directory");
        let vault = TestVault::default();
        update_connection(
            temp.path(),
            &vault,
            update(Some("http://old.test"), Some("old-key")),
        )
        .expect("save baseline connection");
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::metadata(temp.path())
            .expect("read config directory metadata")
            .permissions();
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o500))
            .expect("deny config directory writes");
        let results: Vec<_> = ["http://old.test", "http://new.test"]
            .into_iter()
            .map(|host| {
                let mut change = update(Some(host), Some("new-key"));
                change.category_ids = Some(vec![3000]);
                update_connection(temp.path(), &vault, change)
            })
            .collect();
        std::fs::set_permissions(temp.path(), permissions)
            .expect("restore config directory permissions");
        assert!(results.into_iter().all(|result| result.is_err()));
        let saved = get_connection(temp.path(), &vault).expect("read saved connection");
        assert_eq!(saved.base_url.as_deref(), Some("http://old.test"));
        assert_eq!(saved.category_ids, [3000, 3030]);
        assert_eq!(
            std::fs::read_dir(temp.path())
                .expect("list config files")
                .count(),
            1
        );
        assert_eq!(
            read_api_key(&vault, Some("http://old.test"))
                .expect("read original host key")
                .expect("original host key remains present")
                .expose_secret(),
            "old-key"
        );
        assert!(read_api_key(&vault, Some("http://new.test"))
            .expect("read new host key")
            .is_none());
    }

    #[test]
    fn vault_failure_reports_partial_settings_save_without_cross_host_key() {
        let temp = TempDir::new().expect("temporary config directory");
        let vault = TestVault::default();
        update_connection(
            temp.path(),
            &vault,
            update(Some("http://old.test"), Some("old-key")),
        )
        .expect("save baseline connection");
        let vault = TestVault {
            fail_writes: true,
            ..vault
        };
        let error = update_connection(
            temp.path(),
            &vault,
            update(Some("http://new.test"), Some("new-key")),
        )
        .expect_err("API key write should fail");
        assert!(error.to_string().contains("settings were saved"));
        let saved = get_connection(temp.path(), &vault).expect("read saved connection");
        assert_eq!(saved.base_url.as_deref(), Some("http://new.test"));
        assert!(!saved.api_key_configured);
        assert_eq!(
            read_api_key(&vault, Some("http://old.test"))
                .expect("read original host key")
                .expect("original host key remains present")
                .expose_secret(),
            "old-key"
        );
    }
}
