use super::{IndexerWorkSnapshot, RemoteDraftStatus, RemoteUiResult, UiAction, UiState};
use std::sync::{Arc, Mutex, PoisonError};

use crate::app_settings::{IndexerConnection, SettingsOutcome, SettingsRun};
use crate::errors::{AppError, Result};
use crate::remote_source::{IndexerProvider, ProviderId, RemoteSourceRuntime};

/// The validated connection and the key typed with it, as a Save was
/// accepted against the draft.
pub(super) struct AcceptedSave {
    pub(super) revision: u64,
    pub(super) connection: IndexerConnection,
    pub(super) api_key: Option<String>,
}

/// An accepted Save and what it holds until it finishes: the previous
/// connection, the reserved settings turn, and the Indexer write guard.
pub(super) struct IndexerSave {
    pub(super) accepted: AcceptedSave,
    previous: IndexerConnection,
    settings: SettingsRun,
    write: tokio::sync::OwnedRwLockWriteGuard<()>,
}

/// An admitted Reset: the Indexer write lease, and the fact on the UI state
/// that refuses a new Search or Grab until it is dropped.
pub(crate) struct ResetReservation {
    ui: Arc<Mutex<UiState>>,
    _lease: tokio::sync::OwnedRwLockWriteGuard<()>,
}

impl Drop for ResetReservation {
    fn drop(&mut self) {
        self.ui
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .connection_resetting = false;
    }
}

impl UiState {
    pub(in crate::remote_source) fn seeded(saved: &IndexerConnection) -> Self {
        let mut state = Self::default();
        state.apply_connection(saved);
        state
    }

    pub(super) fn save_connection(&mut self) -> Result<AcceptedSave> {
        self.ensure_draft_usable()?;
        if self.connection_saving
            || self.snapshot.indexer.searching
            || self.snapshot.indexer.grabbing
        {
            return Err(AppError::InvalidInput(
                "Wait for the current Indexer save, search, or grab to finish.".into(),
            ));
        }
        let mut connection = IndexerConnection {
            base_url: Some(self.snapshot.connection.base_url.clone()),
            category_ids: self.snapshot.connection.category_ids.clone(),
        };
        connection.validate()?;
        if self.api_key.is_some() && connection.base_url.is_none() {
            return Err(AppError::InvalidInput(
                "Configure Indexer URL before saving an API key.".into(),
            ));
        }
        self.connection_saving = true;
        self.save_request += 1;
        self.edit_revision += 1;
        self.snapshot.indexer = IndexerWorkSnapshot::default();
        self.test_request += 1;
        self.snapshot.connection.test = RemoteDraftStatus::Idle;
        self.snapshot.connection.test_result = None;
        self.snapshot.connection.save = RemoteDraftStatus::Running;
        Ok(AcceptedSave {
            revision: self.edit_revision,
            connection,
            api_key: self.api_key.clone(),
        })
    }

    fn shows_saved(&mut self, saved: &IndexerConnection, api_key_configured: bool) {
        self.apply_connection(saved);
        self.snapshot.connection.api_key_configured = api_key_configured;
        self.save_request += 1;
        self.test_request += 1;
    }
}

impl RemoteSourceRuntime {
    pub(super) fn begin_save(&self) -> Result<UiAction> {
        let write = self.indexer_write_lease("saving the Indexer connection")?;
        let mut state = self.ui();
        state.accept()?;
        let accepted = state.save_connection()?;
        Ok(UiAction::Save(IndexerSave {
            previous: self.inner.settings.indexer_connection(),
            settings: self
                .inner
                .settings
                .set_indexer_connection(accepted.connection.clone(), &self.inner.tasks),
            accepted,
            write,
        }))
    }

    /// The Indexer connection changes only while no search, grab, or Save
    /// holds the lease, so a batch never sees a connection change between two
    /// releases. A Save and a Reset take it through here.
    fn indexer_write_lease(&self, action: &str) -> Result<tokio::sync::OwnedRwLockWriteGuard<()>> {
        self.inner
            .indexer_turn
            .clone()
            .try_write_owned()
            .map_err(|_| {
                AppError::InvalidInput(format!(
                    "Wait for the current Indexer search, grab, or save to finish before {action}."
                ))
            })
    }

