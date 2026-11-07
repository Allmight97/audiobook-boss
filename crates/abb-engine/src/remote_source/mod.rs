use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

mod cancellation;
mod job_state;
mod materializer;
mod providers;
mod scoped_output;
mod session_lifecycle;
mod staging;
mod types;
mod ui;

const AUTH_REGISTRATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
pub use ui::{
    IndexerDraftSnapshot, IndexerWorkSnapshot, ReleaseGrabSnapshot, ReleaseGrabStatus,
    RemoteDraftStatus, RemoteLibrarySnapshot, RemoteUiIntent, RemoteUiSnapshot,
};
pub(crate) use ui::{RemoteUiResult, RemoteUiRun};
mod vault;

pub(crate) use job_state::AcquisitionOutcome;
use materializer::AaxcleanMaterializer;
use providers::audible::{AudibleProvider, PendingAudibleAuth};
use providers::indexer::{IndexerProvider, ReqwestProwlarrAdapter};
#[cfg(test)]
pub(crate) use session_lifecycle::AcquireTitles;
use session_lifecycle::RemoteAcquisitionLifecycle;
use staging::RemoteSourceStaging;
use types::{
    AcquisitionJob as RemoteAcquisitionJob, AcquisitionPlan as RemoteAcquisitionPlan,
    ProviderId as RemoteProviderId, RemoteAuthCompletionRequest as RemoteAuthCompletion,
    RemoteAuthStartResponse as RemoteAuthStart, RemoteSourceAccountState as RemoteAccountState,
};
use vault::{KeyringSecretVault, SecretVault};

use crate::errors::{AppError, Result};

pub(crate) struct RemoteSourceConfig {
    pub(crate) cache_dir: PathBuf,
    pub(crate) settings: crate::app_settings::SettingsRuntime,
    pub(crate) app_identifier: String,
    pub(crate) power: crate::power::PowerManager,
    pub(crate) host: crate::host::Host,
    pub(crate) tasks: crate::engine_tasks::EngineTasks,
}

pub(crate) type Handoff = Arc<
    dyn Fn(RemoteAcquisitionJob) -> Pin<Box<dyn Future<Output = AcquisitionHandoff> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub(crate) struct RemoteSourceRuntime {
    inner: Arc<RemoteSourceRuntimeInner>,
}

struct RemoteSourceRuntimeInner {
    ui: Arc<Mutex<ui::UiState>>,
    ui_host: crate::host::Host,
    power: crate::power::PowerManager,
    settings: crate::app_settings::SettingsRuntime,
    vault: Box<dyn SecretVault>,
    lifecycle: RemoteAcquisitionLifecycle,
    pending_audible_auth: Mutex<Option<PendingAudibleAuth>>,
    indexer_adapter: ReqwestProwlarrAdapter,
    tasks: crate::engine_tasks::EngineTasks,
    handoff: OnceLock<Handoff>,
    /// A connection Save or Reset holds this exclusively; searches and grabs share it.
    indexer_turn: Arc<tokio::sync::RwLock<()>>,
    /// Releases sent since the last search, by `RemoteRelease::key`.
    sent_releases: Mutex<std::collections::HashSet<String>>,
}

impl RemoteSourceRuntime {
    pub(crate) fn new(config: RemoteSourceConfig) -> Result<Self> {
        let ui = Arc::new(Mutex::new(ui::UiState::seeded(
            &config.settings.indexer_connection(),
        )));
        Ok(Self {
            inner: Arc::new(RemoteSourceRuntimeInner {
                ui: ui.clone(),
                ui_host: config.host.clone(),
                power: config.power,
                settings: config.settings,
                vault: Box::new(KeyringSecretVault::for_app_identifier(
                    &config.app_identifier,
                )),
                lifecycle: RemoteAcquisitionLifecycle::new(
                    RemoteSourceStaging::new(config.cache_dir),
                    AaxcleanMaterializer::new_for_helper(None),
                    config.host,
                    ui,
                    session_lifecycle::audible_acquire(),
                ),
                pending_audible_auth: Mutex::new(None),
                indexer_adapter: ReqwestProwlarrAdapter::new(),
                tasks: config.tasks,
                handoff: OnceLock::new(),
                indexer_turn: Arc::default(),
                sent_releases: Mutex::default(),
            }),
        })
    }

