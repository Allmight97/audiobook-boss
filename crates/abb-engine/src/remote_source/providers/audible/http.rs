use std::time::Duration;

use audible_api::api::Client as AudibleClient;
use audible_api::auth::Auth;
use reqwest::redirect::Policy;
use serde_json::Value;

use super::provider_private_failure;
use crate::errors::{AppError, Result};
use abb_audible_core::title_ref;

const READ_TIMEOUT: Duration = Duration::from_secs(60);
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub(in crate::remote_source) const SIGN_IN_REJECTED_MESSAGE: &str =
    "Audible did not accept the saved sign-in. Sign in to Audible again.";

/// Follows redirects with an HTTPS-only guard and redirect cap.
pub(super) fn audio_download_client(max_redirects: usize) -> Result<reqwest::Client> {
    client(Policy::custom(move |attempt| {
        if attempt.previous().len() >= max_redirects {
            return attempt.error("remote source download exceeded redirect limit");
        }
        if attempt.url().scheme() != "https" {
            return attempt.error("remote source download redirect must use https");
        }
        attempt.follow()
    }))
}

/// Follows no redirects; callers handle redirect loops themselves.
pub(super) fn no_redirect_client() -> Result<reqwest::Client> {
    client(Policy::none())
}

fn client(redirect: Policy) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(redirect)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|_| provider_private_failure("download client"))
}

#[derive(Debug)]
pub(in crate::remote_source) enum AudibleRequestError {
    /// Audible answered 401 or 403: the saved sign-in no longer works.
    AuthRejected,
    Failed(AppError),
}

impl From<AppError> for AudibleRequestError {
    fn from(error: AppError) -> Self {
        Self::Failed(error)
    }
}

impl AudibleRequestError {
    pub(in crate::remote_source) fn into_app_error(self) -> AppError {
        match self {
            Self::AuthRejected => AppError::General(SIGN_IN_REJECTED_MESSAGE.to_string()),
            Self::Failed(error) => error,
        }
    }
}

/// Signed JSON requests to the Audible API on the auth's own marketplace.
pub(super) struct AudibleApi {
    client: AudibleClient,
    base_url: String,
    builder: audible_reqwest::Client,
}

impl AudibleApi {
    pub(super) fn new(auth: Auth) -> Result<Self> {
        // The client signs the URL path left after removing this exact prefix.
        let base_url = format!("https://api.audible.{}", auth.locale.domain);
        let client = AudibleClient::new(auth).map_err(|error| {
            AppError::General(format!("Failed to create Audible client: {error}"))
        })?;
        Ok(Self::with_client(client, base_url))
    }

    pub(super) fn with_client(client: AudibleClient, base_url: String) -> Self {
        Self {
            client,
            base_url,
            builder: audible_reqwest::Client::new(),
        }
    }

    pub(super) async fn get(
        &self,
        path: &str,
        query: &Value,
        call: ApiCall<'_>,
    ) -> std::result::Result<Value, AudibleRequestError> {
        let request = self
            .builder
            .get(format!("{}{path}", self.base_url))
            .query(query);
        self.send(request, call).await
    }

    pub(super) async fn post(
        &self,
        path: &str,
        body: &Value,
        call: ApiCall<'_>,
    ) -> std::result::Result<Value, AudibleRequestError> {
        let request = self
            .builder
            .post(format!("{}{path}", self.base_url))
            .json(body);
        self.send(request, call).await
    }

    async fn send(
        &self,
        request: audible_reqwest::RequestBuilder,
        call: ApiCall<'_>,
    ) -> std::result::Result<Value, AudibleRequestError> {
        let stage = call.stage;
        let request = request
            .timeout(API_REQUEST_TIMEOUT)
            .build()
            .map_err(|_| provider_private_failure(&format!("{stage} construction")))?;
        let response = self.client.send_request(request).await.map_err(|_| {
            log::warn!(
                "remote_source audible stage=api_request_failed {}",
                call.log_fields()
            );
            provider_private_failure(stage)
        })?;
        let status = response.status().as_u16();
        log::info!(
            "remote_source audible stage=api_reply {} http_status={status}",
            call.log_fields()
        );
        if !response.status().is_success() {
            return json_reply(call, status, &[]);
        }
        let body = response
            .bytes()
            .await
            .map_err(|_| provider_private_failure(stage))?;
        json_reply(call, status, &body)
    }
}

/// Which refusal means the saved sign-in no longer works.
#[derive(Clone, Copy)]
pub(super) enum Scope {
    /// Library pages and title lookup: 401 or 403.
    Account,
    /// A license: only 401. A 403 can refuse just this title (entitlement,
    /// region) or be a rate limit while the sign-in is fine.
    Title,
}

#[derive(Clone, Copy)]
pub(super) struct ApiCall<'a> {
    stage: &'a str,
    scope: Scope,
    job: Option<(&'a str, &'a str)>,
}

impl<'a> ApiCall<'a> {
    pub(super) fn account(stage: &'a str) -> Self {
        Self {
            stage,
            scope: Scope::Account,
            job: None,
        }
    }

    pub(super) fn for_job(
        stage: &'a str,
        scope: Scope,
        job_id: &'a str,
        title_id: &'a str,
    ) -> Self {
        Self {
            stage,
            scope,
            job: Some((job_id, title_id)),
        }
    }

    fn log_fields(self) -> String {
        let request = self.stage.replace(' ', "_");
        match self.job {
            Some((job_id, title_id)) => format!(
                "request={request} job_id={job_id} title_ref={}",
                title_ref(title_id)
            ),
            None => format!("request={request}"),
        }
    }
}

fn json_reply(
    call: ApiCall<'_>,
    status: u16,
    body: &[u8],
) -> std::result::Result<Value, AudibleRequestError> {
    let stage = call.stage;
    match (status, call.scope) {
        (401, _) | (403, Scope::Account) => Err(AudibleRequestError::AuthRejected),
        (200..=299, _) => serde_json::from_slice(body).map_err(|_| {
            log::warn!(
                "remote_source audible stage=api_reply_failed {} failure=json_parse",
                call.log_fields()
            );
            provider_private_failure(&format!("{stage} response parse")).into()
        }),
        _ => Err(AppError::General(format!(
            "Audible {stage} returned HTTP {status}. Check application logs for sanitized acquisition facts."
        ))
        .into()),
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
pub(super) mod stub;
