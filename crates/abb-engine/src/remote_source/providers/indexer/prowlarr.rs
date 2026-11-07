use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName};
use reqwest::{Client, Method, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use super::api_key::api_key_header_value;
use crate::errors::{AppError, Result};
use crate::remote_source::types::{
    release_key, ProviderId, RemoteAcquisitionFailureKind, RemoteRelease, RemoteReleaseCategory,
    RemoteReleaseProtocol, RemoteReleaseSearchRequest, RemoteSourceDiagnostic,
};

pub(super) const PROWLARR_USER_AGENT: &str = "audiobook-boss/indexer";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const DIAGNOSIS_TIMEOUT: Duration = Duration::from_secs(10);
const LOGGED_MESSAGE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProwlarrSearchParams {
    pub query: String,
    pub category_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProwlarrSearchOutcome {
    pub releases: Vec<RemoteRelease>,
    pub diagnostics: Vec<RemoteSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProwlarrGrabOutcome {
    pub accepted: bool,
    pub unconfirmed: bool,
    pub message: String,
    pub diagnostics: Vec<RemoteSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProwlarrSystemStatusOutcome {
    pub ok: bool,
    pub message: String,
}

pub(in crate::remote_source) struct ReqwestProwlarrAdapter {
    client: std::sync::Mutex<Option<Client>>,
}

impl ReqwestProwlarrAdapter {
    pub(in crate::remote_source) fn new() -> Self {
        Self {
            client: std::sync::Mutex::default(),
        }
    }
    fn client(&self) -> Result<Client> {
        let mut cached = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(client) = &*cached {
            return Ok(client.clone());
        }
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(PROWLARR_USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| {
                AppError::General(format!("Failed to configure Indexer HTTP client: {error}"))
            })?;
        *cached = Some(client.clone());
        Ok(client)
    }

    #[cfg(test)]
    pub(super) fn with_client(client: Client) -> Self {
        Self {
            client: std::sync::Mutex::new(Some(client)),
        }
    }

    pub(super) async fn search(
        &self,
        base_url: &str,
        api_key: &SecretString,
        params: &ProwlarrSearchParams,
    ) -> Result<ProwlarrSearchOutcome> {
        let url = build_search_url(base_url, params)?;
        let response = self
            .client()?
            .get(url)
            .headers(api_key_headers(api_key)?)
            .send()
            .await
            .map_err(|error| {
                AppError::General(format!("Indexer search request failed: {error}"))
            })?;

        parse_search_response(response).await
    }

    pub(super) async fn system_status(
        &self,
        base_url: &str,
        api_key: &SecretString,
    ) -> Result<ProwlarrSystemStatusOutcome> {
        let url = join_api_path(base_url, "/api/v1/system/status")?;
        let response = self
            .client()?
            .get(url)
            .headers(api_key_headers(api_key)?)
            .send()
            .await
            .map_err(|error| {
                AppError::General(format!("Indexer connection test request failed: {error}"))
            })?;

        parse_system_status_response(response).await
    }

    pub(super) async fn grab(
        &self,
        base_url: &str,
        api_key: &SecretString,
        release: &RemoteRelease,
        request_id: &str,
    ) -> Result<ProwlarrGrabOutcome> {
        let url = join_api_path(base_url, "/api/v1/search")?;
        let body = serde_json::json!({
            "guid": release.guid,
            "indexerId": release.indexer_id,
        });
        let sent = self
            .client()?
            .post(url)
            .headers(api_key_headers(api_key)?)
            .json(&body)
            .send()
            .await;
        let response = match sent {
            Ok(response) => response,
            Err(error) if error.is_connect() => {
                log::warn!(
                    "remote_source indexer_grab request_id={} stage=prowlarr_unreachable error={}",
                    request_id,
                    error.without_url()
                );
                return Ok(grab_failed(PROWLARR_NOT_RESPONDING));
            }
            Err(error) => {
                log::warn!(
                    "remote_source indexer_grab request_id={} stage=prowlarr_send_unconfirmed error={}",
                    request_id,
                    error.without_url()
                );
                return Ok(ProwlarrGrabOutcome {
                    unconfirmed: true,
                    ..grab_failed(GRAB_UNCONFIRMED)
                });
            }
        };

        let status = response.status();
        log::info!(
            "remote_source indexer_grab request_id={} stage=prowlarr_response http_status={}",
            request_id,
            status.as_u16()
        );
        let body = match response.text().await {
            Ok(body) => body,
            Err(error) => {
                log::warn!(
                    "remote_source indexer_grab request_id={} stage=prowlarr_answer_unreadable http_status={} error={}",
                    request_id,
                    status.as_u16(),
                    error.without_url()
                );
                return Ok(unreadable_answer(status));
            }
        };

        if rejects_api_key(status) {
            return Ok(ProwlarrGrabOutcome {
                accepted: false,
                unconfirmed: false,
                message: API_KEY_REJECTED.to_string(),
                diagnostics: vec![api_key_rejected_diagnostic()],
            });
        }

        if !status.is_success() {
            match safe_failure_message(&body) {
                Some(message) => log::warn!(
                    "remote_source indexer_grab request_id={} stage=prowlarr_failure http_status={} message={:?}",
                    request_id,
                    status.as_u16(),
                    message
                ),
                None => log::warn!(
                    "remote_source indexer_grab request_id={} stage=prowlarr_failure http_status={}",
                    request_id,
                    status.as_u16()
                ),
            }
            let line = match status_failure_line(status, &indexer_label(release)) {
                Some(line) => line,
                None => {
                    let facts = self
                        .downloader_facts(base_url, api_key, release, request_id)
                        .await;
                    downloader_failure_line(&release.protocol, facts.as_ref())
                }
            };
            return Ok(grab_failed(&line));
        }

        Ok(ProwlarrGrabOutcome {
            accepted: true,
            unconfirmed: false,
            message: "Release sent to downloader.".to_string(),
            diagnostics: Vec::new(),
        })
    }

    async fn downloader_facts(
        &self,
        base_url: &str,
        api_key: &SecretString,
        release: &RemoteRelease,
        request_id: &str,
    ) -> Option<DownloaderFacts> {
        let facts = async {
            let clients: Vec<ProwlarrDownloadClient> = self
                .diagnosis_call(
                    Method::GET,
                    base_url,
                    api_key,
                    "/api/v1/downloadclient",
                    false,
                )
                .await?;
            let tests = if matching_clients(&release.protocol, &clients).is_empty() {
                Vec::new()
            } else {
                self.diagnosis_call(
                    Method::POST,
                    base_url,
                    api_key,
                    "/api/v1/downloadclient/testall",
                    true,
                )
                .await?
            };
            Ok::<_, AppError>(DownloaderFacts { clients, tests })
        }
        .await;
        match facts {
            Ok(facts) => Some(facts),
            Err(error) => {
                log::warn!(
                    "remote_source indexer_grab request_id={request_id} stage=downloader_check_failed error={error}"
                );
                None
            }
        }
    }

    async fn diagnosis_call<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        base_url: &str,
        api_key: &SecretString,
        path: &str,
        bad_request_carries_result: bool,
    ) -> Result<T> {
        let response = self
            .client()?
            .request(method, join_api_path(base_url, path)?)
            .headers(api_key_headers(api_key)?)
            .timeout(DIAGNOSIS_TIMEOUT)
            .send()
            .await
            .map_err(|error| AppError::General(error.without_url().to_string()))?;
        let status = response.status();
        let readable = status.is_success()
            || (bad_request_carries_result && status == StatusCode::BAD_REQUEST);
        if !readable {
            return Err(AppError::General(format!("{path} answered {status}")));
        }
        response
            .json()
            .await
            .map_err(|error| AppError::General(error.without_url().to_string()))
    }
}

pub(super) fn build_search_params(
    request: &RemoteReleaseSearchRequest,
    category_ids: &[u32],
) -> Result<ProwlarrSearchParams> {
    let author = trim_optional(request.author.as_deref());
    let title = trim_optional(request.title.as_deref());
    let query = trim_optional(request.query.as_deref());

    if author.is_none() && title.is_none() && query.is_none() {
        return Err(AppError::InvalidInput(
            "Provide an author, title, or search query.".to_string(),
        ));
    }

    let joined = [author, title, query]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");

    Ok(ProwlarrSearchParams {
        query: joined,
        category_ids: category_ids.to_vec(),
    })
}

fn trim_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_search_url(base_url: &str, params: &ProwlarrSearchParams) -> Result<Url> {
    let mut url = join_api_path(base_url, "/api/v1/search")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("query", &params.query);
        query.append_pair("type", "search");
        for category_id in &params.category_ids {
            query.append_pair("categories", &category_id.to_string());
        }
    }
    Ok(url)
}

fn join_api_path(base_url: &str, path: &str) -> Result<Url> {
    let mut base = Url::parse(base_url).map_err(|_| {
        AppError::InvalidInput("Indexer URL must be a valid http or https URL.".to_string())
    })?;
    base.set_path(&format!("{}{}", base.path().trim_end_matches('/'), path));
    Ok(base)
}

fn api_key_headers(api_key: &SecretString) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-api-key"),
        api_key_header_value(api_key.expose_secret())?,
    );
    Ok(headers)
}

