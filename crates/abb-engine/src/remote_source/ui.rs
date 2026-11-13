use super::providers::audible::{
    AudibleRequestError, PendingAudibleAuth, SIGN_IN_REJECTED_MESSAGE,
};
use super::types::AcquisitionWork;
use super::{
    ProviderId, RemoteAcquisitionSnapshot, RemoteIndexerConnectionTestResult,
    RemoteIndexerConnectionUpdate, RemoteRelease, RemoteReleaseGrabRequest,
    RemoteReleaseSearchRequest, RemoteReleaseSearchResponse, RemoteSourceRuntime, RemoteTitle,
};
use crate::app_settings::IndexerConnection;
use crate::errors::{AppError, AppErrorEnvelope, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemoteUiIntent {
    StartAuth,
    #[serde(rename_all = "camelCase")]
    CompleteAuth {
        response_url_handoff_path: Option<String>,
    },
    /// Disconnects the Audible sign-in; refused unless Audible is the lane on
    /// screen. The Indexer has no sign-in to remove.
    Disconnect,
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
    /// Saves one title's Supplemental PDF into the output folder, where the
    /// audiobook would go, without downloading the audiobook. The selection
    /// and the PDF choices stay as they are.
    #[serde(rename_all = "camelCase")]
    SavePdf {
        title_id: String,
    },
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
        key: String,
        multi: bool,
    },
    GrabSelected,
    #[serde(rename_all = "camelCase")]
    GrabRelease {
        key: String,
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

impl std::fmt::Debug for RemoteUiIntent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RemoteUiIntent (payload redacted)")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemoteDraftStatus {
    #[default]
    Idle,
    Running,
    Succeeded,
    Failed {
        error: AppErrorEnvelope,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexerCategoryOption {
    pub id: u32,
    pub label: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexerDraftSnapshot {
    pub base_url: String,
    /// The draft URL is plain HTTP, so the form warns that it is unencrypted.
    pub uses_http: bool,
    pub category_ids: Vec<u32>,
    /// Every category the form offers: the known ones, then any other
    /// category the draft or the saved connection holds, so it can be unchecked.
    pub category_options: Vec<IndexerCategoryOption>,
    pub api_key_configured: bool,
    pub api_key_entered: bool,
    pub save: RemoteDraftStatus,
    pub test: RemoteDraftStatus,
    pub test_result: Option<RemoteIndexerConnectionTestResult>,
    /// Why the API key or the URL was refused, the key first. The URL draft
    /// shows what was typed; the key draft keeps the value it accepted
    /// before. Save and Test refuse until a new value for each refused field
    /// is accepted.
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
    pub can_grab_selection: bool,
    pub release_grabs: BTreeMap<String, ReleaseGrabSnapshot>,
    pub searching: bool,
    pub message: String,
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
    #[serde(skip)]
    pub account_status: RemoteDraftStatus,
    pub selected_title_ids: Vec<String>,
    pub include_pdf_by_title_id: BTreeMap<String, bool>,
    pub indexer: IndexerWorkSnapshot,
    pub connection: IndexerDraftSnapshot,
    /// The shown lane refuses new work now; hosts disable its actions.
    pub busy: bool,
    /// The Audible lane's status line while no job is running.
    pub audible_message: String,
}

pub(super) struct UiState {
    snapshot: parts::Part<RemoteUiSnapshot>,
    library: parts::Part<RemoteLibrarySnapshot>,
    acquisition: parts::Part<RemoteAcquisitionSnapshot>,
    account_request: u64,
    audible: AudibleLane,
    indexer: IndexerLane,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            snapshot: parts::Part::new(RemoteUiSnapshot {
                revision: 0,
                lane: ProviderId::Audible,
                providers: vec![
                    super::AudibleProvider::capabilities(),
                    super::IndexerProvider::capabilities(),
                ],
                account: None,
                account_status: RemoteDraftStatus::Idle,
                selected_title_ids: Vec::new(),
                include_pdf_by_title_id: BTreeMap::new(),
                indexer: IndexerWorkSnapshot::default(),
                connection: IndexerDraftSnapshot {
                    base_url: String::new(),
                    uses_http: false,
                    category_ids: IndexerConnection::default().category_ids,
                    category_options: Vec::new(),
                    api_key_configured: false,
                    api_key_entered: false,
                    draft_error: None,
                    empty_categories_refused: false,
                    save: RemoteDraftStatus::Idle,
                    test: RemoteDraftStatus::Idle,
                    test_result: None,
                },
                busy: false,
                audible_message: String::new(),
            }),
            library: parts::Part::default(),
            acquisition: parts::Part::default(),
            account_request: 0,
            audible: AudibleLane::default(),
            indexer: IndexerLane::default(),
        }
    }
}

impl UiState {
    fn account_reply(
        &mut self,
        lane: ProviderId,
        request: u64,
        result: &Result<super::RemoteSourceAccountState>,
    ) -> bool {
        if lane != self.snapshot.lane
            || request != self.account_request
            || self.audible.disconnecting()
        {
            return false;
        }
        match result {
            Ok(account) => {
                self.snapshot.account = Some(self.shown_account(account.clone()));
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

    fn accept(&mut self) -> Result<()> {
        self.audible.refuse_while_disconnecting()?;
        self.changed();
        Ok(())
    }

    fn begin(&mut self, intent: RemoteUiIntent) -> Result<UiAction> {
        self.accept()?;
        match intent {
            RemoteUiIntent::StartAuth => return self.begin_auth_start(),
            RemoteUiIntent::CompleteAuth {
                response_url_handoff_path,
            } => return self.begin_auth_completion(response_url_handoff_path),

            RemoteUiIntent::Disconnect => return self.begin_disconnect(),

            RemoteUiIntent::RefreshLibrary => return self.refresh_library(),

            RemoteUiIntent::SelectLane { lane } => return self.select_lane(lane),

            RemoteUiIntent::SearchReleases { author, title } => return self.search(author, title),
            RemoteUiIntent::SelectRelease { key, multi } => self.select_release(key, multi)?,
            RemoteUiIntent::GrabSelected => {
                return self.grab(self.snapshot.indexer.selected_release_keys.clone());
            }
            RemoteUiIntent::GrabRelease { key } => return self.grab(vec![key]),
            RemoteUiIntent::ClearTitles => self.snapshot.selected_title_ids.clear(),
            RemoteUiIntent::ToggleTitle { title_id } => self.toggle_title(title_id)?,
            RemoteUiIntent::TogglePdf { title_id } => self.toggle_pdf(title_id)?,
            RemoteUiIntent::CancelAcquisition { job_id } => {
                return Ok(UiAction::CancelAcquisition(job_id))
            }
            RemoteUiIntent::AcquireSelected => return self.acquire_selected(),
            RemoteUiIntent::SavePdf { .. } => {
                unreachable!("ui_begin routes a PDF save through begin_save_pdf")
            }
            RemoteUiIntent::LoadConnection => {
                if self.indexer.load != ConnectionLoad::NotLoaded {
                    return Ok(UiAction::Done);
                }
                self.indexer.load = ConnectionLoad::Loading;
                return Ok(UiAction::Load {
                    request: self.indexer.connection_generation,
                });
            }
            RemoteUiIntent::EditConnection {
                base_url,
                category_ids,
                api_key,
            } => {
                self.edit_connection(base_url, category_ids, api_key)?;
            }
            RemoteUiIntent::SaveConnection => {
                unreachable!("ui_begin routes a Save through begin_save")
            }
            RemoteUiIntent::TestConnection => {
                self.ensure_draft_usable()?;
                self.indexer.test_request += 1;
                self.snapshot.connection.test = RemoteDraftStatus::Running;
                self.snapshot.connection.test_result = None;
                return Ok(UiAction::Test {
                    request: self.indexer.test_request,
                    revision: self.indexer.edit_revision,
                    update: self.connection_update(),
                });
            }
        }
        Ok(UiAction::Done)
    }

    fn select_lane(&mut self, lane: ProviderId) -> Result<UiAction> {
        self.audible.refuse_while_changing()?;
        if self.indexer.grabbing {
            return Err(indexer_lane::busy_refusal(None));
        }
        if self.snapshot.lane == lane
            && (self.snapshot.account_status == RemoteDraftStatus::Running
                || self.audible.library_status == RemoteDraftStatus::Running)
        {
            return Ok(UiAction::Done);
        }
        let request = self.change_lane(lane);
        Ok(UiAction::Account { lane, request })
    }

    fn change_lane(&mut self, lane: ProviderId) -> u64 {
        if self.snapshot.lane != lane {
            self.snapshot.account = None;
            self.snapshot.selected_title_ids.clear();
            self.indexer.search_request += 1;
            self.snapshot.indexer = IndexerWorkSnapshot {
                searching: self.snapshot.indexer.searching,
                ..IndexerWorkSnapshot::default()
            };
        }
        self.audible.supersede_library_read();
        self.snapshot.lane = lane;
        self.begin_account_read()
    }
}

enum UiAction {
    Done,
    StartAuth,
    CompleteAuth {
        response_url_handoff_path: Option<String>,
        pending: PendingAudibleAuth,
    },
    Disconnect,
    Account {
        lane: ProviderId,
        request: u64,
    },
    Library(u64),
    Load {
        request: u64,
    },
    Save(indexer_save::IndexerSave),
    Test {
        request: u64,
        revision: u64,
        update: RemoteIndexerConnectionUpdate,
    },
    Acquire(AcquisitionWork),
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
    pub(crate) fn ui_begin(&self, intent: RemoteUiIntent) -> RemoteUiRun {
        let action = match intent {
            RemoteUiIntent::SaveConnection => self.begin_save(),
            RemoteUiIntent::SavePdf { title_id } => {
                let output_folder_set = self
                    .inner
                    .settings
                    .output_defaults()
                    .output_directory
                    .is_some();
                self.ui().begin_save_pdf(title_id, output_folder_set)
            }
            intent => self.ui().begin(intent),
        };
        self.publish_ui();
        RemoteUiRun {
            runtime: self.clone(),
            action,
        }
    }
}

impl RemoteSourceRuntime {
    async fn refresh_ui_account(&self, lane: ProviderId, request: u64) -> Result<RemoteUiResult> {
        let runtime = self.clone();
        let result = self
            .inner
            .tasks
            .admit(|| {
                #[expect(clippy::disallowed_methods, reason = "joined by refresh_ui_account")]
                let read = tokio::task::spawn_blocking(move || runtime.account_state(lane));
                read
            })?
            .await
            .map_err(|_| AppError::General("Account read failed.".into()))
            .and_then(|result| result);
        let library_eligible =
            {
                let mut state = self.ui();
                if !state.account_reply(lane, request, &result) {
                    return Ok(RemoteUiResult::Superseded);
                }
                lane == ProviderId::Audible
                    && !state.acquisition.busy()
                    && result.is_ok()
                    && state.snapshot.account.as_ref().is_some_and(|account| {
                        account.status == super::RemoteAccountStatus::Connected
                    })
            };
        self.publish_ui();
        result?;
        if library_eligible {
            if let Ok(request) = self.inner.tasks.admit(|| self.ui().begin_library()) {
                self.publish_ui();
                return self.refresh_ui_library(request).await;
            }
        }
        Ok(RemoteUiResult::Applied)
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
                let result = runtime.start_auth();
                runtime
                    .ui()
                    .auth_started(result)
                    .map(RemoteUiResult::AuthStarted)
            }
            UiAction::CompleteAuth {
                response_url_handoff_path,
                pending,
            } => runtime
                .complete_ui_auth(response_url_handoff_path, pending)
                .await
                .map(|_| RemoteUiResult::Applied),
            UiAction::Disconnect => runtime
                .disconnect_ui()
                .await
                .map(|_| RemoteUiResult::Applied),
            UiAction::Load { request } => {
                let tasks = runtime.inner.tasks.clone();
                let result = tasks
                    .until_closing(runtime.indexer_api_key_configured())
                    .await;
                runtime.ui().loaded(request, &result);
                result.map(|_| RemoteUiResult::Applied)
            }
            UiAction::Save(save) => runtime.finish_indexer_save(save).await,
            UiAction::Test {
                request,
                revision,
                update,
            } => {
                let result = runtime.test_indexer_connection(update).await;
                runtime.ui().tested(request, revision, &result);
                result.map(|_| RemoteUiResult::Applied)
            }
            UiAction::Search { request, query } => {
                let result = runtime.search_releases(query).await;
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
            UiAction::Acquire(work) => {
                let result = runtime.start_acquisition(work).await;
                runtime
                    .ui()
                    .set_acquisition(|acquisition| acquisition.acquiring = false);
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

#[cfg(test)]
#[path = "lane_tests.rs"]
mod lane_tests;

#[path = "ui_parts.rs"]
mod parts;

#[path = "audible_lane.rs"]
mod audible_lane;
use audible_lane::AudibleLane;

#[path = "indexer_lane.rs"]
mod indexer_lane;
use indexer_lane::{ConnectionLoad, IndexerLane};

#[path = "lane_status.rs"]
mod lane_status;

#[path = "indexer_save.rs"]
mod indexer_save;
