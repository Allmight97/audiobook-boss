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
    client: Client,
}

impl ReqwestProwlarrAdapter {
    pub(in crate::remote_source) fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(PROWLARR_USER_AGENT)
            // X-Api-Key belongs only to the configured server.
            .redirect(reqwest::redirect::Policy::none())
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
        category_ids: if category_ids.is_empty() {
            connection::DEFAULT_CATEGORY_IDS.to_vec()
        } else {
            category_ids.to_vec()
        },
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

use super::connection;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
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
        api_key: Option<String>,
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
        let request = timeout(LOCAL_TIMEOUT, async {
            let mut buffer = Vec::new();
            loop {
                let mut chunk = [0_u8; 2048];
                let count = stream.read(&mut chunk).await.expect("read request");
                assert!(count > 0, "request ended early");
                buffer.extend_from_slice(&chunk[..count]);
                let text = String::from_utf8_lossy(&buffer);
                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().expect("content length"))
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break text.into_owned();
                    }
                }
                assert!(buffer.len() < 16384, "unexpectedly large test request");
            }
        })
        .await
        .expect("request bytes should arrive");
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
        let api_key = request.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("x-api-key")
                .then(|| value.trim().to_string())
        });
        CapturedRequest {
            api_key,
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

    #[tokio::test]
    async fn production_client_does_not_forward_api_keys_on_redirect() {
        let (origin, origin_addr) = start_listener().await;
        let (destination, destination_addr) = start_listener().await;
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{destination_addr}/redirected\r\nContent-Length: 0\r\n\r\n"
        );
        let origin_server =
            tokio::spawn(async move { serve_one(&origin, response.as_bytes()).await });
        let mut destination_server = tokio::spawn(async move {
            let body = r#"{"version":"1.2.3"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            serve_one(&destination, response.as_bytes()).await
        });
        let adapter = ReqwestProwlarrAdapter::new().expect("production client");
        let result = timeout(
            LOCAL_TIMEOUT,
            adapter.system_status(
                &format!("http://{origin_addr}"),
                &SecretString::from("origin-only-key".to_string()),
            ),
        )
        .await
        .expect("connection test should finish")
        .expect("connection test response");
        let request = origin_server.await.expect("origin server");
        assert_eq!(request.api_key.as_deref(), Some("origin-only-key"));
        let followed = timeout(Duration::from_millis(30), &mut destination_server)
            .await
            .is_ok();
        destination_server.abort();
        assert!(
            !followed,
            "redirect destination must never receive the origin API key"
        );
        assert!(!result.ok, "redirect must not count as a valid connection");
    }

    #[tokio::test]
    async fn system_status_requires_valid_service_response() {
        for (status, body, expected) in [
            ("200 OK", r#"{"version":"1.2.3"}"#, true),
            ("200 OK", "<html>Login</html>", false),
            ("200 OK", "{}", false),
            ("200 OK", r#"{"version":" "}"#, false),
            ("401 Unauthorized", "{}", false),
            ("500 Internal Server Error", "error", false),
        ] {
            let (listener, addr) = start_listener().await;
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let server =
                tokio::spawn(async move { serve_one(&listener, response.as_bytes()).await });
            let adapter = ReqwestProwlarrAdapter::with_client(local_client("indexer.test", addr));
            let result = adapter
                .system_status(
                    "http://indexer.test/proxy",
                    &SecretString::from("key".to_string()),
                )
                .await
                .expect("status");
            let request = server.await.expect("server");
            assert_eq!(request.api_key.as_deref(), Some("key"));
            assert_eq!(request.method, "GET");
            assert_eq!(request.path_and_query, "/proxy/api/v1/system/status");
            assert_eq!(result.ok, expected, "status={status}, body={body}");
        }
    }

    #[test]
    fn search_params_accept_optional_fields_and_reject_empty_input() {
        for (author, title, query, expected) in [
            (
                Some(" Brandon Sanderson "),
                Some(" The Way of Kings "),
                None,
                "Brandon Sanderson The Way of Kings",
            ),
            (Some("David Crouse"), None, None, "David Crouse"),
            (None, Some("Way of Kings"), None, "Way of Kings"),
            (None, None, Some(" way of kings "), "way of kings"),
        ] {
            let params = build_search_params(
                &RemoteReleaseSearchRequest {
                    author: author.map(str::to_string),
                    title: title.map(str::to_string),
                    query: query.map(str::to_string),
                },
                &[3000, 3030],
            )
            .expect("params");
            assert_eq!(params.query, expected);
            assert_eq!(params.category_ids, [3000, 3030]);
        }
        assert!(build_search_params(
            &RemoteReleaseSearchRequest {
                author: Some(" ".to_string()),
                title: None,
                query: None,
            },
            &[]
        )
        .is_err());
    }

    #[test]
    fn release_details_use_only_valid_source_links() {
        for (info, comment, expected) in [
            (None, None, None),
            (
                Some("https://source.test/book"),
                Some("https://source.test/comments"),
                Some("https://source.test/book"),
            ),
            (
                None,
                Some("http://source.test/comments"),
                Some("http://source.test/comments"),
            ),
            (
                Some("/relative"),
                Some("https://source.test/comments"),
                Some("https://source.test/comments"),
            ),
            (Some("javascript:alert(1)"), None, None),
            (
                Some("https://user:secret@source.test/book"),
                Some("https://source.test/comments"),
                Some("https://source.test/comments"),
            ),
            (
                Some("https://user@source.test/book"),
                Some("file:///tmp/book"),
                None,
            ),
        ] {
            let raw: ProwlarrReleaseRaw = serde_json::from_value(serde_json::json!({
                "guid": "https://source.test/not-a-detail-link",
                "infoUrl": info,
                "commentUrl": comment,
            }))
            .expect("release fixture");
            let mut diagnostics = Vec::new();
            let release = map_release(raw, &mut diagnostics).expect("release remains selectable");
            assert_eq!(release.detail_url.as_deref(), expected);
            assert!(diagnostics.is_empty());
        }
        let raw: ProwlarrReleaseRaw =
            serde_json::from_str(r#"{"guid":"abc"}"#).expect("absent links");
        assert_eq!(
            map_release(raw, &mut Vec::new())
                .expect("release")
                .detail_url,
            None
        );
    }

    #[test]
    fn map_categories_accepts_named_objects_and_bare_ids() {
        let categories = map_categories(vec![
            ProwlarrCategoryEntry::Id(3000),
            ProwlarrCategoryEntry::Object {
                id: 3030,
                name: Some("Audio/Audiobook".to_string()),
            },
            ProwlarrCategoryEntry::Id(3030),
            ProwlarrCategoryEntry::Id(100047),
            ProwlarrCategoryEntry::Object {
                id: 100043,
                name: None,
            },
        ]);

        assert_eq!(
            categories,
            vec![
                RemoteReleaseCategory {
                    id: 3000,
                    name: "Audio".to_string(),
                },
                RemoteReleaseCategory {
                    id: 3030,
                    name: "Audio/Audiobook".to_string(),
                },
            ]
        );
    }

    #[test]
    fn map_categories_keeps_named_indexer_specific_categories() {
        let categories = map_categories(vec![ProwlarrCategoryEntry::Object {
            id: 100047,
            name: Some("Audiobooks".to_string()),
        }]);

        assert_eq!(
            categories,
            vec![RemoteReleaseCategory {
                id: 100047,
                name: "Audiobooks".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn search_maps_query_and_category_without_indexer_ids() {
        let (listener, addr) = start_listener().await;
        let host = "prowlarr.test";
        let body = br#"[{"guid":"abc","indexerId":7,"title":"Book Title","indexer":"Example","size":1234,"protocol":"torrent","seeders":12,"infoUrl":"https://source.test/book/abc","commentUrl":"https://source.test/comments/abc","categories":[{"id":3030,"name":"Audio/Audiobook"}]}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let mut response_bytes = response.into_bytes();
        response_bytes.extend_from_slice(body);
        let server = tokio::spawn(async move { serve_one(&listener, &response_bytes).await });

        let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
        let outcome = adapter
            .search(
                &format!("http://{host}/prowlarr"),
                &SecretString::from("secret-key".to_string()),
                &ProwlarrSearchParams {
                    query: "way of kings".to_string(),
                    category_ids: vec![3000, 3030],
                },
            )
            .await
            .expect("search");

        let request = server.await.expect("server");
        assert_eq!(request.api_key.as_deref(), Some("secret-key"));
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
        assert!(request.path_and_query.contains("categories=3000"));
        assert!(!request.path_and_query.contains("indexerIds"));
        assert_eq!(outcome.releases.len(), 1);
        assert_eq!(outcome.releases[0].guid, "abc");
        assert_eq!(
            outcome.releases[0].detail_url.as_deref(),
            Some("https://source.test/book/abc")
        );
        assert_eq!(outcome.releases[0].indexer, "Example");
        assert_eq!(outcome.releases[0].indexer_id, 7);
        assert_eq!(outcome.releases[0].protocol, RemoteReleaseProtocol::Torrent);
        assert_eq!(outcome.releases[0].seeders, Some(12));
        assert_eq!(outcome.releases[0].categories.len(), 1);
        assert_eq!(outcome.releases[0].categories[0].id, 3030);
        assert_eq!(outcome.releases[0].categories[0].name, "Audio/Audiobook");
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
                    category_ids: vec![3030],
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
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        let server = tokio::spawn(async move { serve_one(&listener, response).await });

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

        let request = server.await.expect("server");
        assert_eq!(request.api_key.as_deref(), Some("secret-key"));
        assert_eq!(request.method, "POST");
        assert_eq!(request.path_and_query, "/api/v1/search");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request.body).expect("grab request JSON"),
            serde_json::json!({"guid":"release-guid", "indexerId":42})
        );
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