    /// Sets where finished acquisitions hand their files. Set once, by the
    /// engine, after the session exists.
    pub(crate) fn set_handoff(&self, handoff: Handoff) {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "set once by the engine after the session exists"
        )]
        let _ = self.inner.handoff.set(handoff);
    }

    pub(crate) fn staging_root(&self) -> PathBuf {
        self.inner.lifecycle.staging.session_root()
    }

    /// Stops every acquisition; their staged files are removed at the next
    /// start.
    pub(crate) fn abort_acquisitions(&self) {
        self.inner.lifecycle.abort_all_acquisition_tasks();
    }

    pub(crate) fn running_acquisitions(&self) -> Vec<String> {
        self.inner.lifecycle.running_acquisitions()
    }

    pub(crate) fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.inner.lifecycle.cleanup_abandoned_sessions()
    }

    pub(crate) fn account_state(
        &self,
        provider_id: RemoteProviderId,
    ) -> Result<RemoteAccountState> {
        match provider_id {
            RemoteProviderId::Audible => AudibleProvider::account_state(self.inner.vault.as_ref()),
            RemoteProviderId::Indexer => IndexerProvider::account_state(
                &self.inner.settings.indexer_connection(),
                self.inner.vault.as_ref(),
            ),
        }
    }

    pub(crate) fn start_auth(&self, provider_id: RemoteProviderId) -> Result<RemoteAuthStart> {
        match provider_id {
            RemoteProviderId::Audible => {
                let (authorization_url, pending) = AudibleProvider::start_auth()?;
                *self.inner.pending_audible_auth.lock().map_err(|_| {
                    AppError::General("Remote auth state lock failed".to_string())
                })? = Some(pending);
                Ok(RemoteAuthStart {
                    provider_id,
                    authorization_url,
                })
            }
            RemoteProviderId::Indexer => Err(AppError::InvalidInput(
                "Indexer uses Settings URL and API key configuration instead of browser auth."
                    .to_string(),
            )),
        }
    }

    pub(crate) async fn complete_auth(
        &self,
        request: RemoteAuthCompletion,
    ) -> Result<RemoteAccountState> {
        match request.provider_id {
            RemoteProviderId::Audible => {
                let response_url = read_handoff_url(request.response_url_handoff_path)?;
                AudibleProvider::check_response_url(&response_url)?;
                let pending = self
                    .inner
                    .pending_audible_auth
                    .lock()
                    .map_err(|_| AppError::General("Remote auth state lock failed".to_string()))?
                    .take()
                    .ok_or_else(|| {
                        AppError::InvalidInput(
                            "Start Audible auth before completing the handoff.".to_string(),
                        )
                    })?;
                let registration = async {
                    tokio::time::timeout(
                        AUTH_REGISTRATION_TIMEOUT,
                        AudibleProvider::register_auth(pending, &response_url),
                    )
                    .await
                    .map_err(|_| {
                        AppError::General(
                            "Amazon did not finish the sign-in in time. Connect again.".into(),
                        )
                    })?
                };
                let auth = self.inner.tasks.until_closing(registration).await?;
                let runtime = self.clone();
                let write = self.inner.tasks.admit(|| {
                    #[expect(clippy::disallowed_methods, reason = "joined by complete_auth")]
                    let write = tokio::task::spawn_blocking(move || {
                        AudibleProvider::persist_auth(runtime.inner.vault.as_ref(), &auth)
                    });
                    write
                })?;
                write.await.map_err(|_| {
                    AppError::General("Audible credential persistence failed.".into())
                })?
            }
            RemoteProviderId::Indexer => Err(AppError::InvalidInput(
                "Indexer uses Settings URL and API key configuration instead of browser auth."
                    .to_string(),
            )),
        }
    }

    fn disconnect_credentials(&self) -> Result<RemoteAccountState> {
        AudibleProvider::logout(self.inner.vault.as_ref())?;
        let account = self.account_state(RemoteProviderId::Audible);
        {
            let mut state = self.ui();
            state.disconnected();
            state.account_disconnected(account.as_ref().ok().cloned());
        }
        *self
            .inner
            .pending_audible_auth
            .lock()
            .map_err(|_| AppError::General("Remote auth state lock failed".into()))? = None;
        self.inner
            .lifecycle
            .cleanup_logout_sessions_without_handoff(RemoteProviderId::Audible)?;
        self.inner.lifecycle.clear_jobs(RemoteProviderId::Audible)?;
        account
    }

    pub(crate) async fn search_releases(
        &self,
        request: types::RemoteReleaseSearchRequest,
    ) -> Result<types::RemoteReleaseSearchResponse> {
        let _turn = self.indexer_turn()?;
        self.forget_sent_releases();
        let connection = self.configured_indexer_connection().await?;
        self.inner
            .tasks
            .until_closing(IndexerProvider::search_releases(
                &self.inner.indexer_adapter,
                connection,
                request,
            ))
            .await
    }

    pub(crate) async fn grab_release(
        &self,
        request: types::RemoteReleaseGrabRequest,
    ) -> Result<types::RemoteReleaseGrabResponse> {
        let _turn = self.indexer_turn()?;
        let key = request.release.key.clone();
        if !self.sent_releases().insert(key.clone()) {
            return Ok(types::RemoteReleaseGrabResponse {
                provider_id: RemoteProviderId::Indexer,
                accepted: true,
                unconfirmed: false,
                message: "Already sent to downloader.".to_string(),
                diagnostics: Vec::new(),
            });
        }
        let _active_work = self.inner.power.begin();
        let response = async {
            let connection = self.configured_indexer_connection().await?;
            self.inner
                .tasks
                .until_closing(IndexerProvider::grab_release(
                    &self.inner.indexer_adapter,
                    connection,
                    request,
                ))
                .await
        }
        .await;
        if !response.as_ref().is_ok_and(|response| response.accepted) {
            self.sent_releases().remove(&key);
        }
        response
    }

    fn indexer_turn(&self) -> Result<tokio::sync::RwLockReadGuard<'_, ()>> {
        self.inner.indexer_turn.try_read().map_err(|_| {
            AppError::InvalidInput(
                "Wait for the Indexer connection save to finish before searching or grabbing."
                    .to_string(),
            )
        })
    }

    fn sent_releases(&self) -> std::sync::MutexGuard<'_, std::collections::HashSet<String>> {
        self.inner
            .sent_releases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn forget_sent_releases(&self) {
        self.sent_releases().clear();
    }

    async fn configured_indexer_connection(
        &self,
    ) -> Result<providers::indexer::ConfiguredIndexerConnection> {
        let connection = self.inner.settings.indexer_connection();
        self.vault_blocking(move |runtime| {
            IndexerProvider::configured_connection(connection, runtime.inner.vault.as_ref())
        })
        .await
    }

    pub(crate) async fn indexer_api_key_configured(&self) -> Result<bool> {
        let connection = self.inner.settings.indexer_connection();
        self.vault_blocking(move |runtime| {
            IndexerProvider::api_key_configured(&connection, runtime.inner.vault.as_ref())
        })
        .await
    }

    /// Runs vault reads or writes on a blocking thread and waits for them, so
    /// an OS keyring prompt blocks that thread and not the executor. Callers
    /// must not wrap this in `until_closing`: that would drop a started read.
    async fn vault_blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce(&RemoteSourceRuntime) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let runtime = self.clone();
        let read = self.inner.tasks.admit(|| {
            #[expect(clippy::disallowed_methods, reason = "joined by vault_blocking")]
            let read = tokio::task::spawn_blocking(move || work(&runtime));
            read
        })?;
        read.await
            .map_err(|error| AppError::General(error.to_string()))?
    }

    pub(crate) async fn test_indexer_connection(
        &self,
        update: types::RemoteIndexerConnectionUpdate,
    ) -> Result<types::RemoteIndexerConnectionTestResult> {
        let (base_url, api_key) = self
            .vault_blocking(move |runtime| {
                IndexerProvider::draft_credentials(runtime.inner.vault.as_ref(), update)
            })
            .await?;
        self.inner
            .tasks
            .until_closing(IndexerProvider::test_connection(
                &self.inner.indexer_adapter,
                base_url,
                api_key,
            ))
            .await
    }

    pub(crate) async fn start_acquisition(&self, plan: RemoteAcquisitionPlan) -> Result<()> {
        self.inner
            .lifecycle
            .start_acquisition(self.clone(), plan)
            .await
    }
}