async fn parse_system_status_response(
    response: reqwest::Response,
) -> Result<ProwlarrSystemStatusOutcome> {
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        AppError::General(format!(
            "Indexer connection test response unreadable: {error}"
        ))
    })?;

    if rejects_api_key(status) {
        return Ok(ProwlarrSystemStatusOutcome {
            ok: false,
            message: API_KEY_REJECTED.to_string(),
        });
    }

    if !status.is_success() {
        return Ok(ProwlarrSystemStatusOutcome {
            ok: false,
            message: failure_line("Indexer connection test failed", status, &body),
        });
    }

    let version = serde_json::from_str::<ProwlarrSystemStatusRaw>(&body)
        .ok()
        .and_then(|raw| raw.version.filter(|value| !value.trim().is_empty()));

    Ok(match version {
        Some(version) => ProwlarrSystemStatusOutcome {
            ok: true,
            message: format!("Connected to Indexer (version {version})."),
        },
        None => ProwlarrSystemStatusOutcome {
            ok: false,
            message: "Indexer returned an unexpected system status response. Check the URL and reverse-proxy configuration.".to_string(),
        },
    })
}

async fn parse_search_response(response: reqwest::Response) -> Result<ProwlarrSearchOutcome> {
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        AppError::General(format!("Indexer search response unreadable: {error}"))
    })?;

    if rejects_api_key(status) {
        return Ok(ProwlarrSearchOutcome {
            releases: Vec::new(),
            diagnostics: vec![api_key_rejected_diagnostic()],
        });
    }

    if !status.is_success() {
        return Ok(ProwlarrSearchOutcome {
            releases: Vec::new(),
            diagnostics: vec![RemoteSourceDiagnostic {
                kind: RemoteAcquisitionFailureKind::ReleaseSearchFailed,
                title_id: None,
                message: failure_line("Indexer search failed", status, &body),
            }],
        });
    }

    let raw_releases: Vec<ProwlarrReleaseRaw> = serde_json::from_str(&body).map_err(|error| {
        AppError::General(format!("Indexer search response was invalid JSON: {error}"))
    })?;

    let mut diagnostics = Vec::new();
    let releases = raw_releases
        .into_iter()
        .filter_map(|raw| map_release(raw, &mut diagnostics))
        .collect();

    Ok(ProwlarrSearchOutcome {
        releases,
        diagnostics,
    })
}

