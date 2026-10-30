use super::*;
use secrecy::SecretString;
use tempfile::TempDir;

use secrecy::ExposeSecret;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct TestVault {
    secrets: Mutex<HashMap<String, SecretString>>,
    reads: Mutex<usize>,
    fail_writes: bool,
}

impl SecretVault for TestVault {
    fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
        *self.reads.lock().expect("read counter lock") += 1;
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
fn configured_request_resolves_the_saved_host_key_once() {
    let temp = TempDir::new().expect("temp dir");
    let vault = TestVault::default();
    update_connection(
        temp.path(),
        &vault,
        update(Some("https://indexer.test/proxy/"), Some("saved-key")),
    )
    .expect("save connection");
    *vault.reads.lock().expect("read counter lock") = 0;

    let connection = configured_connection(temp.path(), &vault).expect("resolve connection");
    assert_eq!(connection.base_url, "https://indexer.test/proxy");
    assert_eq!(connection.category_ids, [3000, 3030]);
    assert_eq!(connection.api_key.expose_secret(), "saved-key");
    assert_eq!(*vault.reads.lock().expect("read counter lock"), 1);
}

#[test]
fn a_stored_url_with_credentials_is_never_exposed_or_used() {
    let temp = TempDir::new().expect("temp dir");
    let vault = TestVault::default();
    std::fs::write(
        connection_path(temp.path()),
        "url = \"https://user:old-password@indexer.test\"\ncategories = [3030]\n",
    )
    .expect("write a connection edited outside ABB");

    let connection = get_connection(temp.path(), &vault).expect("connection");
    assert_eq!(connection.base_url, None);
    assert!(draft_credentials(temp.path(), &vault, update(None, Some("api-key"))).is_err());
    assert!(configured_connection(temp.path(), &vault).is_err());
    assert_eq!(*vault.reads.lock().expect("read counter lock"), 0);
}

#[test]
fn saves_exactly_the_chosen_categories_and_refuses_none() {
    let temp = TempDir::new().expect("temp dir");
    let vault = TestVault::default();
    assert_eq!(
        get_connection(temp.path(), &vault)
            .expect("defaults")
            .category_ids,
        [3000, 3030]
    );
    let mut change = update(None, None);
    change.category_ids = Some(vec![3030]);
    update_connection(temp.path(), &vault, change).expect("save categories");
    let mut empty = update(None, None);
    empty.category_ids = Some(Vec::new());
    assert!(update_connection(temp.path(), &vault, empty).is_err());
    assert_eq!(
        get_connection(temp.path(), &vault)
            .expect("reload")
            .category_ids,
        [3030]
    );
}

#[test]
fn persists_multiple_category_ids() {
    let temp = TempDir::new().expect("temp dir");
    let vault = TestVault::default();

    update_connection(
        temp.path(),
        &vault,
        RemoteIndexerConnectionUpdate {
            base_url: Some("http://192.0.2.20:9696".to_string()),
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
    let disk =
        std::fs::read_to_string(connection_path(temp.path())).expect("read persisted connection");
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
        draft_credentials(temp.path(), &vault, update(Some("http://draft.test"), None)).is_err()
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
    // chmod is not enough: root ignores a 0o500 directory. A directory at
    // the file path makes the atomic replace fail for every uid.
    let path = connection_path(temp.path());
    let original = std::fs::read(&path).expect("read baseline connection");
    std::fs::remove_file(&path).expect("remove connection file");
    std::fs::create_dir(&path).expect("block connection path with a directory");
    let results: Vec<_> = ["http://old.test", "http://new.test"]
        .into_iter()
        .map(|host| {
            let mut change = update(Some(host), Some("new-key"));
            change.category_ids = Some(vec![3000]);
            update_connection(temp.path(), &vault, change)
        })
        .collect();
    std::fs::remove_dir(&path).expect("remove blocker directory");
    std::fs::write(&path, original).expect("restore connection file");
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
