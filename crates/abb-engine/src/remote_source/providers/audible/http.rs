use std::time::{Duration, SystemTime};

use audible_api::auth::Auth;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::redirect::Policy;
use serde_json::Value;

use super::provider_private_failure;
use super::signing::{adp_date, AdpSigner};
use crate::errors::{AppError, Result};
use abb_audible_core::title_ref;

const READ_TIMEOUT: Duration = Duration::from_secs(60);
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const AUDIBLE_API_HOST_PREFIX: &str = "https://api.audible.";

pub(in crate::remote_source) const SIGN_IN_REJECTED_MESSAGE: &str =
    "Audible did not accept the saved sign-in. Sign in to Audible again.";

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

pub(super) struct AudibleApi {
    client: reqwest::Client,
    signer: AdpSigner,
    base_url: String,
}

impl AudibleApi {
    pub(super) fn new(auth: &Auth) -> Result<Self> {
        Self::at(
            auth,
            format!("{AUDIBLE_API_HOST_PREFIX}{}", auth.locale.domain),
        )
    }

    fn at(auth: &Auth, base_url: String) -> Result<Self> {
        let registration = &auth.device_registration;
        let signer = AdpSigner::new(&registration.adp_token, &registration.device_private_key)?;
        log::info!(
            "remote_source audible stage=device_key key_bits={}",
            signer.key_bits()
        );
        let client = reqwest::Client::builder()
            .default_headers(api_default_headers())
            .build()
            .map_err(|_| provider_private_failure("API client"))?;
        Ok(Self {
            client,
            signer,
            base_url,
        })
    }

    pub(super) fn marketplace_domain(&self) -> &str {
        self.base_url
            .strip_prefix(AUDIBLE_API_HOST_PREFIX)
            .unwrap_or("non-audible-host")
    }

    pub(super) async fn get(
        &self,
        path: &str,
        query: &Value,
        call: ApiCall<'_>,
    ) -> std::result::Result<Value, AudibleRequestError> {
        let request = self
            .client
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
            .client
            .post(format!("{}{path}", self.base_url))
            .json(body);
        self.send(request, call).await
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        call: ApiCall<'_>,
    ) -> std::result::Result<Value, AudibleRequestError> {
        let stage = call.stage;
        let mut request = request
            .timeout(API_REQUEST_TIMEOUT)
            .build()
            .map_err(|_| provider_private_failure(&format!("{stage} construction")))?;
        let signed = self.signer.headers(
            request.method().as_str(),
            &request.url().as_str().replace(&self.base_url, ""),
            request
                .body()
                .and_then(reqwest::Body::as_bytes)
                .unwrap_or_default(),
            &adp_date(SystemTime::now()),
        )?;
        request.headers_mut().extend(signed);
        let response = self.client.execute(request).await.map_err(|_| {
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

fn api_default_headers() -> HeaderMap {
    HeaderMap::from_iter([
        (header::ACCEPT, HeaderValue::from_static("application/json")),
        (header::ACCEPT_CHARSET, HeaderValue::from_static("utf-8")),
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        ),
        (header::CONNECTION, HeaderValue::from_static("keep-alive")),
    ])
}

#[derive(Clone, Copy)]
pub(super) enum Scope {
    Account,
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
