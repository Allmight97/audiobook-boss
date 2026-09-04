use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::errors::{AppError, Result};
use crate::remote_source::types::{
    ProviderId, RemoteAcquisitionFailureKind, RemoteRelease, RemoteReleaseProtocol,
    RemoteReleaseSearchRequest, RemoteSourceDiagnostic,
};

pub(super) const PROWLARR_USER_AGENT: &str = "audiobook-boss/indexer";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProwlarrSearchType {
    Search,
    Book,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProwlarrSearchParams {
    pub query: String,
    pub search_type: ProwlarrSearchType,
    pub category_id: u32,
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
    client: Client,
}

impl ReqwestProwlarrAdapter {
    pub(in crate::remote_source) fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(PROWLARR_USER_AGENT)
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .map_err(|error| {
                AppError::General(format!("Failed to configure Indexer HTTP client: {error}"))
            })?;
        Ok(Self { client })
    }

    #[cfg(test)]
    pub(super) fn with_client(client: Client) -> Self {
        Self { client }
    }

    pub(super) async fn search(
        &self,
        base_url: &str,
        api_key: &SecretString,
        params: &ProwlarrSearchParams,
    ) -> Result<ProwlarrSearchOutcome> {
        let url = build_search_url(base_url, params)?;
        let response = self
            .client
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
            .client
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
    ) -> Result<ProwlarrGrabOutcome> {
        let url = join_api_path(base_url, "/api/v1/search")?;
        let body = serde_json::json!({
            "guid": guid,
            "indexerId": indexer_id,
        });
        let response = self
            .client
            .post(url)
            .headers(api_key_headers(api_key))
            .json(&body)
            .send()
            .await
            .map_err(|error| AppError::General(format!("Indexer grab request failed: {error}")))?;

        parse_grab_response(response).await
    }
}

pub(super) fn build_search_params(
    request: &RemoteReleaseSearchRequest,
) -> Result<ProwlarrSearchParams> {
    let author = trim_optional(request.author.as_deref());
    let title = trim_optional(request.title.as_deref());
    let query = trim_optional(request.query.as_deref());

    if author.is_none() && title.is_none() && query.is_none() {
        return Err(AppError::InvalidInput(
            "Provide an author, title, or search query.".to_string(),
        ));
    }

    if author.is_some() || title.is_some() {
        let mut parts = Vec::new();
        if let Some(author) = author {
            parts.push(format!("{{Author:{author}}}"));
        }
        if let Some(title) = title {
            parts.push(format!("{{Title:{title}}}"));
        }
        Ok(ProwlarrSearchParams {
            query: parts.join(" "),
            search_type: ProwlarrSearchType::Book,
            category_id: connection::DEFAULT_CATEGORY_ID,
        })
    } else {
        Ok(ProwlarrSearchParams {
            query: query.expect("validated above"),
            search_type: ProwlarrSearchType::Search,
            category_id: connection::DEFAULT_CATEGORY_ID,
        })
    }
}

pub(super) fn build_search_params_with_category(
    request: &RemoteReleaseSearchRequest,
    category_id: u32,
) -> Result<ProwlarrSearchParams> {
    let mut params = build_search_params(request)?;
    params.category_id = category_id;
    Ok(params)
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
        query.append_pair(
            "type",
            match params.search_type {
                ProwlarrSearchType::Search => "search",
                ProwlarrSearchType::Book => "book",
            },
        );
        query.append_pair("categories", &params.category_id.to_string());
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

    Ok(ProwlarrSystemStatusOutcome {
        ok: true,
        message: version
            .map(|value| format!("Connected to Indexer (version {value})."))
            .unwrap_or_else(|| "Connected to Indexer.".to_string()),
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
        message: "Release sent to Indexer.".to_string(),
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
    })
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
    #[serde(default)]
    size: i64,
    #[serde(default)]
    protocol: Option<String>,
    #[serde(default)]
    seeders: Option<i64>,
}