fn read_handoff_url(path: Option<PathBuf>) -> Result<String> {
    let path = match path {
        Some(path) => path,
        None => std::env::var("ABB_AUDIBLE_AUTH_RESPONSE_URL_PATH")
            .map(PathBuf::from)
            .map_err(|_| {
                AppError::InvalidInput(
                    "Provide a handoff path or set ABB_AUDIBLE_AUTH_RESPONSE_URL_PATH.".to_string(),
                )
            })?,
    };
    if let Some(response_url) = direct_response_url_from_input(&path) {
        return Ok(response_url);
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::InvalidInput(
            "Audible auth handoff path must not be a symlink.".to_string(),
        ));
    }
    let content = fs::read_to_string(path)?;
    let response_url = content.trim().to_string();
    if response_url.is_empty() {
        return Err(AppError::InvalidInput(
            "Audible auth handoff file was empty.".to_string(),
        ));
    }
    Ok(response_url)
}

fn direct_response_url_from_input(path: &Path) -> Option<String> {
    let input = path.as_os_str().to_str()?.trim();
    if input.starts_with("https://") || input.starts_with("http://") {
        return Some(input.to_string());
    }
    None
}

pub use types::*;

#[cfg(test)]
#[path = "remote_source_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "test_support_tests.rs"]
pub(crate) mod test_support;
