mod connection;
mod prowlarr;

use std::path::Path;

use secrecy::SecretString;

use crate::errors::{AppError, Result};
use crate::remote_source::types::{
    AccountRef, ProviderId, RemoteAccountStatus, RemoteAcquisitionFailureKind, RemoteAuthFlow,
    RemoteIndexerConnection, RemoteIndexerConnectionUpdate, RemoteRelease,
    RemoteReleaseGrabRequest, RemoteReleaseGrabResponse, RemoteReleaseSearchRequest,
    RemoteReleaseSearchResponse, RemoteSourceAccountState, RemoteSourceProviderCapabilities,
};
use crate::remote_source::vault::SecretVault;

use connection::{get_connection, update_connection, API_KEY_VAULT_KEY};
use prowlarr::{build_search_params_with_category, ProwlarrSearchOutcome};

pub(in crate::remote_source) use prowlarr::ReqwestProwlarrAdapter;

#[derive(Debug, Clone)]
pub(in crate::remote_source) struct IndexerProvider;

impl IndexerProvider {
    pub(in crate::remote_source) fn capabilities() -> RemoteSourceProviderCapabilities {
        RemoteSourceProviderCapabilities {
            provider_id: ProviderId::Indexer,
            label: "Indexer".to_string(),
            auth_flow: RemoteAuthFlow::ApiKey,
            supports_library_scan: false,
            supports_paged_scan: false,
            supports_typeahead_filter: false,
            supports_supplemental_pdf: false,
            supports_materialized_audio: false,
            supports_release_search: true,
            supports_release_grab: true,
            supports_refresh: false,
            requires_live_session: false,
            known_unsupported_reasons: vec![
                RemoteAcquisitionFailureKind::IndexerConnectionRequired,
                RemoteAcquisitionFailureKind::ReleaseSearchFailed,
                RemoteAcquisitionFailureKind::ReleaseGrabFailed,
            ],
        }
    }

    pub(in crate::remote_source) fn account_state(
        config_dir: &Path,
        vault: &dyn SecretVault,
    ) -> Result<RemoteSourceAccountState> {
        let connection = get_connection(config_dir, vault)?;
        if connection.base_url.is_none() || !connection.api_key_configured {
            return Ok(RemoteSourceAccountState {
                provider_id: ProviderId::Indexer,
                status: RemoteAccountStatus::NeedsAuth,
                account: None,
                message: Some(
                    "Configure Indexer URL and API key in Settings before searching.".to_string(),
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
            message: None,
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

    pub(in crate::remote_source) async fn search_releases(
        config_dir: &Path,
        vault: &dyn SecretVault,
        adapter: &ReqwestProwlarrAdapter,
        request: RemoteReleaseSearchRequest,
    ) -> Result<RemoteReleaseSearchResponse> {
        let connection = get_connection(config_dir, vault)?;
        let (base_url, api_key) = require_connection(&connection, vault)?;
        let params = build_search_params_with_category(&request, connection.category_id)?;
        let outcome = adapter.search(&base_url, &api_key, &params).await?;
        Ok(map_search_outcome(outcome))
    }

    pub(in crate::remote_source) async fn grab_release(
        config_dir: &Path,
        vault: &dyn SecretVault,
        adapter: &ReqwestProwlarrAdapter,
        request: RemoteReleaseGrabRequest,
    ) -> Result<RemoteReleaseGrabResponse> {
        validate_grab_release(&request.release)?;
        let connection = get_connection(config_dir, vault)?;
        let (base_url, api_key) = require_connection(&connection, vault)?;
        let outcome = adapter
            .grab(
                &base_url,
                &api_key,
                &request.release.guid,
                request.release.indexer_id,
            )
            .await?;
        Ok(map_grab_outcome(outcome))
    }

    pub(in crate::remote_source) async fn test_connection(
        config_dir: &Path,
        vault: &dyn SecretVault,
        adapter: &ReqwestProwlarrAdapter,
    ) -> Result<crate::remote_source::types::RemoteIndexerConnectionTestResult> {
        let connection = get_connection(config_dir, vault)?;
        let (base_url, api_key) = require_connection(&connection, vault)?;
        let outcome = adapter.system_status(&base_url, &api_key).await?;
        Ok(
            crate::remote_source::types::RemoteIndexerConnectionTestResult {
                ok: outcome.ok,
                message: outcome.message,
            },
        )
    }

    pub(in crate::remote_source) fn logout(vault: &dyn SecretVault) -> Result<()> {
        vault.delete_secret(API_KEY_VAULT_KEY)
    }
}

fn require_connection(
    connection: &RemoteIndexerConnection,
    vault: &dyn SecretVault,
) -> Result<(String, SecretString)> {
    let base_url = connection.base_url.clone().ok_or_else(|| {
        AppError::InvalidInput("Configure Indexer URL in Settings before continuing.".to_string())
    })?;
    let api_key = vault.get_secret(API_KEY_VAULT_KEY)?.ok_or_else(|| {
        AppError::InvalidInput(
            "Configure Indexer API key in Settings before continuing.".to_string(),
        )
    })?;
    Ok((base_url, api_key))
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
    fn account_state_reports_needs_auth_when_unconfigured() {
        let temp = TempDir::new().expect("temp dir");
        let vault = TestVault::default();

        let state = IndexerProvider::account_state(temp.path(), &vault).expect("state");

        assert_eq!(state.provider_id, ProviderId::Indexer);
        assert_eq!(state.status, RemoteAccountStatus::NeedsAuth);
        assert!(state.account.is_none());
    }
}
