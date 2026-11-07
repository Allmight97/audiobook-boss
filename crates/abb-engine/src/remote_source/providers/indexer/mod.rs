mod api_key;
mod connection;
mod prowlarr;

use secrecy::SecretString;

use crate::app_settings::IndexerConnection;
use crate::errors::{AppError, Result};
use crate::remote_source::types::{
    ProviderId, RemoteAccountStatus, RemoteIndexerConnectionUpdate, RemoteRelease,
    RemoteReleaseGrabRequest, RemoteReleaseGrabResponse, RemoteReleaseSearchRequest,
    RemoteReleaseSearchResponse, RemoteSourceAccountState, RemoteSourceProviderCapabilities,
};
use crate::remote_source::vault::{account_message, SecretVault};

use connection::{api_key_configured, apply_key_change, configured_connection, draft_credentials};
use prowlarr::{build_search_params, ProwlarrSearchOutcome};

pub(in crate::remote_source) use api_key::validate_api_key;
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
        connection: &IndexerConnection,
        vault: &dyn SecretVault,
    ) -> Result<RemoteSourceAccountState> {
        let survives = vault.survives_restart();
        if !api_key_configured(connection, vault)? {
            return Ok(RemoteSourceAccountState {
                provider_id: ProviderId::Indexer,
                status: RemoteAccountStatus::NeedsAuth,
                message: account_message(
                    Some("Configure Indexer URL and API key in Settings before searching."),
                    survives,
                ),
            });
        }

        Ok(RemoteSourceAccountState {
            provider_id: ProviderId::Indexer,
            status: RemoteAccountStatus::Connected,
            message: account_message(None, survives),
        })
    }

    pub(in crate::remote_source) fn api_key_configured(
        connection: &IndexerConnection,
        vault: &dyn SecretVault,
    ) -> Result<bool> {
        api_key_configured(connection, vault)
    }

    pub(in crate::remote_source) fn apply_key_change(
        vault: &dyn SecretVault,
        base_url: &str,
        key_change: Option<String>,
    ) -> Result<bool> {
        apply_key_change(vault, base_url, key_change)
    }

    pub(in crate::remote_source) fn configured_connection(
        connection: IndexerConnection,
        vault: &dyn SecretVault,
    ) -> Result<ConfiguredIndexerConnection> {
        configured_connection(connection, vault)
    }

    pub(in crate::remote_source) fn draft_credentials(
        vault: &dyn SecretVault,
        update: RemoteIndexerConnectionUpdate,
    ) -> Result<(String, SecretString)> {
        draft_credentials(vault, update)
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
                &request.release,
                &request_id.to_string(),
            )
            .await;
        let outcome = match &result {
            Ok(outcome) if outcome.accepted => "handoff_confirmed",
            Ok(outcome) if outcome.unconfirmed => "unconfirmed",
            Ok(_) => "rejected",
            Err(_) => "failed",
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
        unconfirmed: outcome.unconfirmed,
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
#[path = "indexer_tests.rs"]
mod tests;
