use super::super::{AcquisitionSelection, PendingAudibleAuth};
use super::{
    AcquisitionPlan, AppError, AppErrorEnvelope, AudibleRequestError, ProviderId,
    RemoteDraftStatus, RemoteSourceRuntime, RemoteTitle, RemoteUiResult, Result, UiAction, UiState,
    SIGN_IN_REJECTED_MESSAGE,
};
use std::collections::BTreeSet;

const ACCOUNT_CHANGING: &str = "Wait for the current account change to finish.";
const DISCONNECT_DURING_ACQUISITION: &str =
    "Wait for the Audible acquisition and handoff before disconnecting.";
const REFRESH_WHILE_BUSY: &str = "Wait for the current Audible work before refreshing.";
const ACQUIRE_DURING_ACQUISITION: &str = "Wait for the current Audible acquisition to finish.";
const NOTHING_TO_ACQUIRE: &str = "Select Audible titles before acquiring.";

/// Where the Audible account change stands. The pending sign-in lives only in
/// `AwaitingHandoff`; leaving that state hands it out or drops it.
#[derive(Default)]
pub(super) enum AudibleSignIn {
    #[default]
    Idle,
    Starting,
    AwaitingHandoff(PendingAudibleAuth),
    Completing,
    Failed(AppErrorEnvelope),
    Disconnecting,
}

/// Audible's sign-in, library read, and job-line facts, with the predicates
/// that decide what the lane accepts. Account facts shared with the Indexer
/// stay on the UI state.
#[derive(Default)]
pub(super) struct AudibleLane {
    pub(super) sign_in: AudibleSignIn,
    pub(super) library_status: RemoteDraftStatus,
    pub(super) library_request: u64,
    pub(super) sign_in_rejected: bool,
    pub(super) settled_job_line: Option<String>,
}

impl AudibleLane {
    pub(super) fn disconnecting(&self) -> bool {
        matches!(self.sign_in, AudibleSignIn::Disconnecting)
    }

    fn signing_in(&self) -> bool {
        matches!(
            self.sign_in,
            AudibleSignIn::Starting | AudibleSignIn::Completing
        )
    }

    pub(super) fn is_changing(&self) -> bool {
        self.disconnecting() || self.signing_in()
    }

    pub(super) fn is_busy(&self, acquisition_busy: bool) -> bool {
        self.library_status == RemoteDraftStatus::Running || self.is_changing() || acquisition_busy
    }