use super::connection;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::timeout;

    const LOCAL_TIMEOUT: Duration = Duration::from_secs(3);

    #[derive(Default, Clone)]
    struct CapturedRequest {
        method: String,
        path_and_query: String,
        body: String,
    }

    fn local_client(host: &str, addr: SocketAddr) -> Client {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(LOCAL_TIMEOUT)
            .resolve(host, addr)
            .build()
            .expect("client")
    }

    async fn serve_one(listener: &TcpListener, response: &[u8]) -> CapturedRequest {
        let (mut stream, _) = timeout(LOCAL_TIMEOUT, listener.accept())
            .await
            .expect("request should arrive")
            .expect("accept request");
        let mut buffer = vec![0_u8; 8192];
        let bytes = timeout(LOCAL_TIMEOUT, stream.read(&mut buffer))
            .await
            .expect("request bytes should arrive")
            .expect("read request");
        let request = String::from_utf8_lossy(&buffer[..bytes]).to_string();
        timeout(LOCAL_TIMEOUT, stream.write_all(response))
            .await
            .expect("response should write")
            .expect("write response");

        let mut lines = request.lines();
        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path_and_query = parts.next().unwrap_or_default().to_string();
        let body = request
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or_default()
            .to_string();
        CapturedRequest {
            method,
            path_and_query,
            body,
        }
    }

    async fn start_listener() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        (listener, addr)
    }

    #[test]
    fn build_search_params_uses_book_query_for_structured_author_title() {
        let params = build_search_params(&RemoteReleaseSearchRequest {
            author: Some(" Brandon Sanderson ".to_string()),
            title: Some(" The Way of Kings ".to_string()),
            query: None,
        })
        .expect("params");

        assert_eq!(params.search_type, ProwlarrSearchType::Book);
        assert_eq!(
            params.query,
            "{Author:Brandon Sanderson} {Title:The Way of Kings}"
        );
    }

    #[test]
    fn build_search_params_uses_search_type_for_freeform_query() {
        let params = build_search_params(&RemoteReleaseSearchRequest {
            author: None,
            title: None,
            query: Some("way of kings".to_string()),
        })
        .expect("params");

        assert_eq!(params.search_type, ProwlarrSearchType::Search);
        assert_eq!(params.query, "way of kings");
    }

    #[tokio::test]
    async fn search_maps_query_and_category_without_indexer_ids() {
        let (listener, addr) = start_listener().await;
        let host = "prowlarr.test";
        let captured = Arc::new(Mutex::new(CapturedRequest::default()));
        let captured_task = Arc::clone(&captured);
        let body = br#"[{"guid":"abc","indexerId":7,"title":"Book Title","indexer":"Example","size":1234,"protocol":"torrent","seeders":12}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let mut response_bytes = response.into_bytes();
        response_bytes.extend_from_slice(body);
        tokio::spawn(async move {
            *captured_task.lock().expect("lock") = serve_one(&listener, &response_bytes).await;
        });

        let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
        let outcome = adapter
            .search(
                &format!("http://{host}/prowlarr"),
                &SecretString::from("secret-key".to_string()),
                &ProwlarrSearchParams {
                    query: "way of kings".to_string(),
                    search_type: ProwlarrSearchType::Search,
                    category_id: 3030,
                },
            )
            .await
            .expect("search");

        let request = captured.lock().expect("lock").clone();
        assert_eq!(request.method, "GET");
        assert!(request
            .path_and_query
            .starts_with("/prowlarr/api/v1/search?"));
        assert!(request.path_and_query.contains("query=way"));
        assert!(
            request.path_and_query.contains("of+kings")
                || request.path_and_query.contains("of%20kings")
        );
        assert!(request.path_and_query.contains("type=search"));
        assert!(request.path_and_query.contains("categories=3030"));
        assert!(!request.path_and_query.contains("indexerIds"));
        assert_eq!(outcome.releases.len(), 1);
        assert_eq!(outcome.releases[0].guid, "abc");
        assert_eq!(outcome.releases[0].protocol, RemoteReleaseProtocol::Torrent);
        assert_eq!(outcome.releases[0].seeders, Some(12));
        assert!(outcome.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn search_returns_auth_diagnostic_for_rejected_api_key() {
        let (listener, addr) = start_listener().await;
        let host = "prowlarr.test";
        let response = b"HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        tokio::spawn(async move {
            serve_one(&listener, response).await;
        });

        let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
        let outcome = adapter
            .search(
                &format!("http://{host}"),
                &SecretString::from("bad-key".to_string()),
                &ProwlarrSearchParams {
                    query: "test".to_string(),
                    search_type: ProwlarrSearchType::Search,
                    category_id: 3030,
                },
            )
            .await
            .expect("search");

        assert!(outcome.releases.is_empty());
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].kind,
            RemoteAcquisitionFailureKind::AuthRequired
        );
    }

    #[tokio::test]
    async fn grab_posts_guid_and_indexer_id() {
        let (listener, addr) = start_listener().await;
        let host = "prowlarr.test";
        let captured = Arc::new(Mutex::new(CapturedRequest::default()));
        let captured_task = Arc::clone(&captured);
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        tokio::spawn(async move {
            *captured_task.lock().expect("lock") = serve_one(&listener, response).await;
        });

        let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
        let outcome = adapter
            .grab(
                &format!("http://{host}"),
                &SecretString::from("secret-key".to_string()),
                "release-guid",
                42,
            )
            .await
            .expect("grab");

        let request = captured.lock().expect("lock").clone();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path_and_query, "/api/v1/search");
        assert_eq!(request.body, "{\"guid\":\"release-guid\",\"indexerId\":42}");
        assert!(outcome.accepted);
    }

    #[tokio::test]
    async fn grab_returns_failure_diagnostic_for_server_error() {
        let (listener, addr) = start_listener().await;
        let host = "prowlarr.test";
        let response = b"HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nerror";
        tokio::spawn(async move {
            serve_one(&listener, response).await;
        });

        let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
        let outcome = adapter
            .grab(
                &format!("http://{host}"),
                &SecretString::from("secret-key".to_string()),
                "release-guid",
                42,
            )
            .await
            .expect("grab");

        assert!(!outcome.accepted);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].kind,
            RemoteAcquisitionFailureKind::ReleaseGrabFailed
        );
    }
}
