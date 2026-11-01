use super::*;
use secrecy::SecretString;
use tempfile::TempDir;

#[derive(Default)]
struct TestVault {
    secret: Option<SecretString>,
    ephemeral: bool,
}

impl SecretVault for TestVault {
    fn survives_restart(&self) -> bool {
        !self.ephemeral
    }

    fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
        assert!(key.starts_with("indexer.api_key:"));
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
        let temp = TempDir::new().expect("temporary config directory");
        let vault = TestVault {
            secret: secret.map(|key| SecretString::from(key.to_string())),
            ephemeral: false,
        };
        update_connection(
            temp.path(),
            &vault,
            RemoteIndexerConnectionUpdate {
                base_url: url.map(str::to_string),
                api_key: None,
                clear_api_key: None,
                category_ids: None,
            },
        )
        .expect("save account configuration");
        let state =
            IndexerProvider::account_state(temp.path(), &vault).expect("read account state");
        assert_eq!(state.provider_id, ProviderId::Indexer);
        assert_eq!(state.status, expected);
        assert_eq!(
            state.account.is_some(),
            expected == RemoteAccountStatus::Connected
        );
    }
}

#[test]
fn account_state_says_the_key_will_not_be_remembered_when_the_store_does_not_survive_restart() {
    let temp = TempDir::new().expect("temporary config directory");
    let vault = TestVault {
        ephemeral: true,
        ..TestVault::default()
    };
    let needs_key =
        IndexerProvider::account_state(temp.path(), &vault).expect("read account state");
    assert_eq!(
        needs_key.message.as_deref(),
        Some(concat!(
            "Configure Indexer URL and API key in Settings before searching. ",
            "Sign-in won't be remembered after a restart."
        ))
    );

    update_connection(
        temp.path(),
        &vault,
        RemoteIndexerConnectionUpdate {
            base_url: Some("http://indexer.test".to_string()),
            api_key: None,
            clear_api_key: None,
            category_ids: None,
        },
    )
    .expect("save url");
    let vault = TestVault {
        secret: Some(SecretString::from("key".to_string())),
        ephemeral: true,
    };
    let connected =
        IndexerProvider::account_state(temp.path(), &vault).expect("read account state");
    assert_eq!(connected.status, RemoteAccountStatus::Connected);
    assert_eq!(
        connected.message.as_deref(),
        Some("Sign-in won't be remembered after a restart.")
    );
}
