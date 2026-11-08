use super::super::RemoteIndexerConnectionTestResult;
use super::lane_status;
use super::{
    AppErrorEnvelope, IndexerCategoryOption, IndexerConnection, IndexerWorkSnapshot, ProviderId,
    ReleaseGrabSnapshot, ReleaseGrabStatus, RemoteDraftStatus, RemoteIndexerConnectionUpdate,
    RemoteRelease, RemoteReleaseGrabRequest, RemoteReleaseSearchRequest,
    RemoteReleaseSearchResponse, RemoteSourceRuntime, UiAction, UiState,
};
use crate::app_settings::normalize_base_url;
use crate::errors::{AppError, Result};
use crate::remote_source::providers::indexer::validate_api_key;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ConnectionLoad {
    #[default]
    NotLoaded,
    Loading,
    Loaded,
}

#[derive(Default)]
pub(super) struct IndexerLane {
    pub(super) api_key: Option<String>,
    pub(super) url_refusal: Option<AppErrorEnvelope>,
    pub(super) key_refusal: Option<AppErrorEnvelope>,
    pub(super) edit_revision: u64,
    pub(super) load: ConnectionLoad,
    pub(super) grabbing: bool,
    pub(super) resetting: bool,
    pub(super) test_request: u64,
    pub(super) search_request: u64,
    pub(super) connection_generation: u64,
    pub(super) saved_category_ids: Vec<u32>,
}

const RECOMMENDED_CATEGORY: u32 = 3030;
const KNOWN_CATEGORIES: [(u32, &str); 2] = [(3030, "Audiobooks (3030)"), (3000, "Audio (3000)")];

impl IndexerLane {
    pub(super) fn category_options(&self, draft_ids: &[u32]) -> Vec<IndexerCategoryOption> {
        let mut others: Vec<u32> = self
            .saved_category_ids
            .iter()
            .chain(draft_ids)
            .copied()
            .filter(|id| KNOWN_CATEGORIES.iter().all(|(known, _)| known != id))
            .collect();
        others.sort_unstable();
        others.dedup();
        KNOWN_CATEGORIES
            .iter()
            .map(|(id, label)| (*id, (*label).to_string()))
            .chain(others.into_iter().map(|id| (id, format!("Category {id}"))))
            .map(|(id, label)| IndexerCategoryOption {
                recommended: id == RECOMMENDED_CATEGORY,
                id,
                label,
            })
            .collect()
    }
}

pub(super) fn uses_http(base_url: &str) -> bool {
    base_url
        .trim()
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
}

impl UiState {
    pub(super) fn indexer_saving(&self) -> bool {
        self.snapshot.connection.save == RemoteDraftStatus::Running
    }

    fn indexer_changing(&self) -> bool {
        self.indexer_saving() || self.indexer.resetting
    }

    pub(super) fn indexer_busy(&self) -> bool {
        self.snapshot.indexer.searching || self.indexer.grabbing || self.indexer_changing()
    }

    pub(super) fn refuse_while_indexer_busy(&self, before: Option<&str>) -> Result<()> {
        if self.indexer_busy() {
            return Err(busy_refusal(before));
        }
        Ok(())
    }
}

pub(super) fn busy_refusal(before: Option<&str>) -> AppError {
    AppError::InvalidInput(match before {
        Some(action) => {
            format!("Wait for the current Indexer search, grab, or save to finish before {action}.")
        }
        None => "Wait for the current Indexer search, grab, or save to finish.".to_string(),
    })
}

