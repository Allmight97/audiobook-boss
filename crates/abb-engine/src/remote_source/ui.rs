//! Working remote choices and connection drafts; hosts send intents and read facts.

use super::{
    AcquisitionPlan, AcquisitionSelection, AcquisitionSnapshot, ProviderId,
    RemoteIndexerConnection, RemoteIndexerConnectionTestResult, RemoteIndexerConnectionUpdate,
    RemoteRelease, RemoteReleaseGrabRequest, RemoteReleaseSearchRequest,
    RemoteReleaseSearchResponse, RemoteSourceRuntime, RemoteTitle,
};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::host::EngineEvent;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemoteUiIntent {
    StartAuth,
    #[serde(rename_all = "camelCase")]
    CompleteAuth {
        response_url_handoff_path: Option<String>,
    },
    /// Disconnects `provider`; refused unless it is the lane on screen.
    Disconnect {
        provider: ProviderId,
    },
    RefreshLibrary,
    SelectLane {
        lane: ProviderId,
    },
    #[serde(rename_all = "camelCase")]
    ToggleTitle {
        title_id: String,
    },
    ClearTitles,
    #[serde(rename_all = "camelCase")]
    TogglePdf {
        title_id: String,
    },
    AcquireSelected,
    #[serde(rename_all = "camelCase")]
    CancelAcquisition {
        job_id: String,
    },
    #[serde(rename_all = "camelCase")]
    SearchReleases {
        author: String,
        title: String,
    },
    #[serde(rename_all = "camelCase")]
    SelectRelease {
        indexer_id: i64,
        guid: String,
        multi: bool,
    },
    GrabSelected,
    #[serde(rename_all = "camelCase")]
    GrabRelease {
        indexer_id: i64,
        guid: String,
    },
    LoadConnection,
    #[serde(rename_all = "camelCase")]
    EditConnection {
        base_url: Option<String>,
        category_ids: Option<Vec<u32>>,
        api_key: Option<String>,
    },
    SaveConnection,
    TestConnection,
}

