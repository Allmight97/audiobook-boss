use super::*;
use crate::remote_source::test_support::MemoryVault;
use secrecy::ExposeSecret;

fn update(url: Option<&str>, key: Option<&str>) -> RemoteIndexerConnectionUpdate {
    RemoteIndexerConnectionUpdate {
        base_url: url.map(str::to_string),
        api_key: key.map(str::to_string),
    }
}

fn saved(url: &str) -> IndexerConnection {
    IndexerConnection {
        base_url: Some(url.to_string()),
        category_ids: vec![3000, 3030],
    }
}

fn key_of(vault: &MemoryVault, url: &str) -> Option<String> {
    read_api_key(vault, Some(url))
        .expect("read key")
        .map(|key| key.expose_secret().to_string())
}

#[test]
fn draft_testing_rejects_url_credentials_and_leaves_the_saved_key_alone() {
    let vault = MemoryVault::default();
    apply_key_change(&vault, "https://indexer.test", Some("saved-key".into())).expect("save key");
    for url in [
        "https://user:password@indexer.test",
        "http://user@indexer.test:9696",
        "https://:password@indexer.test",
    ] {
        let error = draft_credentials(&vault, update(Some(url), Some("replacement-key")))
            .expect_err("URL credentials must not be tested");
        assert!(error.to_string().contains("must not contain credentials"));
        assert!(!error.to_string().contains(url));
        assert_eq!(
            key_of(&vault, "https://indexer.test").as_deref(),
            Some("saved-key")
        );
    }
}

#[test]
fn configured_request_resolves_the_saved_host_key_once() {
    let vault = MemoryVault::default();
    apply_key_change(
        &vault,
        "https://indexer.test/proxy",
        Some("saved-key".into()),
    )
    .expect("save key");
    vault.clear_reads();

    let connection = configured_connection(saved("https://indexer.test/proxy"), &vault)
        .expect("resolve connection");
    assert_eq!(connection.base_url, "https://indexer.test/proxy");
    assert_eq!(connection.category_ids, [3000, 3030]);
    assert_eq!(connection.api_key.expose_secret(), "saved-key");
    assert_eq!(vault.read_count(), 1);
}

#[test]
fn an_unconfigured_connection_is_never_resolved_or_read_from_the_vault() {
    let vault = MemoryVault::default();

    assert!(configured_connection(IndexerConnection::default(), &vault).is_err());
    assert!(configured_connection(saved("https://indexer.test"), &vault).is_err());
    assert!(!api_key_configured(&IndexerConnection::default(), &vault).expect("fact"));
    assert_eq!(vault.read_count(), 1);
}

#[test]
fn keys_are_write_only_and_scoped_to_their_host() {
    let vault = MemoryVault::default();
    assert!(
        apply_key_change(&vault, "http://one.test/proxy", Some("secret-one".into()))
            .expect("save first host key")
    );
    assert_eq!(
        key_of(&vault, "http://one.test/proxy").as_deref(),
        Some("secret-one")
    );
    assert!(!apply_key_change(&vault, "http://two.test", None).expect("second host has no key"));
    assert!(apply_key_change(&vault, "http://one.test/proxy", None)
        .expect("first host key is still there"));
}

#[test]
fn draft_test_uses_current_host_key_without_writes() {
    let vault = MemoryVault::default();
    apply_key_change(&vault, "http://saved.test", Some("saved-key".into()))
        .expect("save baseline key");
    let (url, key) = draft_credentials(
        &vault,
        update(Some(" http://draft.test/proxy/ "), Some(" draft-key ")),
    )
    .expect("resolve supplied draft credentials");
    assert_eq!(url, "http://draft.test/proxy");
    assert_eq!(key.expose_secret(), "draft-key");
    assert!(draft_credentials(&vault, update(Some("http://draft.test"), None)).is_err());
    assert!(draft_credentials(&vault, update(None, Some("key"))).is_err());
    let (_, saved_key) = draft_credentials(&vault, update(Some("http://saved.test"), None))
        .expect("resolve saved host credentials");
    assert_eq!(saved_key.expose_secret(), "saved-key");
    assert_eq!(vault.secret_count(), 1);
}