fn map_release(
    raw: ProwlarrReleaseRaw,
    diagnostics: &mut Vec<RemoteSourceDiagnostic>,
) -> Option<RemoteRelease> {
    if raw.guid.trim().is_empty() {
        diagnostics.push(RemoteSourceDiagnostic {
            kind: RemoteAcquisitionFailureKind::ReleaseSearchFailed,
            title_id: None,
            message: "Skipped an indexer hit with a missing release identifier.".to_string(),
        });
        return None;
    }

    Some(RemoteRelease {
        detail_url: release_detail_url(raw.info_url.as_deref(), raw.comment_url.as_deref()),
        provider_id: ProviderId::Indexer,
        key: release_key(raw.indexer_id, &raw.guid),
        guid: raw.guid,
        indexer_id: raw.indexer_id,
        title: raw.title,
        indexer: raw.indexer,
        size_bytes: u64::try_from(raw.size.max(0)).unwrap_or(0),
        protocol: map_protocol(raw.protocol.as_deref()),
        seeders: raw
            .seeders
            .filter(|value| *value >= 0)
            .map(|value| value as u32),
        categories: map_categories(raw.categories),
    })
}

fn release_detail_url(info_url: Option<&str>, comment_url: Option<&str>) -> Option<String> {
    [info_url, comment_url]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            let url = Url::parse(candidate).ok()?;
            (matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none())
            .then(|| url.to_string())
        })
}

