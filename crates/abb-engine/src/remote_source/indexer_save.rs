use super::{
    IndexerWorkSnapshot, RemoteDraftStatus, RemoteUiIntent, RemoteUiResult, UiAction, UiState,
};
use crate::app_settings::{IndexerConnection, SettingsOutcome, SettingsRun};
use crate::errors::{AppError, Result};
use crate::remote_source::{IndexerProvider, ProviderId, RemoteSourceRuntime};

/// An accepted Save: the validated connection, the key typed with it, and
/// what it holds until it finishes (`T`).
pub(super) struct IndexerSave<T = SaveTurn> {
    pub(super) revision: u64,
    pub(super) request: u64,
    pub(super) connection: IndexerConnection,
    pub(super) api_key: Option<String>,
    pub(super) turn: T,
}

/// The Indexer write guard and the reserved settings turn, taken together
/// when a Save is accepted.
pub(super) struct SaveTurn {
    previous: IndexerConnection,
    settings: SettingsRun,
    write: tokio::sync::OwnedRwLockWriteGuard<()>,
}

impl UiState {
    pub(in crate::remote_source) fn seeded(saved: &IndexerConnection) -> Self {
        let mut state = Self::default();
        state.apply_connection(saved);
        state
    }

    pub(super) fn save_connection<T>(
        &mut self,
        reserve: impl FnOnce(&IndexerConnection) -> T,
    ) -> Result<IndexerSave<T>> {
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
        Ok(IndexerSave {
            revision: self.edit_revision,
            request: self.save_request,
            turn: reserve(&connection),
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
    pub(super) fn begin_ui(&self, intent: RemoteUiIntent) -> Result<UiAction> {
        if matches!(intent, RemoteUiIntent::SaveConnection) {
            return self.begin_save();
        }
        let mut state = self.ui();
        if matches!(
            intent,
            RemoteUiIntent::Disconnect {
                provider: ProviderId::Audible
            }
        ) && self.inner.lifecycle.has_unsettled_acquisition()
        {
            Err(AppError::InvalidInput(
                "Wait for the Audible acquisition and handoff before disconnecting.".into(),
            ))
        } else {
            state.begin(intent)
        }
    }

    fn begin_save(&self) -> Result<UiAction> {
        let write = self
            .inner
            .indexer_turn
            .clone()
            .try_write_owned()
            .map_err(|_| {
                AppError::InvalidInput(
                    "Wait for the current search or grab to finish before saving the Indexer connection."
                        .to_string(),
                )
            })?;
        let mut state = self.ui();
        state.accept()?;
        state
            .save_connection(|connection| SaveTurn {
                previous: self.inner.settings.indexer_connection(),
                settings: self
                    .inner
                    .settings
                    .set_indexer_connection(connection.clone(), &self.inner.tasks),
                write,
            })
            .map(UiAction::Save)
    }

    pub(super) async fn finish_indexer_save(&self, save: IndexerSave) -> Result<RemoteUiResult> {
        let IndexerSave {
            revision,
            request,
            connection,
            api_key,
            turn:
                SaveTurn {
                    previous,
                    settings,
                    write,
                },
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
        let superseded = self.ui().save_request != request;
        let key_fact = if superseded || (applied && result.is_err() && connection != previous) {
            Some(self.indexer_api_key_configured().await.unwrap_or(false))
        } else {
            None
        };
        let refresh = {
            let saved = self.inner.settings.indexer_connection();
            let mut state = self.ui();
            if state.save_request == request {
                state.saved(revision, &connection, &result);
                if let Some(fact) = key_fact {
                    state.snapshot.connection.api_key_configured = fact;
                }
            } else {
                state.connection_saving = false;
                state.snapshot.connection.save = RemoteDraftStatus::Idle;
                state.apply_connection(&saved);
                state.snapshot.connection.api_key_configured = key_fact.unwrap_or(false);
                state.changed();
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
            let connection = &mut state.snapshot.connection;
            connection.draft_error = None;
            connection.empty_categories_refused = false;
            connection.save = RemoteDraftStatus::Idle;
            connection.test = RemoteDraftStatus::Idle;
            connection.test_result = None;
            state.search_request += 1;
            state.snapshot.indexer = IndexerWorkSnapshot {
                searching: state.snapshot.indexer.searching,
                grabbing: state.snapshot.indexer.grabbing,
                ..IndexerWorkSnapshot::default()
            };
            state.changed();
            (state.snapshot.lane == ProviderId::Indexer).then(|| {
                state.account_request += 1;
                state.snapshot.account_status = RemoteDraftStatus::Running;
                state.account_request
            })
        };
        self.publish_ui();
        if let Some(request) = refresh {
            let _ = self.refresh_ui_account(ProviderId::Indexer, request).await;
        }
    }
}

#[cfg(test)]
#[path = "indexer_save_tests.rs"]
mod tests;
