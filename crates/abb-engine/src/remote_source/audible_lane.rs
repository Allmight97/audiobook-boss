use super::super::AcquisitionSelection;
use super::{
    AcquisitionPlan, AppError, AudibleRequestError, ProviderId, RemoteAuthStatus,
    RemoteDraftStatus, RemoteSourceRuntime, RemoteTitle, RemoteUiResult, Result, UiAction, UiState,
    SIGN_IN_REJECTED_MESSAGE,
};
use std::collections::BTreeSet;

impl UiState {
    pub(super) fn begin_auth_start(&mut self) -> Result<UiAction> {
        self.account_change_allowed()?;
        self.account_request += 1;
        self.library_request += 1;
        self.snapshot.account_status = RemoteDraftStatus::Idle;
        self.snapshot.library_status = RemoteDraftStatus::Idle;
        self.snapshot.auth = RemoteAuthStatus::Starting;
        Ok(UiAction::StartAuth)
    }

    fn account_change_allowed(&self) -> Result<()> {
        if self.snapshot.lane != ProviderId::Audible {
            return Err(AppError::InvalidInput(
                "Indexer uses connection settings instead of browser auth.".into(),
            ));
        }
        self.disconnect_allowed()
    }

    /// Applies a finished sign-in; returns the library read it starts.
    pub(super) fn auth_completed(
        &mut self,
        result: &Result<super::super::RemoteSourceAccountState>,
    ) -> Option<u64> {
        self.snapshot.auth = match result {
            Ok(account) => {
                self.sign_in_rejected = false;
                self.snapshot.account = Some(account.clone());
                self.snapshot.account_status = RemoteDraftStatus::Succeeded;
                RemoteAuthStatus::Idle
            }
            Err(error) => RemoteAuthStatus::Failed {
                error: error.into(),
            },
        };
        self.changed();
        result.is_ok().then(|| self.begin_library())
    }

    pub(super) fn shown_account(
        &self,
        account: super::super::RemoteSourceAccountState,
    ) -> super::super::RemoteSourceAccountState {
        if !self.sign_in_rejected
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
        self.sign_in_rejected = true;
        if let Some(account) = self.snapshot.account.take() {
            self.snapshot.account = Some(self.shown_account(account));
        }
    }

    pub(super) fn library_result(
        &mut self,
        request: u64,
        result: &std::result::Result<super::super::RemoteLibraryResponse, AudibleRequestError>,
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
            Err(AudibleRequestError::AuthRejected) => {
                self.reject_sign_in();
                self.snapshot.library_status = RemoteDraftStatus::Failed {
                    error: AudibleRequestError::AuthRejected.into_app_error().into(),
                };
                self.changed();
            }
            Err(AudibleRequestError::Failed(error)) => {
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

    pub(super) fn disconnect_allowed(&self) -> Result<()> {
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
        if self.acquisition.busy() {
            return Err(AppError::InvalidInput(
                "Wait for the Audible acquisition and handoff before disconnecting.".into(),
            ));
        }
        Ok(())
    }

    pub(in crate::remote_source) fn account_disconnected(
        &mut self,
        account: Option<super::super::RemoteSourceAccountState>,
    ) {
        self.snapshot.account = account;
        self.changed();
    }

    pub(in crate::remote_source) fn disconnected(&mut self) {
        self.library_request += 1;
        self.account_request += 1;
        self.snapshot.library_status = RemoteDraftStatus::Idle;
        self.sign_in_rejected = false;
        self.snapshot.auth = RemoteAuthStatus::Idle;
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

    pub(super) fn refresh_library(&mut self) -> Result<UiAction> {
        if self.snapshot.lane != ProviderId::Audible
            || self.acquisition.busy()
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

    pub(super) fn begin_disconnect(&mut self) -> Result<UiAction> {
        if self.snapshot.lane != ProviderId::Audible {
            return Err(AppError::InvalidInput(
                "That account is no longer the one shown. Try again.".into(),
            ));
        }
        self.disconnect_allowed()?;
        self.disconnecting = true;
        self.account_request += 1;
        self.library_request += 1;
        self.snapshot.account_status = RemoteDraftStatus::Running;
        self.snapshot.library_status = RemoteDraftStatus::Idle;
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
        if self.disconnecting
            || matches!(
                self.snapshot.auth,
                RemoteAuthStatus::Starting | RemoteAuthStatus::Completing
            )
            || self.acquisition.busy()
            || self.snapshot.lane != ProviderId::Audible
            || self.snapshot.selected_title_ids.is_empty()
        {
            return Err(AppError::InvalidInput(
                "Select Audible titles before acquiring.".into(),
            ));
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

    pub(super) async fn complete_ui_auth(&self, path: Option<String>) -> Result<()> {
        let result = self
            .complete_auth(super::super::RemoteAuthCompletionRequest {
                provider_id: ProviderId::Audible,
                response_url_handoff_path: path.map(std::path::PathBuf::from),
            })
            .await;
        let refresh = self.ui().auth_completed(&result);
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
