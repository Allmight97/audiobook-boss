use super::*;
use crate::remote_source::test_support::MemoryVault;

#[test]
fn account_state_requires_both_url_and_current_host_key() {
    for (url, secret, expected) in [
        (None, None, RemoteAccountStatus::NeedsAuth),
        (
            Some("http://indexer.test"),
            None,
            RemoteAccountStatus::NeedsAuth,
        ),
        (None, Some("key"), RemoteAccountStatus::NeedsAuth),
        (
            Some("http://indexer.test"),
            Some("key"),
            RemoteAccountStatus::Connected,
        ),
    ] {
        let vault = secret.map_or_else(MemoryVault::default, |key| {
            MemoryVault::default().with_secret("indexer.api_key:http://indexer.test", key)
        });
        let connection = IndexerConnection {
            base_url: url.map(str::to_string),
            ..IndexerConnection::default()
        };
        let state =
            IndexerProvider::account_state(&connection, &vault).expect("read account state");
        assert_eq!(state.provider_id, ProviderId::Indexer);
        assert_eq!(state.status, expected);
    }
}

#[test]
fn account_state_says_the_key_will_not_be_remembered_when_the_store_does_not_survive_restart() {
    let vault = MemoryVault::default().ephemeral();
    let needs_key = IndexerProvider::account_state(&IndexerConnection::default(), &vault)
        .expect("read account state");
    assert_eq!(
        needs_key.message.as_deref(),
        Some(concat!(
            "Configure Indexer URL and API key in Settings before searching. ",
            "Sign-in won't be remembered after a restart."
        ))
    );

    let connection = IndexerConnection {
        base_url: Some("http://indexer.test".to_string()),
        ..IndexerConnection::default()
    };
    let vault = MemoryVault::default()
        .with_secret("indexer.api_key:http://indexer.test", "key")
        .ephemeral();
    let connected =
        IndexerProvider::account_state(&connection, &vault).expect("read account state");
    assert_eq!(connected.status, RemoteAccountStatus::Connected);
    assert_eq!(
        connected.message.as_deref(),
        Some("Sign-in won't be remembered after a restart.")
    );
}
