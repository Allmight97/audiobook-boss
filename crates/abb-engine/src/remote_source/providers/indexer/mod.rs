mod connection;
mod prowlarr;

use std::path::Path;

use secrecy::SecretString;

use crate::errors::{AppError, Result};
use crate::remote_source::types::{
    AccountRef, ProviderId, RemoteAccountStatus, RemoteIndexerConnection,
    RemoteIndexerConnectionUpdate, RemoteRelease, RemoteReleaseGrabRequest,
    RemoteReleaseGrabResponse, RemoteReleaseSearchRequest, RemoteReleaseSearchResponse,
    RemoteSourceAccountState, RemoteSourceProviderCapabilities,
};
use crate::remote_source::vault::{account_message, SecretVault};

use connection::{
    api_key_vault_key, configured_connection, draft_credentials, get_connection, update_connection,
};
use prowlarr::{build_search_params, ProwlarrSearchOutcome};

pub(in crate::remote_source) use connection::ConfiguredIndexerConnection;
pub(in crate::remote_source) use prowlarr::ReqwestProwlarrAdapter;

#[derive(Debug, Clone)]
pub(in crate::remote_source) struct IndexerProvider;

impl IndexerProvider {
    pub(in crate::remote_source) fn capabilities() -> RemoteSourceProviderCapabilities {
        RemoteSourceProviderCapabilities {
            provider_id: ProviderId::Indexer,
            label: "Indexer".to_string(),
        }
    }

    pub(in crate::remote_source) fn account_state(
        config_dir: &Path,
        vault: &dyn SecretVault,
    ) -> Result<RemoteSourceAccountState> {
        let connection = get_connection(config_dir, vault)?;
        let survives = vault.survives_restart();
        if connection.base_url.is_none() || !connection.api_key_configured {
            return Ok(RemoteSourceAccountState {
                provider_id: ProviderId::Indexer,
                status: RemoteAccountStatus::NeedsAuth,
                account: None,
                message: account_message(
                    Some("Configure Indexer URL and API key in Settings before searching."),
                    survives,
                ),
            });
        }

        Ok(RemoteSourceAccountState {
            provider_id: ProviderId::Indexer,
            status: RemoteAccountStatus::Connected,
            account: Some(AccountRef {
                provider_id: ProviderId::Indexer,
                account_id: "indexer".to_string(),
                display_name: connection.base_url.clone().unwrap_or_default(),
            }),
            message: account_message(None, survives),
        })
    }

    pub(in crate::remote_source) fn get_connection(
        config_dir: &Path,
        vault: &dyn SecretVault,
    ) -> Result<RemoteIndexerConnection> {
        get_connection(config_dir, vault)
    }

    pub(in crate::remote_source) fn update_connection(
        config_dir: &Path,
        vault: &dyn SecretVault,
        update: RemoteIndexerConnectionUpdate,
    ) -> Result<RemoteIndexerConnection> {
        update_connection(config_dir, vault, update)
    }

    pub(in crate::remote_source) fn configured_connection(
        config_dir: &Path,
        vault: &dyn SecretVault,
    ) -> Result<ConfiguredIndexerConnection> {
        configured_connection(config_dir, vault)
    }

    pub(in crate::remote_source) fn draft_credentials(
        config_dir: &Path,
        vault: &dyn SecretVault,
        update: RemoteIndexerConnectionUpdate,
    ) -> Result<(String, SecretString)> {
        draft_credentials(config_dir, vault, update)
    }

    pub(in crate::remote_source) async fn search_releases(
        adapter: &ReqwestProwlarrAdapter,
        connection: ConfiguredIndexerConnection,
        request: RemoteReleaseSearchRequest,
    ) -> Result<RemoteReleaseSearchResponse> {
        let params = build_search_params(&request, &connection.category_ids)?;
        let outcome = adapter
            .search(&connection.base_url, &connection.api_key, &params)
            .await?;
        Ok(map_search_outcome(outcome))
    }

    pub(in crate::remote_source) async fn grab_release(
        adapter: &ReqwestProwlarrAdapter,
        connection: ConfiguredIndexerConnection,
        request: RemoteReleaseGrabRequest,
    ) -> Result<RemoteReleaseGrabResponse> {
        validate_grab_release(&request.release)?;
        let request_id = uuid::Uuid::new_v4();
        let started = std::time::Instant::now();
        log::info!(
            "remote_source indexer_grab request_id={} stage=start indexer_id={} indexer={:?} title={:?} protocol={:?}",
            request_id, request.release.indexer_id, request.release.indexer,
            request.release.title, request.release.protocol
        );
        let result = adapter
            .grab(
                &connection.base_url,
                &connection.api_key,
                &request.release.guid,
                request.release.indexer_id,
                &request_id.to_string(),
            )
            .await;
        let outcome = match &result {
            Ok(outcome) if outcome.accepted => "handoff_confirmed",
            Ok(_) => "rejected",
            Err(_) => "unconfirmed",
        };
        log::info!(
            "remote_source indexer_grab request_id={} stage=complete outcome={} elapsed_ms={}",
            request_id,
            outcome,
            started.elapsed().as_millis()
        );
        result.map(map_grab_outcome)
    }

    pub(in crate::remote_source) async fn test_connection(
        adapter: &ReqwestProwlarrAdapter,
        base_url: String,
        api_key: SecretString,
    ) -> Result<crate::remote_source::types::RemoteIndexerConnectionTestResult> {
        let outcome = adapter.system_status(&base_url, &api_key).await?;
        Ok(
            crate::remote_source::types::RemoteIndexerConnectionTestResult {
                ok: outcome.ok,
                message: outcome.message,
            },
        )
    }

    pub(in crate::remote_source) fn logout(
        config_dir: &Path,
        vault: &dyn SecretVault,
    ) -> Result<()> {
        if let Some(url) = get_connection(config_dir, vault)?.base_url {
            vault.delete_secret(&api_key_vault_key(&url))?;
        }
        Ok(())
    }
}

fn map_search_outcome(outcome: ProwlarrSearchOutcome) -> RemoteReleaseSearchResponse {
    RemoteReleaseSearchResponse {
        provider_id: ProviderId::Indexer,
        releases: outcome.releases,
        diagnostics: outcome.diagnostics,
    }
}

fn map_grab_outcome(outcome: prowlarr::ProwlarrGrabOutcome) -> RemoteReleaseGrabResponse {
    RemoteReleaseGrabResponse {
        provider_id: ProviderId::Indexer,
        accepted: outcome.accepted,
        message: outcome.message,
        diagnostics: outcome.diagnostics,
    }
}

fn validate_grab_release(release: &RemoteRelease) -> Result<()> {
    if release.provider_id != ProviderId::Indexer {
        return Err(AppError::InvalidInput(
            "Grab requests must target Indexer releases.".to_string(),
        ));
    }
    if release.guid.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "Select a release before grabbing.".to_string(),
        ));
    }
    Ok(())
}

pub(in crate::remote_source) fn default_category_ids() -> Vec<u32> {
    connection::DEFAULT_CATEGORY_IDS.to_vec()
}

pub(in crate::remote_source) fn normalize_draft_url(url: String) -> Result<String> {
    Ok(connection::normalize_base_url(url)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
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
}