impl UiState {
    pub(super) fn ensure_draft_usable(&self) -> Result<()> {
        if self.snapshot.connection.draft_error.is_some() {
            return Err(AppError::InvalidInput(
                "Correct the Indexer connection details before saving or testing.".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn clear_draft_refusals(&mut self) {
        self.indexer.url_refusal = None;
        self.indexer.key_refusal = None;
        self.snapshot.connection.draft_error = None;
    }

    pub(super) fn edit_connection(
        &mut self,
        base_url: Option<String>,
        category_ids: Option<Vec<u32>>,
        api_key: Option<String>,
    ) -> Result<()> {
        let mut refusal = None;
        if let Some(url) = base_url {
            match normalize_base_url(url.clone()) {
                Ok(_) => self.indexer.url_refusal = None,
                Err(error) => {
                    self.indexer.url_refusal = Some(AppErrorEnvelope::from(&error));
                    refusal = Some(error);
                }
            }
            self.snapshot.connection.base_url = url;
        }
        if let Some(key) = api_key {
            match validate_api_key(key.trim()) {
                Ok(()) => {
                    self.indexer.key_refusal = None;
                    self.indexer.api_key = (!key.trim().is_empty()).then(|| key.trim().to_string());
                }
                Err(error) => {
                    self.indexer.key_refusal = Some(AppErrorEnvelope::from(&error));
                    refusal = Some(error);
                }
            }
        }
        self.snapshot.connection.draft_error = self
            .indexer
            .key_refusal
            .clone()
            .or_else(|| self.indexer.url_refusal.clone());
        let refused = category_ids
            .as_ref()
            .is_some_and(|ids| ids.iter().all(|id| *id == 0));
        self.snapshot.connection.empty_categories_refused = refused;
        let category_ids = category_ids.filter(|_| !refused);
        self.indexer.edit_revision += 1;
        if let Some(ids) = category_ids {
            self.snapshot.connection.category_ids = ids;
        }
        self.snapshot.connection.api_key_entered = self.indexer.api_key.is_some();
        if !self.indexer_saving() {
            self.snapshot.connection.save = RemoteDraftStatus::Idle;
        }
        self.snapshot.connection.test = RemoteDraftStatus::Idle;
        self.snapshot.connection.test_result = None;
        match refusal {
            Some(error) => {
                self.changed();
                Err(error)
            }
            None => Ok(()),
        }
    }

    pub(super) fn connection_update(&self) -> RemoteIndexerConnectionUpdate {
        RemoteIndexerConnectionUpdate {
            base_url: Some(self.snapshot.connection.base_url.trim().to_string()),
            api_key: self.indexer.api_key.clone(),
        }
    }

    pub(super) fn apply_connection(&mut self, connection: &IndexerConnection) {
        self.snapshot.connection.base_url = connection.base_url.clone().unwrap_or_default();
        self.snapshot.connection.category_ids = connection.category_ids.clone();
        self.indexer.saved_category_ids = connection.category_ids.clone();
        self.indexer.api_key = None;
        self.snapshot.connection.api_key_entered = false;
    }

    pub(super) fn loaded(&mut self, request: u64, result: &Result<bool>) {
        if self.indexer.load == ConnectionLoad::Loading {
            self.indexer.load = ConnectionLoad::NotLoaded;
        }
        if request != self.indexer.connection_generation {
            return;
        }
        match result {
            Ok(configured) => {
                self.indexer.load = ConnectionLoad::Loaded;
                self.snapshot.connection.api_key_configured = *configured;
            }
            Err(error) if !self.indexer_saving() => {
                self.snapshot.connection.save = RemoteDraftStatus::Failed {
                    error: error.into(),
                }
            }
            Err(error) => log::warn!("Indexer connection load failed during a save: {error}"),
        }
        self.changed();
    }

    pub(super) fn saved(
        &mut self,
        revision: u64,
        connection: &IndexerConnection,
        result: &Result<bool>,
    ) {
        match result {
            Ok(configured) => {
                self.indexer.load = ConnectionLoad::Loaded;
                self.indexer.saved_category_ids = connection.category_ids.clone();
                self.snapshot.connection.api_key_configured = *configured;
                if self.indexer.edit_revision == revision {
                    self.apply_connection(connection);
                }
                self.snapshot.connection.save = if self.indexer.edit_revision == revision {
                    RemoteDraftStatus::Succeeded
                } else {
                    RemoteDraftStatus::Idle
                };
            }
            Err(error) => {
                self.snapshot.connection.save = RemoteDraftStatus::Failed {
                    error: error.into(),
                }
            }
        }
        self.changed();
    }

    pub(super) fn tested(
        &mut self,
        request: u64,
        revision: u64,
        result: &Result<RemoteIndexerConnectionTestResult>,
    ) {
        if revision != self.indexer.edit_revision || request != self.indexer.test_request {
            return;
        }
        match result {
            Ok(result) => {
                self.snapshot.connection.test = RemoteDraftStatus::Succeeded;
                self.snapshot.connection.test_result = Some(result.clone());
            }
            Err(error) => {
                self.snapshot.connection.test = RemoteDraftStatus::Failed {
                    error: error.into(),
                }
            }
        }
        self.changed();
    }
}

impl UiState {
    pub(super) fn indexer_available(&self) -> Result<()> {
        if self.snapshot.lane != ProviderId::Indexer {
            return Err(busy_refusal(None));
        }
        self.refuse_while_indexer_busy(None)
    }
    pub(super) fn search(&mut self, author: String, title: String) -> Result<UiAction> {
        self.indexer_available()?;
        let author = author.trim();
        let title = title.trim();
        if author.is_empty() && title.is_empty() {
            return Err(AppError::InvalidInput(
                "Enter an author and/or title to search.".into(),
            ));
        }
        self.snapshot.indexer = IndexerWorkSnapshot {
            searching: true,
            message: "Searching Indexer releases.".into(),
            ..IndexerWorkSnapshot::default()
        };
        self.indexer.search_request += 1;
        Ok(UiAction::Search {
            request: self.indexer.search_request,
            query: RemoteReleaseSearchRequest {
                author: (!author.is_empty()).then(|| author.to_string()),
                title: (!title.is_empty()).then(|| title.to_string()),
                query: None,
            },
        })
    }
    pub(super) fn select_release(&mut self, key: String, multi: bool) -> Result<()> {
        if !self
            .snapshot
            .indexer
            .releases
            .iter()
            .any(|release| release.key == key)
        {
            return Err(AppError::InvalidInput(
                "Search again before selecting this release.".into(),
            ));
        }
        let keys = &mut self.snapshot.indexer.selected_release_keys;
        if !multi {
            keys.clear();
        }
        if multi && keys.contains(&key) {
            keys.retain(|candidate| candidate != &key);
        } else {
            keys.push(key);
        }
        Ok(())
    }
    pub(super) fn grab(&mut self, keys: Vec<String>) -> Result<UiAction> {
        self.indexer_available()?;
        let releases: Vec<_> = self
            .snapshot
            .indexer
            .releases
            .iter()
            .filter(|release| {
                keys.contains(&release.key)
                    && !self
                        .snapshot
                        .indexer
                        .release_grabs
                        .get(&release.key)
                        .is_some_and(|grab| grab.status == ReleaseGrabStatus::Sent)
            })
            .cloned()
            .collect();
        if releases.is_empty() {
            return Err(AppError::InvalidInput(
                "Select unsent releases from the current search.".into(),
            ));
        }
        for release in &releases {
            self.snapshot.indexer.release_grabs.insert(
                release.key.clone(),
                ReleaseGrabSnapshot {
                    status: ReleaseGrabStatus::Queued,
                    message: "Waiting to send.".into(),
                },
            );
        }
        self.indexer.grabbing = true;
        self.snapshot.indexer.message = "Sending to downloader via Indexer…".into();
        Ok(UiAction::Grab(releases))
    }
    pub(super) fn searched(&mut self, request: u64, result: &Result<RemoteReleaseSearchResponse>) {
        self.snapshot.indexer.searching = false;
        if request != self.indexer.search_request {
            self.changed();
            return;
        }
        match result {
            Ok(response) => {
                self.snapshot.indexer.releases = response.releases.clone();
                self.snapshot.indexer.message = if response.diagnostics.is_empty() {
                    format!("{} releases found.", response.releases.len())
                } else {
                    response
                        .diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                };
            }
            Err(error) => self.snapshot.indexer.message = error.to_string(),
        }
        self.changed();
    }
}

impl RemoteSourceRuntime {
    pub(super) async fn grab_ui_batch(&self, releases: Vec<RemoteRelease>) {
        let turn = self.indexer_turn();
        let mut unconfirmed = 0;
        for release in &releases {
            self.ui().snapshot.indexer.release_grabs.insert(
                release.key.clone(),
                ReleaseGrabSnapshot {
                    status: ReleaseGrabStatus::Sending,
                    message: "Sending to downloader via Indexer…".into(),
                },
            );
            self.ui().changed();
            self.publish_ui();
            let result = match &turn {
                Ok(_) => {
                    self.grab_release(RemoteReleaseGrabRequest {
                        release: release.clone(),
                    })
                    .await
                }
                Err(error) => Err(AppError::InvalidInput(error.to_string())),
            };
            if result.as_ref().is_ok_and(|response| response.unconfirmed) {
                unconfirmed += 1;
            }
            let outcome = match result {
                Ok(response) => ReleaseGrabSnapshot {
                    status: if response.accepted {
                        ReleaseGrabStatus::Sent
                    } else {
                        ReleaseGrabStatus::Error
                    },
                    message: if response.diagnostics.is_empty() {
                        response.message
                    } else {
                        response
                            .diagnostics
                            .iter()
                            .map(|diagnostic| diagnostic.message.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    },
                },
                Err(error) => ReleaseGrabSnapshot {
                    status: ReleaseGrabStatus::Error,
                    message: error.to_string(),
                },
            };
            self.ui()
                .snapshot
                .indexer
                .release_grabs
                .insert(release.key.clone(), outcome);
            self.ui().changed();
            self.publish_ui();
        }
        let mut state = self.ui();
        let outcomes: Vec<_> = releases
            .iter()
            .filter_map(|release| state.snapshot.indexer.release_grabs.get(&release.key))
            .collect();
        let sent = outcomes
            .iter()
            .filter(|outcome| outcome.status == ReleaseGrabStatus::Sent)
            .count();
        state.snapshot.indexer.message = if outcomes.len() == 1 {
            outcomes[0].message.clone()
        } else {
            lane_status::grab_batch_line(sent, outcomes.len() - sent - unconfirmed, unconfirmed)
        };
        state.indexer.grabbing = false;
        state.changed();
    }
}