fn map_categories(entries: Vec<ProwlarrCategoryEntry>) -> Vec<RemoteReleaseCategory> {
    let mut categories: Vec<RemoteReleaseCategory> = entries
        .into_iter()
        .filter_map(|entry| {
            let (id, name) = match entry {
                ProwlarrCategoryEntry::Object { id, name } => (id, name),
                ProwlarrCategoryEntry::Id(id) => (id, None),
            };
            if id == 0 {
                return None;
            }
            Some(RemoteReleaseCategory {
                id,
                name: category_display_name(id, name.as_deref())?,
            })
        })
        .collect();
    categories.sort_by_key(|category| category.id);
    categories.dedup_by(|left, right| left.id == right.id);
    categories
}

fn category_display_name(id: u32, name: Option<&str>) -> Option<String> {
    let trimmed = name.map(str::trim).filter(|value| !value.is_empty());
    if let Some(name) = trimmed {
        return Some(name.to_string());
    }
    match id {
        3030 => Some("Audio/Audiobook".to_string()),
        3000 => Some("Audio".to_string()),
        _ => None,
    }
}

fn map_protocol(protocol: Option<&str>) -> RemoteReleaseProtocol {
    match protocol.unwrap_or("").to_ascii_lowercase().as_str() {
        "usenet" => RemoteReleaseProtocol::Usenet,
        "torrent" => RemoteReleaseProtocol::Torrent,
        _ => RemoteReleaseProtocol::Unknown,
    }
}

const API_KEY_REJECTED: &str = "Indexer rejected the API key.";

fn rejects_api_key(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

fn api_key_rejected_diagnostic() -> RemoteSourceDiagnostic {
    RemoteSourceDiagnostic {
        kind: RemoteAcquisitionFailureKind::AuthRequired,
        title_id: None,
        message: API_KEY_REJECTED.to_string(),
    }
}

fn failure_line(what: &str, status: StatusCode, body: &str) -> String {
    match safe_failure_message(body) {
        Some(message) => format!("{what} with status {status}: {message}"),
        None => format!("{what} with status {status}"),
    }
}

const GRAB_UNCONFIRMED: &str = "Prowlarr did not answer in time, so this release may already be sent. Check your downloader before you Retry.";

const ANSWER_UNREADABLE: &str = "Prowlarr's answer could not be read. Details are in the log.";

const ANSWER_UNREADABLE_MAY_BE_SENT: &str = "Prowlarr's answer could not be read, so this release may already be sent. Check your downloader before you Retry.";

const PROWLARR_NOT_RESPONDING: &str =
    "Prowlarr is not responding. Check that it is running, then Retry.";

fn unreadable_answer(status: StatusCode) -> ProwlarrGrabOutcome {
    if status.is_success() {
        ProwlarrGrabOutcome {
            unconfirmed: true,
            ..grab_failed(ANSWER_UNREADABLE_MAY_BE_SENT)
        }
    } else {
        grab_failed(ANSWER_UNREADABLE)
    }
}

fn grab_failed(line: &str) -> ProwlarrGrabOutcome {
    ProwlarrGrabOutcome {
        accepted: false,
        unconfirmed: false,
        message: line.to_string(),
        diagnostics: vec![RemoteSourceDiagnostic {
            kind: RemoteAcquisitionFailureKind::ReleaseGrabFailed,
            title_id: None,
            message: line.to_string(),
        }],
    }
}

fn indexer_label(release: &RemoteRelease) -> String {
    let name = release.indexer.trim();
    if name.is_empty() {
        format!("Indexer {}", release.indexer_id)
    } else {
        name.to_string()
    }
}

fn status_failure_line(status: StatusCode, indexer: &str) -> Option<String> {
    match status {
        StatusCode::NOT_FOUND => Some("This search result has expired. Search again.".to_string()),
        StatusCode::CONFLICT => Some(format!(
            "{indexer} did not provide this release. Retry, or pick another release."
        )),
        _ => None,
    }
}

fn matching_clients<'a>(
    protocol: &RemoteReleaseProtocol,
    clients: &'a [ProwlarrDownloadClient],
) -> Vec<&'a ProwlarrDownloadClient> {
    let mut matching: Vec<_> = clients
        .iter()
        .filter(|client| {
            client.enable
                && match protocol {
                    RemoteReleaseProtocol::Torrent => {
                        client.protocol.eq_ignore_ascii_case("torrent")
                    }
                    RemoteReleaseProtocol::Usenet => client.protocol.eq_ignore_ascii_case("usenet"),
                    RemoteReleaseProtocol::Unknown => true,
                }
        })
        .collect();
    matching.sort_by_key(|client| client.priority);
    matching
}