    pub(super) fn refuse_while_disconnecting(&self) -> Result<()> {
        if self.disconnecting() {
            return Err(AppError::InvalidInput(
                "Wait for disconnect to finish.".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn refuse_while_changing(&self) -> Result<()> {
        if self.is_changing() {
            return Err(AppError::InvalidInput(ACCOUNT_CHANGING.into()));
        }
        Ok(())
    }

    pub(super) fn can_disconnect(&self, acquisition_busy: bool) -> Result<()> {
        self.refuse_while_changing()?;
        if acquisition_busy {
            return Err(AppError::InvalidInput(DISCONNECT_DURING_ACQUISITION.into()));
        }
        Ok(())
    }

    fn can_refresh(&self, acquisition_busy: bool) -> Result<()> {
        if self.is_busy(acquisition_busy) {
            return Err(AppError::InvalidInput(REFRESH_WHILE_BUSY.into()));
        }
        Ok(())
    }

    pub(super) fn supersede_library_read(&mut self) {
        self.library_request += 1;
        self.library_status = RemoteDraftStatus::Idle;
    }
}

impl UiState {
    pub(super) fn begin_account_read(&mut self) -> u64 {
        self.supersede_account_read();
        self.snapshot.account_status = RemoteDraftStatus::Running;
        self.account_request
    }

    pub(super) fn supersede_account_read(&mut self) {
        self.account_request += 1;
    }

    fn expire_reads(&mut self) {
        self.supersede_account_read();
        self.snapshot.account_status = RemoteDraftStatus::Idle;
        self.audible.supersede_library_read();
    }

    pub(super) fn begin_auth_start(&mut self) -> Result<UiAction> {
        self.account_change_allowed()?;
        self.expire_reads();
        self.audible.sign_in = AudibleSignIn::Starting;
        self.audible.settled_job_line = None;
        Ok(UiAction::StartAuth)
    }

    pub(super) fn auth_started(
        &mut self,
        result: Result<(super::super::RemoteAuthStartResponse, PendingAudibleAuth)>,
    ) -> Result<super::super::RemoteAuthStartResponse> {
        let result = result.map(|(start, pending)| {
            self.audible.sign_in = AudibleSignIn::AwaitingHandoff(pending);
            start
        });
        if let Err(error) = &result {
            self.audible.sign_in = AudibleSignIn::Failed(error.into());
        }
        self.changed();
        result
    }

    fn account_change_allowed(&self) -> Result<()> {
        if self.snapshot.lane != ProviderId::Audible {
            return Err(AppError::InvalidInput(
                "Indexer uses connection settings instead of browser auth.".into(),
            ));
        }
        self.audible.can_disconnect(self.acquisition.busy())
    }

    /// Applies a finished sign-in; returns the library read it starts.
    /// `unused_sign_in` is the pending sign-in a completion handed back
    /// because it failed before it reached Amazon.
    pub(super) fn auth_completed(
        &mut self,
        result: &Result<super::super::RemoteSourceAccountState>,
        unused_sign_in: Option<PendingAudibleAuth>,
    ) -> Option<u64> {
        self.audible.sign_in = match (result, unused_sign_in) {
            (Ok(account), _) => {
                self.audible.sign_in_rejected = false;
                self.snapshot.account = Some(account.clone());
                self.snapshot.account_status = RemoteDraftStatus::Succeeded;
                AudibleSignIn::Idle
            }
            (Err(error), Some(pending)) => {
                self.snapshot.account_status = RemoteDraftStatus::Failed {
                    error: error.into(),
                };
                AudibleSignIn::AwaitingHandoff(pending)
            }
            (Err(error), None) => AudibleSignIn::Failed(error.into()),
        };
        self.changed();
        result.is_ok().then(|| self.begin_library())
    }

    pub(super) fn shown_account(
        &self,
        account: super::super::RemoteSourceAccountState,
    ) -> super::super::RemoteSourceAccountState {
        if !self.audible.sign_in_rejected
            || account.provider_id != ProviderId::Audible
            || account.status != super::super::RemoteAccountStatus::Connected
        {
            return account;
        }
        super::super::RemoteSourceAccountState {
            status: super::super::RemoteAccountStatus::NeedsAuth,
            message: Some(SIGN_IN_REJECTED_MESSAGE.to_string()),
            ..account
        }
    }

    pub(super) fn reject_sign_in(&mut self) {
        self.audible.sign_in_rejected = true;
        if let Some(account) = self.snapshot.account.take() {
            self.snapshot.account = Some(self.shown_account(account));
        }
    }

    pub(super) fn library_result(
        &mut self,
        request: u64,
        result: &std::result::Result<super::super::RemoteLibraryResponse, AudibleRequestError>,
    ) -> bool {
        if request != self.audible.library_request
            || self.snapshot.lane != ProviderId::Audible
            || self.audible.disconnecting()
        {
            return false;
        }
        match result {
            Ok(library) => {
                self.library_loaded_with_diagnostics(
                    library.titles.clone(),
                    library.diagnostics.clone(),
                );
                self.audible.library_status = RemoteDraftStatus::Succeeded;
                if matches!(self.audible.sign_in, AudibleSignIn::Failed(_)) {
                    self.audible.sign_in = AudibleSignIn::Idle;
                }
                if matches!(
                    self.snapshot.account_status,
                    RemoteDraftStatus::Failed { .. }
                ) {
                    self.snapshot.account_status = RemoteDraftStatus::Idle;
                }
            }
            Err(AudibleRequestError::AuthRejected) => {
                self.reject_sign_in();
                self.audible.library_status = RemoteDraftStatus::Failed {
                    error: AudibleRequestError::AuthRejected.into_app_error().into(),
                };
                self.changed();
            }
            Err(AudibleRequestError::Failed(error)) => {
                self.audible.library_status = RemoteDraftStatus::Failed {
                    error: error.into(),
                };
                self.changed();
            }
        }
        true
    }

    pub(super) fn begin_library(&mut self) -> u64 {
        self.audible.supersede_library_read();
        self.audible.library_status = RemoteDraftStatus::Running;
        self.audible.settled_job_line = None;
        self.changed();
        self.audible.library_request
    }

    pub(in crate::remote_source) fn account_disconnected(
        &mut self,
        account: Option<super::super::RemoteSourceAccountState>,
    ) {
        self.snapshot.account = account;
        self.changed();
    }

    pub(in crate::remote_source) fn disconnected(&mut self) {
        self.audible.supersede_library_read();
        self.supersede_account_read();
        self.audible.sign_in_rejected = false;
        self.audible.settled_job_line = None;
        self.set_acquisition(|acquisition| acquisition.job = None);
        self.library_loaded(Vec::new());
    }

    pub(in crate::remote_source) fn library_loaded(&mut self, titles: Vec<RemoteTitle>) {
        self.library_loaded_with_diagnostics(titles, Vec::new());
    }

    fn library_loaded_with_diagnostics(
        &mut self,
        titles: Vec<RemoteTitle>,
        diagnostics: Vec<super::super::RemoteSourceDiagnostic>,
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
            self.library.bump();
        }
        self.changed();
    }

    pub(super) fn begin_auth_completion(
        &mut self,
        response_url_handoff_path: Option<String>,
    ) -> Result<UiAction> {
        self.account_change_allowed()?;
        let AudibleSignIn::AwaitingHandoff(_) = self.audible.sign_in else {
            return Err(AppError::InvalidInput(
                "Start Audible auth before completing the handoff.".into(),
            ));
        };
        self.expire_reads();
        let AudibleSignIn::AwaitingHandoff(pending) =
            std::mem::replace(&mut self.audible.sign_in, AudibleSignIn::Completing)
        else {
            unreachable!("the sign-in was awaiting its handoff a moment ago")
        };
        self.audible.settled_job_line = None;
        Ok(UiAction::CompleteAuth {
            response_url_handoff_path,
            pending,
        })
    }

    pub(super) fn refresh_library(&mut self) -> Result<UiAction> {
        if self.snapshot.lane != ProviderId::Audible {
            return Err(AppError::InvalidInput(REFRESH_WHILE_BUSY.into()));
        }
        self.audible.can_refresh(self.acquisition.busy())?;
        let request = self.begin_library();
        Ok(UiAction::Library(request))
    }

    pub(super) fn begin_disconnect(&mut self) -> Result<UiAction> {
        if self.snapshot.lane != ProviderId::Audible {
            return Err(AppError::InvalidInput(
                "That account is no longer the one shown. Try again.".into(),
            ));
        }
        self.audible.can_disconnect(self.acquisition.busy())?;
        self.audible.sign_in = AudibleSignIn::Disconnecting;
        self.audible.supersede_library_read();
        self.begin_account_read();
        Ok(UiAction::Disconnect)
    }

    pub(super) fn toggle_title(&mut self, title_id: String) -> Result<()> {
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

    pub(super) fn toggle_pdf(&mut self, title_id: String) -> Result<()> {
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

    pub(super) fn acquire_selected(&mut self) -> Result<UiAction> {
        self.audible.refuse_while_changing()?;
        if self.acquisition.busy() {
            return Err(AppError::InvalidInput(ACQUIRE_DURING_ACQUISITION.into()));
        }
        if self.snapshot.lane != ProviderId::Audible || self.snapshot.selected_title_ids.is_empty()
        {
            return Err(AppError::InvalidInput(NOTHING_TO_ACQUIRE.into()));
        }
        self.set_acquisition(|acquisition| acquisition.acquiring = true);
        Ok(UiAction::Acquire(AcquisitionPlan {
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
}

impl RemoteSourceRuntime {
    pub(crate) fn library_offers_cover(&self, cover: &str) -> bool {
        self.ui()
            .library
            .titles
            .iter()
            .any(|title| title.cover.as_deref() == Some(cover))
    }

    pub(super) async fn refresh_ui_library(&self, request: u64) -> Result<RemoteUiResult> {
        let result = async {
            let auth = self
                .vault_blocking(|runtime| {
                    super::super::AudibleProvider::stored_auth(runtime.inner.vault.as_ref())
                })
                .await?;
            self.inner
                .tasks
                .until_closing(async {
                    Ok(super::super::AudibleProvider::load_library(auth).await)
                })
                .await?
        }
        .await;
        if !self.ui().library_result(request, &result) {
            return Ok(RemoteUiResult::Superseded);
        }
        self.publish_ui();
        result
            .map(|_| RemoteUiResult::Applied)
            .map_err(AudibleRequestError::into_app_error)
    }

    pub(super) async fn complete_ui_auth(
        &self,
        path: Option<String>,
        pending: PendingAudibleAuth,
    ) -> Result<()> {
        let (result, unused_sign_in) = match super::super::read_audible_response_url(path) {
            Ok(response_url) => (self.complete_auth(pending, &response_url).await, None),
            Err(error) => (Err(error), Some(pending)),
        };
        let refresh = self.ui().auth_completed(&result, unused_sign_in);
        self.publish_ui();
        result?;
        if let Some(request) = refresh {
            self.refresh_ui_library(request).await?;
        }
        Ok(())
    }

    pub(super) async fn disconnect_ui(&self) -> Result<()> {
        let runtime = self.clone();
        #[expect(clippy::disallowed_methods, reason = "joined by disconnect_ui")]
        let result = tokio::task::spawn_blocking(move || runtime.disconnect_credentials())
            .await
            .map_err(|_| AppError::General("Account disconnect failed.".into()))
            .and_then(|result| result);
        let mut state = self.ui();
        state.audible.sign_in = AudibleSignIn::Idle;
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
