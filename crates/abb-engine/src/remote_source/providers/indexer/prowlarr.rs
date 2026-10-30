use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::errors::{AppError, Result};
use crate::remote_source::types::{
    ProviderId, RemoteAcquisitionFailureKind, RemoteRelease, RemoteReleaseCategory,
    RemoteReleaseProtocol, RemoteReleaseSearchRequest, RemoteSourceDiagnostic,
};

pub(super) const PROWLARR_USER_AGENT: &str = "audiobook-boss/indexer";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

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
    // Session-only hosts must not initialize a network client they never use.
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
            // X-Api-Key belongs only to the configured server.
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
            .headers(api_key_headers(api_key))
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
            .headers(api_key_headers(api_key))
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
        guid: &str,
        indexer_id: i64,
        request_id: &str,
    ) -> Result<ProwlarrGrabOutcome> {
        let url = join_api_path(base_url, "/api/v1/search")?;
        let body = serde_json::json!({
            "guid": guid,
            "indexerId": indexer_id,
        });
        let response = self
            .client()?
            .post(url)
            .headers(api_key_headers(api_key))
            .json(&body)
            .send()
            .await
            .map_err(|error| AppError::General(format!("Indexer grab request failed: {error}")))?;

        log::info!(
            "remote_source indexer_grab request_id={} stage=prowlarr_response http_status={}",
            request_id,
            response.status().as_u16()
        );
        parse_grab_response(response).await
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

fn api_key_headers(api_key: &SecretString) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(name) = HeaderName::from_bytes(b"X-Api-Key") {
        if let Ok(value) = HeaderValue::from_str(api_key.expose_secret()) {
            headers.insert(name, value);
        }
    }
    headers
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

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Ok(ProwlarrSystemStatusOutcome {
            ok: false,
            message: "Indexer rejected the API key.".to_string(),
        });
    }

    if !status.is_success() {
        return Ok(ProwlarrSystemStatusOutcome {
            ok: false,
            message: format!(
                "Indexer connection test failed with status {status}: {}",
                trim_body(&body)
            ),
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

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Ok(ProwlarrSearchOutcome {
            releases: Vec::new(),
            diagnostics: vec![RemoteSourceDiagnostic {
                kind: RemoteAcquisitionFailureKind::AuthRequired,
                title_id: None,
                message: "Indexer rejected the API key.".to_string(),
            }],
        });
    }

    if !status.is_success() {
        return Ok(ProwlarrSearchOutcome {
            releases: Vec::new(),
            diagnostics: vec![search_failed_diagnostic(status, &body)],
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

async fn parse_grab_response(response: reqwest::Response) -> Result<ProwlarrGrabOutcome> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| AppError::General(format!("Indexer grab response unreadable: {error}")))?;

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Ok(ProwlarrGrabOutcome {
            accepted: false,
            message: "Indexer rejected the API key.".to_string(),
            diagnostics: vec![RemoteSourceDiagnostic {
                kind: RemoteAcquisitionFailureKind::AuthRequired,
                title_id: None,
                message: "Indexer rejected the API key.".to_string(),
            }],
        });
    }

    if !status.is_success() {
        return Ok(ProwlarrGrabOutcome {
            accepted: false,
            message: "Indexer did not accept the grab.".to_string(),
            diagnostics: vec![grab_failed_diagnostic(status, &body)],
        });
    }

    Ok(ProwlarrGrabOutcome {
        accepted: true,
        message: "Release sent to downloader.".to_string(),
        diagnostics: Vec::new(),
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

fn search_failed_diagnostic(status: StatusCode, body: &str) -> RemoteSourceDiagnostic {
    RemoteSourceDiagnostic {
        kind: RemoteAcquisitionFailureKind::ReleaseSearchFailed,
        title_id: None,
        message: format!(
            "Indexer search failed with status {status}: {}",
            trim_body(body)
        ),
    }
}

fn grab_failed_diagnostic(status: StatusCode, body: &str) -> RemoteSourceDiagnostic {
    RemoteSourceDiagnostic {
        kind: RemoteAcquisitionFailureKind::ReleaseGrabFailed,
        title_id: None,
        message: format!(
            "Indexer grab failed with status {status}: {}",
            trim_body(body)
        ),
    }
}

fn trim_body(body: &str) -> String {
    body.chars().take(240).collect()
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