fn downloader_failure_line(
    protocol: &RemoteReleaseProtocol,
    facts: Option<&DownloaderFacts>,
) -> String {
    let Some(facts) = facts else {
        return "Prowlarr could not send this release. Details are in the log.".to_string();
    };
    let clients = matching_clients(protocol, &facts.clients);
    let Some(first) = clients.first() else {
        return match protocol {
            RemoteReleaseProtocol::Torrent => "Prowlarr has no torrent downloader set up.",
            RemoteReleaseProtocol::Usenet => "Prowlarr has no usenet downloader set up.",
            RemoteReleaseProtocol::Unknown => "Prowlarr has no downloader set up.",
        }
        .to_string();
    };
    let failing = clients
        .iter()
        .take_while(|client| client.priority == first.priority)
        .find(|client| {
            facts
                .tests
                .iter()
                .any(|test| test.id == client.id && !test.is_valid)
        });
    match failing {
        Some(failing) => format!(
            "{} is not responding. Check that it is running, then Retry.",
            failing.name
        ),
        None => format!(
            "Prowlarr could not send this release, but {} is responding now. Retry.",
            first.name
        ),
    }
}

fn safe_failure_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let message = value.get("message")?.as_str()?;
    let redacted = message
        .split_whitespace()
        .map(|word| if looks_like_url(word) { "<url>" } else { word })
        .collect::<Vec<_>>()
        .join(" ");
    Some(truncated(&redacted, LOGGED_MESSAGE_CHARS))
}

fn looks_like_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    lower.contains("://")
        || lower.starts_with("www.")
        || lower.starts_with("magnet:")
        || lower
            .split_once('?')
            .is_some_and(|(_, query)| query.contains('='))
}

fn truncated(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[derive(Debug, Deserialize)]
struct ProwlarrDownloadClient {
    id: i64,
    name: String,
    enable: bool,
    protocol: String,
    priority: i64,
}

#[derive(Debug, Deserialize)]
struct ProwlarrClientTest {
    id: i64,
    #[serde(rename = "isValid")]
    is_valid: bool,
}

struct DownloaderFacts {
    clients: Vec<ProwlarrDownloadClient>,
    tests: Vec<ProwlarrClientTest>,
}

#[derive(Debug, Deserialize)]
struct ProwlarrSystemStatusRaw {
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProwlarrReleaseRaw {
    #[serde(rename = "guid")]
    guid: String,
    #[serde(rename = "indexerId", default)]
    indexer_id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    indexer: String,
    #[serde(rename = "infoUrl", default)]
    info_url: Option<String>,
    #[serde(rename = "commentUrl", default)]
    comment_url: Option<String>,
    #[serde(default)]
    size: i64,
    #[serde(default)]
    protocol: Option<String>,
    #[serde(default)]
    seeders: Option<i64>,
    #[serde(default)]
    categories: Vec<ProwlarrCategoryEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ProwlarrCategoryEntry {
    Object {
        id: u32,
        #[serde(default)]
        name: Option<String>,
    },
    Id(u32),
}

#[cfg(test)]
#[path = "prowlarr_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prowlarr_answer_tests.rs"]
mod answer_tests;

#[cfg(test)]
#[path = "prowlarr_failure_tests.rs"]
mod failure_tests;