    /// Reset replaces the saved connection, so it is refused while the Indexer
    /// works or saves, as a Save is, and once admitted it refuses a new Search
    /// or Grab. Hold the returned reservation until
    /// [`Self::saved_connection_replaced`] has run; dropping it, however the
    /// Reset ends, admits Search and Grab again.
    pub(crate) fn reset_lease(&self) -> Result<ResetReservation> {
        let lease = self.indexer_write_lease("resetting settings")?;
        let mut state = self.ui();
        if state.connection_saving
            || state.snapshot.indexer.searching
            || state.snapshot.indexer.grabbing
        {
            return Err(AppError::InvalidInput(
                "Wait for the current Indexer search, grab, or save to finish before resetting settings."
                    .into(),
            ));
        }
        state.connection_resetting = true;
        drop(state);
        Ok(ResetReservation {
            ui: Arc::clone(&self.inner.ui),
            _lease: lease,
        })
    }

    pub(super) async fn finish_indexer_save(&self, save: IndexerSave) -> Result<RemoteUiResult> {
        let IndexerSave {
            accepted:
                AcceptedSave {
                    revision,
                    connection,
                    api_key,
                },
            previous,
            settings,
            write,
        } = save;
        self.forget_sent_releases();
        let reply = settings.finish().await;
        self.inner
            .ui_host
            .emit(crate::EngineEvent::Settings(Box::new(reply.snapshot)));
        let applied = matches!(reply.outcome, SettingsOutcome::Applied);
        let result = match reply.outcome {
            SettingsOutcome::Applied => {
                self.change_indexer_key(&connection, api_key, &previous)
                    .await
            }
            SettingsOutcome::Rejected { error } => Err(AppError::General(error.message)),
        };
        let key_fact = if applied && result.is_err() && connection != previous {
            Some(self.indexer_api_key_configured().await.unwrap_or(false))
        } else {
            None
        };
        let refresh = {
            let mut state = self.ui();
            state.saved(revision, &connection, &result);
            if let Some(fact) = key_fact {
                state.snapshot.connection.api_key_configured = fact;
            }
            if state.snapshot.lane == ProviderId::Indexer {
                state.account_request += 1;
                state.snapshot.account_status = RemoteDraftStatus::Running;
                Some(state.account_request)
            } else {
                None
            }
        };
        drop(write);
        if let Some(request) = refresh {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the refresh records its own failure in the account status"
            )]
            let _ = self.refresh_ui_account(ProviderId::Indexer, request).await;
        }
        result.map(|_| RemoteUiResult::Saved)
    }

    async fn change_indexer_key(
        &self,
        connection: &IndexerConnection,
        api_key: Option<String>,
        previous: &IndexerConnection,
    ) -> Result<bool> {
        let Some(base_url) = connection.base_url.clone() else {
            return Ok(false);
        };
        self.vault_blocking(move |runtime| {
            IndexerProvider::apply_key_change(runtime.inner.vault.as_ref(), &base_url, api_key)
        })
        .await
        .map_err(|error| {
            if connection == previous {
                error
            } else {
                AppError::General(format!(
                    "The Indexer URL and categories changed, but the API key could not be updated: {error}"
                ))
            }
        })
    }

    /// An applied Reset put the default connection in effect; the draft, the
    /// entered key, the key fact, results, and old failures follow it, and
    /// the account is read again.
    pub(crate) async fn saved_connection_replaced(&self) {
        let refresh = {
            let saved = self.inner.settings.indexer_connection();
            let mut state = self.ui();
            state.shows_saved(&saved, false);
            state.clear_draft_refusals();
            let connection = &mut state.snapshot.connection;
            connection.empty_categories_refused = false;
            connection.save = RemoteDraftStatus::Idle;
            connection.test = RemoteDraftStatus::Idle;
            connection.test_result = None;
            state.snapshot.indexer = IndexerWorkSnapshot::default();
            state.changed();
            (state.snapshot.lane == ProviderId::Indexer).then(|| {
                state.account_request += 1;
                state.snapshot.account_status = RemoteDraftStatus::Running;
                state.account_request
            })
        };
        self.publish_ui();
        if let Some(request) = refresh {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the refresh records its own failure in the account status"
            )]
            let _ = self.refresh_ui_account(ProviderId::Indexer, request).await;
        }
    }
}

#[cfg(test)]
#[path = "indexer_save_tests.rs"]
mod tests;