// An intent can contain an entered key or a URL not yet validated.
impl std::fmt::Debug for RemoteUiIntent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RemoteUiIntent (payload redacted)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemoteDraftStatus {
    Idle,
    Running,
    Succeeded,
    Failed { error: AppErrorEnvelope },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexerDraftSnapshot {
    pub base_url: String,
    pub category_ids: Vec<u32>,
    pub api_key_configured: bool,
    pub api_key_entered: bool,
    pub save: RemoteDraftStatus,
    pub test: RemoteDraftStatus,
    pub test_result: Option<RemoteIndexerConnectionTestResult>,
    /// Why the last edit was refused. The draft keeps the values it accepted
    /// before; Save and Test refuse until an edit is accepted.
    pub draft_error: Option<AppErrorEnvelope>,
    /// The last category edit chose none and was refused; the draft keeps
    /// its categories. A search needs at least one.
    pub empty_categories_refused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseGrabSnapshot {
    pub status: ReleaseGrabStatus,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseGrabStatus {
    Queued,
    Sending,
    Sent,
    Error,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexerWorkSnapshot {
    pub releases: Vec<RemoteRelease>,
    pub selected_release_keys: Vec<String>,
    pub release_grabs: BTreeMap<String, ReleaseGrabSnapshot>,
    pub searching: bool,
    pub grabbing: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemoteAuthStatus {
    Idle,
    Starting,
    AwaitingHandoff,
    Completing,
    Failed { error: AppErrorEnvelope },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLibrarySnapshot {
    #[specta(type = specta_typescript::Number)]
    pub revision: u64,
    pub titles: Vec<RemoteTitle>,
    pub diagnostics: Vec<super::RemoteSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteUiSnapshot {
    #[specta(type = specta_typescript::Number)]
    pub revision: u64,
    pub lane: ProviderId,
    pub providers: Vec<super::RemoteSourceProviderCapabilities>,
    pub account: Option<super::RemoteSourceAccountState>,
    pub account_status: RemoteDraftStatus,
    pub auth: RemoteAuthStatus,
    pub library_status: RemoteDraftStatus,
    pub selected_title_ids: Vec<String>,
    pub include_pdf_by_title_id: BTreeMap<String, bool>,
    pub indexer: IndexerWorkSnapshot,
    pub connection: IndexerDraftSnapshot,
    pub acquisition: Option<Box<AcquisitionSnapshot>>,
    pub acquiring: bool,
}

pub(super) struct UiState {
    snapshot: RemoteUiSnapshot,
    library: RemoteLibrarySnapshot,
    library_published: u64,
    account_request: u64,
    disconnecting: bool,
    api_key: Option<String>,
    edit_revision: u64,
    connection_loading: bool,
    connection_loaded: bool,
    connection_saving: bool,
    /// Fields edited since the saved connection began loading; the load
    /// fills in only the others.
    url_edited: bool,
    categories_edited: bool,
    test_request: u64,
    search_request: u64,
    save_request: u64,
    library_request: u64,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            snapshot: RemoteUiSnapshot {
                revision: 0,
                lane: ProviderId::Audible,
                providers: vec![
                    super::AudibleProvider::capabilities(),
                    super::IndexerProvider::capabilities(),
                ],
                account: None,
                account_status: RemoteDraftStatus::Idle,
                auth: RemoteAuthStatus::Idle,
                library_status: RemoteDraftStatus::Idle,
                selected_title_ids: Vec::new(),
                include_pdf_by_title_id: BTreeMap::new(),
                acquisition: None,
                acquiring: false,
                indexer: IndexerWorkSnapshot::default(),
                connection: IndexerDraftSnapshot {
                    base_url: String::new(),
                    category_ids: super::providers::indexer::default_category_ids(),
                    api_key_configured: false,
                    api_key_entered: false,
                    draft_error: None,
                    empty_categories_refused: false,
                    save: RemoteDraftStatus::Idle,
                    test: RemoteDraftStatus::Idle,
                    test_result: None,
                },
            },
            library: RemoteLibrarySnapshot::default(),
            library_published: 0,
            account_request: 0,
            disconnecting: false,
            api_key: None,
            edit_revision: 0,
            connection_loading: false,
            connection_loaded: false,
            url_edited: false,
            categories_edited: false,
            connection_saving: false,
            test_request: 0,
            search_request: 0,
            save_request: 0,
            library_request: 0,
        }
    }
}

impl UiState {
    fn account_change_allowed(&self) -> Result<()> {
        if self.snapshot.lane != ProviderId::Audible {
            return Err(AppError::InvalidInput(
                "Indexer uses connection settings instead of browser auth.".into(),
            ));
        }
        self.disconnect_allowed(ProviderId::Audible)
    }

    fn account_reply(
        &mut self,
        lane: ProviderId,
        request: u64,
        result: &Result<super::RemoteSourceAccountState>,
    ) -> bool {
        if lane != self.snapshot.lane || request != self.account_request || self.disconnecting {
            return false;
        }
        match result {
            Ok(account) => {
                self.snapshot.account = Some(account.clone());
                self.snapshot.account_status = RemoteDraftStatus::Succeeded;
            }
            Err(error) => {
                self.snapshot.account_status = RemoteDraftStatus::Failed {
                    error: error.into(),
                }
            }
        }
        self.changed();
        true
    }

    fn library_result(
        &mut self,
        request: u64,
        result: &Result<super::RemoteLibraryResponse>,
    ) -> bool {
        if request != self.library_request
            || self.snapshot.lane != ProviderId::Audible
            || self.disconnecting
        {
            return false;
        }
        match result {
            Ok(library) => {
                self.library_loaded_with_diagnostics(
                    library.titles.clone(),
                    library.diagnostics.clone(),
                );
                self.snapshot.library_status = RemoteDraftStatus::Succeeded;
            }
            Err(error) => {
                self.snapshot.library_status = RemoteDraftStatus::Failed {
                    error: error.into(),
                };
                self.changed();
            }
        }
        true
    }

    pub(super) fn begin_library(&mut self) -> u64 {
        self.library_request += 1;
        self.snapshot.library_status = RemoteDraftStatus::Running;
        self.changed();
        self.library_request
    }

    pub(super) fn disconnect_allowed(&self, provider: ProviderId) -> Result<()> {
        if self.disconnecting
            || matches!(
                self.snapshot.auth,
                RemoteAuthStatus::Starting | RemoteAuthStatus::Completing
            )
        {
            return Err(AppError::InvalidInput(
                "Wait for the current account change to finish.".into(),
            ));
        }
        if provider == ProviderId::Audible
            && (self.snapshot.acquiring
                || self
                    .snapshot
                    .acquisition
                    .as_ref()
                    .is_some_and(|job| !job.settled))
        {
            return Err(AppError::InvalidInput(
                "Wait for the Audible acquisition and handoff before disconnecting.".into(),
            ));
        }
        if provider == ProviderId::Indexer
            && (self.connection_saving
                || self.snapshot.indexer.searching
                || self.snapshot.indexer.grabbing)
        {
            return Err(AppError::InvalidInput(
                "Wait for the current Indexer work before disconnecting.".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn account_disconnected(
        &mut self,
        account: Option<super::RemoteSourceAccountState>,
    ) {
        self.snapshot.account = account;
        self.changed();
    }

    pub(super) fn disconnected(&mut self, provider: ProviderId) {
        self.library_request += 1;
        self.account_request += 1;
        self.snapshot.library_status = RemoteDraftStatus::Idle;
        if provider == ProviderId::Audible {
            self.snapshot.auth = RemoteAuthStatus::Idle;
            self.snapshot.acquisition = None;
            self.library_loaded(Vec::new());
        }
        if provider == ProviderId::Indexer {
            self.save_request += 1;
            self.test_request += 1;
            self.connection_loaded = false;
            self.api_key = None;
            self.snapshot.connection.api_key_entered = false;
            self.snapshot.connection.api_key_configured = false;
            self.snapshot.connection.save = RemoteDraftStatus::Idle;
            self.snapshot.connection.test = RemoteDraftStatus::Idle;
            self.snapshot.connection.test_result = None;
            self.snapshot.indexer = IndexerWorkSnapshot::default();
        }
    }

    pub(super) fn snapshot(&self) -> RemoteUiSnapshot {
        self.snapshot.clone()
    }
    fn changed(&mut self) {
        self.snapshot.revision += 1;
    }

    pub(super) fn library_loaded(&mut self, titles: Vec<RemoteTitle>) {
        self.library_loaded_with_diagnostics(titles, Vec::new());
    }

    fn library_loaded_with_diagnostics(
        &mut self,
        titles: Vec<RemoteTitle>,
        diagnostics: Vec<super::RemoteSourceDiagnostic>,
    ) {
        let acquirable: BTreeSet<_> = titles
            .iter()
            .filter(|title| title.availability.acquirable)
            .map(|title| title.title_id.as_str())
            .collect();
        self.snapshot
            .selected_title_ids
            .retain(|id| acquirable.contains(id.as_str()));
        self.snapshot.include_pdf_by_title_id = titles
            .iter()
            .map(|title| {
                let included = title.supplemental_pdf_available
                    && self
                        .snapshot
                        .include_pdf_by_title_id
                        .get(&title.title_id)
                        .copied()
                        .unwrap_or(true);
                (title.title_id.clone(), included)
            })
            .collect();
        if self.library.titles != titles || self.library.diagnostics != diagnostics {
            self.library.titles = titles;
            self.library.diagnostics = diagnostics;
            self.library.revision += 1;
        }
        self.changed();
    }

    pub(super) fn acquisition_changed(&mut self, job: super::AcquisitionJob) {
        self.snapshot.acquisition = Some(Box::new(job.into()));
        self.changed();
    }

    fn begin(&mut self, intent: RemoteUiIntent) -> Result<UiAction> {
        if self.disconnecting {
            return Err(AppError::InvalidInput(
                "Wait for disconnect to finish.".into(),
            ));
        }
        self.changed();
        match intent {
            RemoteUiIntent::StartAuth => {
                self.account_change_allowed()?;
                self.account_request += 1;
                self.library_request += 1;
                self.snapshot.account_status = RemoteDraftStatus::Idle;
                self.snapshot.library_status = RemoteDraftStatus::Idle;
                self.snapshot.auth = RemoteAuthStatus::Starting;
                return Ok(UiAction::StartAuth);
            }
            RemoteUiIntent::CompleteAuth {
                response_url_handoff_path,
            } => return self.begin_auth_completion(response_url_handoff_path),

            RemoteUiIntent::Disconnect { provider } => return self.begin_disconnect(provider),

            RemoteUiIntent::RefreshLibrary => return self.refresh_library(),

            RemoteUiIntent::SelectLane { lane } => return self.select_lane(lane),

            RemoteUiIntent::SearchReleases { author, title } => return self.search(author, title),
            RemoteUiIntent::SelectRelease {
                indexer_id,
                guid,
                multi,
            } => {
                self.select_release(release_key(indexer_id, &guid), multi)?;
            }
            RemoteUiIntent::GrabSelected => {
                return self.grab(self.snapshot.indexer.selected_release_keys.clone());
            }
            RemoteUiIntent::GrabRelease { indexer_id, guid } => {
                return self.grab(vec![release_key(indexer_id, &guid)]);
            }
            RemoteUiIntent::ClearTitles => self.snapshot.selected_title_ids.clear(),
            RemoteUiIntent::ToggleTitle { title_id } => self.toggle_title(title_id)?,
            RemoteUiIntent::TogglePdf { title_id } => self.toggle_pdf(title_id)?,
            RemoteUiIntent::CancelAcquisition { job_id } => {
                return Ok(UiAction::CancelAcquisition(job_id))
            }
            RemoteUiIntent::AcquireSelected => return self.acquire_selected(),
            RemoteUiIntent::LoadConnection => {
                if self.connection_loaded || self.connection_loading {
                    return Ok(UiAction::Done);
                }
                self.connection_loading = true;
                (self.url_edited, self.categories_edited) = (false, false);
                return Ok(UiAction::Load {
                    revision: self.edit_revision,
                    request: self.save_request,
                });
            }
            RemoteUiIntent::EditConnection {
                base_url,
                category_ids,
                api_key,
            } => {
                self.edit_connection(base_url, category_ids, api_key)?;
            }
            RemoteUiIntent::SaveConnection => return self.save_connection(),
            RemoteUiIntent::TestConnection => {
                self.ensure_draft_usable()?;
                self.test_request += 1;
                self.snapshot.connection.test = RemoteDraftStatus::Running;
                self.snapshot.connection.test_result = None;
                return Ok(UiAction::Test {
                    request: self.test_request,
                    revision: self.edit_revision,
                    update: self.connection_update(),
                });
            }
        }
        Ok(UiAction::Done)
    }

    fn begin_auth_completion(
        &mut self,
        response_url_handoff_path: Option<String>,
    ) -> Result<UiAction> {
        self.account_change_allowed()?;
        if self.snapshot.auth != RemoteAuthStatus::AwaitingHandoff {
            return Err(AppError::InvalidInput(
                "Start Audible auth before completing the handoff.".into(),
            ));
        }
        self.account_request += 1;
        self.library_request += 1;
        self.snapshot.account_status = RemoteDraftStatus::Idle;
        self.snapshot.library_status = RemoteDraftStatus::Idle;
        self.snapshot.auth = RemoteAuthStatus::Completing;
        Ok(UiAction::CompleteAuth(response_url_handoff_path))
    }

    fn select_lane(&mut self, lane: ProviderId) -> Result<UiAction> {
        if self.disconnecting {
            return Err(AppError::InvalidInput(
                "Wait for disconnect to finish.".into(),
            ));
        }
        if matches!(
            self.snapshot.auth,
            RemoteAuthStatus::Starting | RemoteAuthStatus::Completing
        ) {
            return Err(AppError::InvalidInput(
                "Wait for Audible sign-in to finish.".into(),
            ));
        }
        if self.snapshot.indexer.grabbing {
            return Err(AppError::InvalidInput(
                "Wait for the current Indexer grab to finish.".into(),
            ));
        }
        if self.snapshot.lane == lane
            && (self.snapshot.account_status == RemoteDraftStatus::Running
                || self.snapshot.library_status == RemoteDraftStatus::Running)
        {
            return Ok(UiAction::Done);
        }
        if self.snapshot.lane != lane {
            self.snapshot.account = None;
            self.snapshot.selected_title_ids.clear();
            self.search_request += 1;
            self.snapshot.indexer = IndexerWorkSnapshot {
                searching: self.snapshot.indexer.searching,
                ..IndexerWorkSnapshot::default()
            };
        }
        self.library_request += 1;
        self.snapshot.library_status = RemoteDraftStatus::Idle;
        self.snapshot.lane = lane;
        self.snapshot.account_status = RemoteDraftStatus::Running;
        self.account_request += 1;
        Ok(UiAction::Account {
            lane,
            request: self.account_request,
        })
    }

    fn refresh_library(&mut self) -> Result<UiAction> {
        if self.snapshot.lane != ProviderId::Audible
            || self.snapshot.acquiring
            || self
                .snapshot
                .acquisition
                .as_ref()
                .is_some_and(|job| !job.settled)
            || self.disconnecting
            || matches!(
                self.snapshot.auth,
                RemoteAuthStatus::Starting | RemoteAuthStatus::Completing
            )
            || self.snapshot.library_status == RemoteDraftStatus::Running
        {
            return Err(AppError::InvalidInput(
                "Wait for the current Audible work before refreshing.".into(),
            ));
        }
        let request = self.begin_library();
        Ok(UiAction::Library(request))
    }

    fn begin_disconnect(&mut self, provider: ProviderId) -> Result<UiAction> {
        if provider != self.snapshot.lane {
            return Err(AppError::InvalidInput(
                "That account is no longer the one shown. Try again.".into(),
            ));
        }
        self.disconnect_allowed(provider)?;
        self.disconnecting = true;
        self.account_request += 1;
        self.library_request += 1;
        self.snapshot.account_status = RemoteDraftStatus::Running;
        self.snapshot.library_status = RemoteDraftStatus::Idle;
        Ok(UiAction::Disconnect(provider))
    }

    fn toggle_title(&mut self, title_id: String) -> Result<()> {
        if !self
            .library
            .titles
            .iter()
            .any(|title| title.title_id == title_id && title.availability.acquirable)
        {
            return Err(AppError::InvalidInput(
                "This title is not available for acquisition.".into(),
            ));
        }
        if self.snapshot.selected_title_ids.contains(&title_id) {
            self.snapshot
                .selected_title_ids
                .retain(|id| id != &title_id);
        } else {
            self.snapshot.selected_title_ids.push(title_id);
        }

        Ok(())
    }
    fn toggle_pdf(&mut self, title_id: String) -> Result<()> {
        if !self
            .library
            .titles
            .iter()
            .any(|title| title.title_id == title_id && title.supplemental_pdf_available)
        {
            return Err(AppError::InvalidInput(
                "This title has no supplemental PDF.".into(),
            ));
        }
        let included = self
            .snapshot
            .include_pdf_by_title_id
            .entry(title_id)
            .or_default();
        *included = !*included;

        Ok(())
    }
    fn acquire_selected(&mut self) -> Result<UiAction> {
        if self.disconnecting
            || matches!(
                self.snapshot.auth,
                RemoteAuthStatus::Starting | RemoteAuthStatus::Completing
            )
            || self.snapshot.acquiring
            || self
                .snapshot
                .acquisition
                .as_ref()
                .is_some_and(|job| !job.settled)
            || self.snapshot.lane != ProviderId::Audible
            || self.snapshot.selected_title_ids.is_empty()
        {
            return Err(AppError::InvalidInput(
                "Select Audible titles before acquiring.".into(),
            ));
        }
        self.snapshot.acquiring = true;
        Ok(UiAction::Acquire(AcquisitionPlan {
            provider_id: ProviderId::Audible,
            selections: self
                .snapshot
                .selected_title_ids
                .iter()
                .map(|id| AcquisitionSelection {
                    title_id: id.clone(),
                    include_supplemental_pdf: self
                        .snapshot
                        .include_pdf_by_title_id
                        .get(id)
                        .copied()
                        .unwrap_or(false),
                })
                .collect(),
        }))
    }
    /// Save and Test act on the draft only once the fields show what it holds.
    fn ensure_draft_usable(&self) -> Result<()> {
        if self.connection_loading {
            return Err(AppError::InvalidInput(
                "Wait for the saved Indexer connection to load.".into(),
            ));
        }
        if self.snapshot.connection.draft_error.is_some() {
            return Err(AppError::InvalidInput(
                "Correct the Indexer connection details before saving or testing.".into(),
            ));
        }
        Ok(())
    }

    fn save_connection(&mut self) -> Result<UiAction> {
        self.ensure_draft_usable()?;
        if self.connection_saving
            || self.snapshot.indexer.searching
            || self.snapshot.indexer.grabbing
        {
            return Err(AppError::InvalidInput(
                "Wait for the current Indexer save, search, or grab to finish.".into(),
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
        Ok(UiAction::Save {
            revision: self.edit_revision,
            update: self.connection_update(),
        })
    }
    fn edit_connection(
        &mut self,
        base_url: Option<String>,
        category_ids: Option<Vec<u32>>,
        api_key: Option<String>,
    ) -> Result<()> {
        let url = match base_url
            .map(super::providers::indexer::normalize_draft_url)
            .transpose()
        {
            Ok(url) => url,
            Err(error) => {
                self.snapshot.connection.draft_error = Some(AppErrorEnvelope::from(&error));
                self.changed();
                return Err(error);
            }
        };
        self.snapshot.connection.draft_error = None;
        self.url_edited |= url.is_some();
        let category_ids = category_ids.filter(|ids| {
            let refused = ids.iter().all(|id| *id == 0);
            self.snapshot.connection.empty_categories_refused = refused;
            !refused
        });
        self.categories_edited |= category_ids.is_some();
        self.edit_revision += 1;
        if let Some(url) = url {
            self.snapshot.connection.base_url = url;
        }
        if let Some(ids) = category_ids {
            self.snapshot.connection.category_ids = ids;
        }
        if let Some(key) = api_key {
            self.api_key = (!key.trim().is_empty()).then(|| key.trim().to_string());
        }
        self.snapshot.connection.api_key_entered = self.api_key.is_some();
        if !self.connection_saving {
            self.snapshot.connection.save = RemoteDraftStatus::Idle;
        }
        self.snapshot.connection.test = RemoteDraftStatus::Idle;
        self.snapshot.connection.test_result = None;
        Ok(())
    }

    fn connection_update(&self) -> RemoteIndexerConnectionUpdate {
        RemoteIndexerConnectionUpdate {
            base_url: Some(self.snapshot.connection.base_url.trim().to_string()),
            category_ids: Some(self.snapshot.connection.category_ids.clone()),
            api_key: self.api_key.clone(),
            clear_api_key: None,
        }
    }

    fn apply_connection(&mut self, connection: &RemoteIndexerConnection) {
        self.snapshot.connection.base_url = connection.base_url.clone().unwrap_or_default();
        self.snapshot.connection.category_ids = connection.category_ids.clone();
        self.api_key = None;
        self.snapshot.connection.api_key_entered = false;
    }

    fn loaded(&mut self, request: u64, revision: u64, result: &Result<RemoteIndexerConnection>) {
        self.connection_loading = false;
        if request != self.save_request {
            return;
        }
        match result {
            Ok(connection) => {
                self.connection_loaded = true;
                self.snapshot.connection.api_key_configured = connection.api_key_configured;
                if self.edit_revision == revision {
                    self.apply_connection(connection);
                } else {
                    // Fields typed while it loaded stay; the rest come from disk.
                    if !self.url_edited {
                        self.snapshot.connection.base_url =
                            connection.base_url.clone().unwrap_or_default();
                    }
                    if !self.categories_edited {
                        self.snapshot.connection.category_ids = connection.category_ids.clone();
                    }
                }
            }
            Err(error) => {
                self.snapshot.connection.save = RemoteDraftStatus::Failed {
                    error: error.into(),
                }
            }
        }
        self.changed();
    }

    fn saved(&mut self, revision: u64, result: &Result<RemoteIndexerConnection>) {
        self.connection_saving = false;
        match result {
            Ok(connection) => {
                self.connection_loaded = true;
                self.snapshot.connection.api_key_configured = connection.api_key_configured;
                if self.edit_revision == revision {
                    self.apply_connection(connection);
                }
                self.snapshot.connection.save = if self.edit_revision == revision {
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

    fn tested(
        &mut self,
        request: u64,
        revision: u64,
        result: &Result<RemoteIndexerConnectionTestResult>,
    ) {
        if revision != self.edit_revision || request != self.test_request {
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

fn release_key(indexer_id: i64, guid: &str) -> String {
    serde_json::to_string(&(indexer_id, guid)).expect("release identity is serializable")
}
impl UiState {
    fn indexer_available(&self) -> Result<()> {
        if self.snapshot.lane != ProviderId::Indexer
            || self.connection_saving
            || self.snapshot.indexer.searching
            || self.snapshot.indexer.grabbing
        {
            return Err(AppError::InvalidInput(
                "Wait for the current Indexer work to finish.".into(),
            ));
        }
        Ok(())
    }
    fn search(&mut self, author: String, title: String) -> Result<UiAction> {
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
        self.search_request += 1;
        Ok(UiAction::Search {
            request: self.search_request,
            query: RemoteReleaseSearchRequest {
                author: (!author.is_empty()).then(|| author.to_string()),
                title: (!title.is_empty()).then(|| title.to_string()),
                query: None,
            },
        })
    }
    fn select_release(&mut self, key: String, multi: bool) -> Result<()> {
        if !self
            .snapshot
            .indexer
            .releases
            .iter()
            .any(|release| release_key(release.indexer_id, &release.guid) == key)
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
    fn grab(&mut self, keys: Vec<String>) -> Result<UiAction> {
        self.indexer_available()?;
        let releases: Vec<_> = self
            .snapshot
            .indexer
            .releases
            .iter()
            .filter(|release| {
                let key = release_key(release.indexer_id, &release.guid);
                keys.contains(&key)
                    && !self
                        .snapshot
                        .indexer
                        .release_grabs
                        .get(&key)
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
                release_key(release.indexer_id, &release.guid),
                ReleaseGrabSnapshot {
                    status: ReleaseGrabStatus::Queued,
                    message: "Waiting to send.".into(),
                },
            );
        }
        self.snapshot.indexer.grabbing = true;
        self.snapshot.indexer.message = "Sending to downloader via Indexer…".into();
        Ok(UiAction::Grab(releases))
    }
    fn searched(&mut self, request: u64, result: &Result<RemoteReleaseSearchResponse>) {
        self.snapshot.indexer.searching = false;
        if request != self.search_request {
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

enum UiAction {
    Done,
    StartAuth,
    CompleteAuth(Option<String>),
    Disconnect(ProviderId),
    Account {
        lane: ProviderId,
        request: u64,
    },
    Library(u64),
    Load {
        request: u64,
        revision: u64,
    },
    Save {
        revision: u64,
        update: RemoteIndexerConnectionUpdate,
    },
    Test {
        request: u64,
        revision: u64,
        update: RemoteIndexerConnectionUpdate,
    },
    Acquire(AcquisitionPlan),
    CancelAcquisition(String),
    Search {
        request: u64,
        query: RemoteReleaseSearchRequest,
    },
    Grab(Vec<RemoteRelease>),
}

pub(crate) struct RemoteUiRun {
    runtime: RemoteSourceRuntime,
    action: Result<UiAction>,
}
pub(crate) enum RemoteUiResult {
    Applied,
    Saved,
    Superseded,
    AuthStarted(super::RemoteAuthStartResponse),
}

impl RemoteSourceRuntime {
    pub(super) fn ui(&self) -> std::sync::MutexGuard<'_, UiState> {
        self.inner
            .ui
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(crate) fn ui_revisions(&self) -> (u64, u64) {
        let state = self.ui();
        (state.snapshot.revision, state.library.revision)
    }
    pub(crate) fn ui_parts(
        &self,
        since: Option<(u64, u64)>,
    ) -> (Option<RemoteUiSnapshot>, Option<RemoteLibrarySnapshot>) {
        let state = self.ui();
        (
            since
                .is_none_or(|revision| state.snapshot.revision > revision.0)
                .then(|| state.snapshot()),
            since
                .is_none_or(|revision| state.library.revision > revision.1)
                .then(|| state.library.clone()),
        )
    }
    #[cfg(test)]
    pub(crate) fn ui_snapshot(&self) -> RemoteUiSnapshot {
        self.ui().snapshot()
    }
    pub(super) fn publish_ui(&self) {
        let update = {
            let mut state = self.ui();
            let mut update = crate::session::SessionUpdate::remote(state.snapshot());
            if state.library.revision > state.library_published {
                update.remote_library = Some(state.library.clone());
                state.library_published = state.library.revision;
            }
            update
        };
        self.inner.ui_host.emit(EngineEvent::Session(update));
    }
    pub(crate) fn ui_begin(&self, intent: RemoteUiIntent) -> RemoteUiRun {
        let action = {
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
        };
        self.publish_ui();
        RemoteUiRun {
            runtime: self.clone(),
            action,
        }
    }
}

impl RemoteSourceRuntime {
    async fn grab_ui_batch(&self, releases: Vec<RemoteRelease>) {
        // One turn covers every item, including the gap between two HTTP requests.
        let turn = self.indexer_turn();
        for release in &releases {
            let key = release_key(release.indexer_id, &release.guid);
            self.ui().snapshot.indexer.release_grabs.insert(
                key.clone(),
                ReleaseGrabSnapshot {
                    status: ReleaseGrabStatus::Sending,
                    message: "Sending to downloader via Indexer…".into(),
                },
            );
            self.ui().changed();
            self.publish_ui();
            // Shutdown stops a batch before its next release is sent.
            let result = match &turn {
                Ok(_) => {
                    self.inner
                        .tasks
                        .until_closing(self.grab_release(RemoteReleaseGrabRequest {
                            release: release.clone(),
                        }))
                        .await
                }
                Err(error) => Err(AppError::InvalidInput(error.to_string())),
            };
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
                .insert(key, outcome);
            self.ui().changed();
            self.publish_ui();
        }
        let mut state = self.ui();
        let outcomes: Vec<_> = releases
            .iter()
            .filter_map(|release| {
                state
                    .snapshot
                    .indexer
                    .release_grabs
                    .get(&release_key(release.indexer_id, &release.guid))
            })
            .collect();
        let sent = outcomes
            .iter()
            .filter(|outcome| outcome.status == ReleaseGrabStatus::Sent)
            .count();
        state.snapshot.indexer.message = if outcomes.len() == 1 {
            outcomes[0].message.clone()
        } else {
            format!(
                "{sent} releases sent. {} failed; retry their rows.",
                outcomes.len() - sent
            )
        };
        state.snapshot.indexer.grabbing = false;
        state.changed();
    }
}

impl RemoteSourceRuntime {
    async fn refresh_ui_library(&self, request: u64) -> Result<RemoteUiResult> {
        let result = self
            .inner
            .tasks
            .until_closing(super::AudibleProvider::load_library(
                self.inner.vault.as_ref(),
            ))
            .await;
        if !self.ui().library_result(request, &result) {
            return Ok(RemoteUiResult::Superseded);
        }
        self.publish_ui();
        result.map(|_| RemoteUiResult::Applied)
    }

    async fn refresh_ui_account(&self, lane: ProviderId, request: u64) -> Result<RemoteUiResult> {
        // A keychain read can wait on a system prompt; one started now would
        // hold shutdown behind it.
        if self.inner.tasks.is_closed() {
            return Err(AppError::General("ABB is closing.".into()));
        }
        let runtime = self.clone();
        // Once the keychain read starts, await its blocking worker so shutdown
        // cannot return while it still uses the vault.
        let result = tokio::task::spawn_blocking(move || runtime.account_state(lane))
            .await
            .map_err(|_| AppError::General("Account read failed.".into()))
            .and_then(|result| result);
        let library_request = {
            let mut state = self.ui();
            if !state.account_reply(lane, request, &result) {
                return Ok(RemoteUiResult::Superseded);
            }
            (lane == ProviderId::Audible
                && !self.inner.tasks.is_closed()
                && !state.snapshot.acquiring
                && state
                    .snapshot
                    .acquisition
                    .as_ref()
                    .is_none_or(|job| job.settled)
                && result
                    .as_ref()
                    .is_ok_and(|account| account.status == super::RemoteAccountStatus::Connected))
            .then(|| state.begin_library())
        };
        self.publish_ui();
        result?;
        if let Some(request) = library_request {
            return self.refresh_ui_library(request).await;
        }
        Ok(RemoteUiResult::Applied)
    }

    async fn complete_ui_auth(&self, path: Option<String>) -> Result<()> {
        let result = self
            .complete_auth(super::RemoteAuthCompletionRequest {
                provider_id: ProviderId::Audible,
                response_url_handoff_path: path.map(std::path::PathBuf::from),
            })
            .await;
        let refresh = {
            let mut state = self.ui();
            state.snapshot.auth = match &result {
                Ok(account) => {
                    state.snapshot.account = Some(account.clone());
                    state.snapshot.account_status = RemoteDraftStatus::Succeeded;
                    RemoteAuthStatus::Idle
                }
                Err(error) => RemoteAuthStatus::Failed {
                    error: error.into(),
                },
            };
            state.changed();
            result.is_ok().then(|| state.begin_library())
        };
        self.publish_ui();
        result?;
        if let Some(request) = refresh {
            self.refresh_ui_library(request).await?;
        }
        Ok(())
    }

    async fn disconnect_ui(&self, provider: ProviderId) -> Result<()> {
        // Admission keeps disconnecting set while the blocking vault operation
        // runs, so no acquisition can be admitted during credential deletion.
        let runtime = self.clone();
        let result = tokio::task::spawn_blocking(move || runtime.disconnect_credentials(provider))
            .await
            .map_err(|_| AppError::General("Account disconnect failed.".into()))
            .and_then(|result| result);
        let mut state = self.ui();
        state.disconnecting = false;
        match &result {
            Ok(account) => {
                state.snapshot.account = Some(account.clone());
                state.snapshot.account_status = RemoteDraftStatus::Succeeded;
            }
            Err(error) => {
                state.snapshot.account_status = RemoteDraftStatus::Failed {
                    error: error.into(),
                }
            }
        }
        state.changed();
        result.map(|_| ())
    }
}

impl RemoteUiRun {
    pub(crate) async fn finish(self) -> Result<RemoteUiResult> {
        let runtime = self.runtime;
        let result = match self.action? {
            UiAction::Done => Ok(RemoteUiResult::Applied),
            UiAction::Account { lane, request } => runtime.refresh_ui_account(lane, request).await,
            UiAction::Library(request) => runtime.refresh_ui_library(request).await,
            UiAction::StartAuth => {
                let result = runtime.start_auth(ProviderId::Audible);
                let mut state = runtime.ui();
                state.snapshot.auth = match &result {
                    Ok(_) => RemoteAuthStatus::AwaitingHandoff,
                    Err(error) => RemoteAuthStatus::Failed {
                        error: error.into(),
                    },
                };
                state.changed();
                result.map(RemoteUiResult::AuthStarted)
            }
            UiAction::CompleteAuth(path) => runtime
                .complete_ui_auth(path)
                .await
                .map(|_| RemoteUiResult::Applied),
            UiAction::Disconnect(provider) => runtime
                .disconnect_ui(provider)
                .await
                .map(|_| RemoteUiResult::Applied),
            // Indexer reads have nothing to save: shutdown drops them rather
            // than waiting out a slow server.
            UiAction::Load { request, revision } => {
                let tasks = runtime.inner.tasks.clone();
                let result = tasks.until_closing(runtime.get_indexer_connection()).await;
                runtime.ui().loaded(request, revision, &result);
                result.map(|_| RemoteUiResult::Applied)
            }
            UiAction::Save { revision, update } => {
                let result = runtime.update_indexer_connection(update).await;
                let refresh = {
                    let mut state = runtime.ui();
                    state.saved(revision, &result);
                    if state.snapshot.lane == ProviderId::Indexer {
                        state.account_request += 1;
                        state.snapshot.account_status = RemoteDraftStatus::Running;
                        Some(state.account_request)
                    } else {
                        None
                    }
                };
                if let Some(request) = refresh {
                    let _ = runtime
                        .refresh_ui_account(ProviderId::Indexer, request)
                        .await;
                }
                result.map(|_| RemoteUiResult::Saved)
            }
            UiAction::Test {
                request,
                revision,
                update,
            } => {
                let tasks = runtime.inner.tasks.clone();
                let result = tasks
                    .until_closing(runtime.test_indexer_connection(update))
                    .await;
                runtime.ui().tested(request, revision, &result);
                result.map(|_| RemoteUiResult::Applied)
            }
            UiAction::Search { request, query } => {
                let tasks = runtime.inner.tasks.clone();
                let result = tasks.until_closing(runtime.search_releases(query)).await;
                runtime.ui().searched(request, &result);
                result.map(|_| RemoteUiResult::Applied)
            }
            UiAction::Grab(releases) => {
                runtime.grab_ui_batch(releases).await;
                Ok(RemoteUiResult::Applied)
            }
            UiAction::CancelAcquisition(job_id) => runtime
                .cancel_acquisition(&job_id)
                .map(|_| RemoteUiResult::Applied),
            UiAction::Acquire(plan) => {
                let result = runtime.start_acquisition(plan).await;
                runtime.ui().snapshot.acquiring = false;
                runtime.ui().changed();
                result.map(|_| RemoteUiResult::Applied)
            }
        };
        runtime.publish_ui();
        result
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
