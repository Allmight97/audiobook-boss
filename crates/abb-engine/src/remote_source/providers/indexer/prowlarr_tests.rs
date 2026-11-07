use super::*;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

// Bounds a hang only: building the production client loads the system
// root certificates, which can take seconds while the whole suite runs.
const LOCAL_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default, Clone)]
pub(super) struct CapturedRequest {
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

pub(super) async fn serve_one(listener: &TcpListener, response: &[u8]) -> CapturedRequest {
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

pub(super) async fn start_listener() -> (TcpListener, SocketAddr) {
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
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by production_client_does_not_forward_api_keys_on_redirect"
    )]
    let origin_server = tokio::spawn(async move { serve_one(&origin, response.as_bytes()).await });
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by production_client_does_not_forward_api_keys_on_redirect"
    )]
    let mut destination_server = tokio::spawn(async move {
        let body = r#"{"version":"1.2.3"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        serve_one(&destination, response.as_bytes()).await
    });
    let adapter = ReqwestProwlarrAdapter::new();
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
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by system_status_requires_valid_service_response"
        )]
        let server = tokio::spawn(async move { serve_one(&listener, response.as_bytes()).await });
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
    let raw: ProwlarrReleaseRaw = serde_json::from_str(r#"{"guid":"abc"}"#).expect("absent links");
    assert_eq!(
        map_release(raw, &mut Vec::new())
            .expect("release")
            .detail_url,
        None
    );
}

#[test]
fn release_categories_decode_bare_ids_and_named_objects() {
    let raw: Vec<ProwlarrReleaseRaw> = serde_json::from_str(
        r#"[{"guid":"g","categories":[3000,{"id":3030,"name":"Audio/Audiobook"},3030,100047,{"id":100043},{"id":100044,"name":"Audiobooks"},0]}]"#,
    )
    .expect("mixed categories decode");

    let categories = map_categories(raw.into_iter().next().expect("release").categories);

    let category = |id, name: &str| RemoteReleaseCategory {
        id,
        name: name.to_string(),
    };
    assert_eq!(
        categories,
        vec![
            category(3000, "Audio"),
            category(3030, "Audio/Audiobook"),
            category(100044, "Audiobooks"),
        ]
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
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by search_maps_query_and_category_without_indexer_ids"
    )]
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
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by search_returns_auth_diagnostic_for_rejected_api_key"
    )]
    let _server = tokio::spawn(async move {
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
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by grab_posts_guid_and_indexer_id"
    )]
    let server = tokio::spawn(async move { serve_one(&listener, response).await });

    let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
    let outcome = adapter
        .grab(
            &format!("http://{host}"),
            &SecretString::from("secret-key".to_string()),
            &release("Example", RemoteReleaseProtocol::Torrent),
            "test-grab",
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

async fn serve_in_order(listener: &TcpListener, responses: &[Vec<u8>]) -> Vec<CapturedRequest> {
    let mut requests = Vec::new();
    for response in responses {
        requests.push(serve_one(listener, response).await);
    }
    requests
}

fn http_response(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

async fn no_further_request(listener: &TcpListener) -> bool {
    timeout(Duration::from_millis(150), listener.accept())
        .await
        .is_err()
}

pub(super) fn release(indexer: &str, protocol: RemoteReleaseProtocol) -> RemoteRelease {
    RemoteRelease {
        provider_id: ProviderId::Indexer,
        key: release_key(42, "release-guid"),
        guid: "release-guid".to_string(),
        indexer_id: 42,
        title: "Book".to_string(),
        indexer: indexer.to_string(),
        detail_url: None,
        size_bytes: 1,
        protocol,
        seeders: None,
        categories: Vec::new(),
    }
}

fn downloader_facts(clients: &str, tests: &str) -> DownloaderFacts {
    DownloaderFacts {
        clients: serde_json::from_str(clients).expect("clients fixture"),
        tests: serde_json::from_str(tests).expect("tests fixture"),
    }
}

const TORRENT_CLIENT_LIST: &str = r#"[
    {"id":1,"name":"qBittorrent","implementation":"QBittorrent","enable":true,"protocol":"torrent","priority":1},
    {"id":2,"name":"SABnzbd","implementation":"Sabnzbd","enable":true,"protocol":"usenet","priority":1}
]"#;

#[test]
fn grab_status_decides_the_line_before_any_downloader_check() {
    for (status, expected) in [
        (
            StatusCode::NOT_FOUND,
            Some("This search result has expired. Search again."),
        ),
        (
            StatusCode::CONFLICT,
            Some("Example did not provide this release. Retry, or pick another release."),
        ),
        (StatusCode::INTERNAL_SERVER_ERROR, None),
        (StatusCode::BAD_GATEWAY, None),
        (StatusCode::BAD_REQUEST, None),
    ] {
        assert_eq!(
            status_failure_line(status, "Example").as_deref(),
            expected,
            "{status}"
        );
    }
}

#[test]
fn a_release_with_no_indexer_name_is_named_by_its_indexer_id() {
    let torrent = RemoteReleaseProtocol::Torrent;
    assert_eq!(indexer_label(&release("  ", torrent.clone())), "Indexer 42");
    assert_eq!(indexer_label(&release(" Example ", torrent)), "Example");
}

#[test]
fn the_downloader_line_names_the_side_that_is_down() {
    let torrent = RemoteReleaseProtocol::Torrent;
    let two_torrent = r#"[
        {"id":1,"name":"Second","enable":true,"protocol":"torrent","priority":2},
        {"id":2,"name":"First","enable":true,"protocol":"torrent","priority":1},
        {"id":3,"name":"Off","enable":false,"protocol":"torrent","priority":0}
    ]"#;
    let pass = |id: i64| format!(r#"{{"id":{id},"isValid":true,"validationFailures":[]}}"#);
    let fail = |id: i64| {
        format!(
            r#"{{"id":{id},"isValid":false,"validationFailures":[{{"errorMessage":"Unable to connect"}}]}}"#
        )
    };
    let tied = r#"[
        {"id":1,"name":"Second","enable":true,"protocol":"torrent","priority":1},
        {"id":2,"name":"First","enable":true,"protocol":"torrent","priority":1}
    ]"#;
    let usenet_only =
        r#"[{"id":2,"name":"SABnzbd","enable":true,"protocol":"usenet","priority":1}]"#;
    let torrent_off = r#"[{"id":3,"name":"Off","enable":false,"protocol":"torrent","priority":1}]"#;

    for (protocol, clients, tests, expected) in [
        (
            &torrent,
            TORRENT_CLIENT_LIST,
            format!("[{}]", fail(1)),
            "qBittorrent is not responding. Check that it is running, then Retry.",
        ),
        (
            &torrent,
            TORRENT_CLIENT_LIST,
            format!("[{},{}]", pass(1), fail(2)),
            "Prowlarr could not send this release, but qBittorrent is responding now. Retry.",
        ),
        (
            &RemoteReleaseProtocol::Usenet,
            TORRENT_CLIENT_LIST,
            format!("[{},{}]", fail(1), fail(2)),
            "SABnzbd is not responding. Check that it is running, then Retry.",
        ),
        (
            &torrent,
            two_torrent,
            format!("[{},{}]", fail(1), fail(2)),
            "First is not responding. Check that it is running, then Retry.",
        ),
        (
            &torrent,
            two_torrent,
            format!("[{},{}]", fail(1), pass(2)),
            "Prowlarr could not send this release, but First is responding now. Retry.",
        ),
        (
            &torrent,
            two_torrent,
            format!("[{},{}]", pass(1), pass(2)),
            "Prowlarr could not send this release, but First is responding now. Retry.",
        ),
        (
            &torrent,
            tied,
            format!("[{},{}]", pass(2), fail(1)),
            "Second is not responding. Check that it is running, then Retry.",
        ),
        (
            &torrent,
            usenet_only,
            "[]".to_string(),
            "Prowlarr has no torrent downloader set up.",
        ),
        (
            &torrent,
            torrent_off,
            "[]".to_string(),
            "Prowlarr has no torrent downloader set up.",
        ),
        (
            &RemoteReleaseProtocol::Usenet,
            "[]",
            "[]".to_string(),
            "Prowlarr has no usenet downloader set up.",
        ),
        (
            &RemoteReleaseProtocol::Unknown,
            "[]",
            "[]".to_string(),
            "Prowlarr has no downloader set up.",
        ),
    ] {
        let facts = downloader_facts(clients, &tests);
        assert_eq!(
            downloader_failure_line(protocol, Some(&facts)),
            expected,
            "{protocol:?} {clients} {tests}"
        );
    }

    assert_eq!(
        downloader_failure_line(&torrent, None),
        "Prowlarr could not send this release. Details are in the log."
    );
}

#[tokio::test]
async fn a_server_error_grab_asks_prowlarr_which_downloader_is_down() {
    let (listener, addr) = start_listener().await;
    let host = "prowlarr.test";
    let responses = vec![
        http_response(
            "500 Internal Server Error",
            r#"{ "message": "Unable to connect to qBittorrent", "description": "System.Net.Http.HttpRequestException ..." }"#,
        ),
        http_response("200 OK", TORRENT_CLIENT_LIST),
        http_response(
            "400 Bad Request",
            r#"[{"id":1,"isValid":false,"validationFailures":[{"errorMessage":"Unable to connect"}]}]"#,
        ),
    ];
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by a_server_error_grab_asks_prowlarr_which_downloader_is_down"
    )]
    let server = tokio::spawn(async move { serve_in_order(&listener, &responses).await });

    let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
    let outcome = adapter
        .grab(
            &format!("http://{host}"),
            &SecretString::from("secret-key".to_string()),
            &release("Example", RemoteReleaseProtocol::Torrent),
            "test-grab",
        )
        .await
        .expect("grab");

    let requests = server.await.expect("server");
    let seen: Vec<_> = requests
        .iter()
        .map(|request| (request.method.as_str(), request.path_and_query.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            ("POST", "/api/v1/search"),
            ("GET", "/api/v1/downloadclient"),
            ("POST", "/api/v1/downloadclient/testall"),
        ]
    );
    assert!(requests
        .iter()
        .all(|request| request.api_key.as_deref() == Some("secret-key")));
    assert!(!outcome.accepted);
    let line = "qBittorrent is not responding. Check that it is running, then Retry.";
    assert_eq!(outcome.message, line);
    assert_eq!(outcome.diagnostics.len(), 1);
    assert_eq!(
        outcome.diagnostics[0].kind,
        RemoteAcquisitionFailureKind::ReleaseGrabFailed
    );
    assert_eq!(outcome.diagnostics[0].message, line);
}

#[tokio::test]
async fn a_grab_that_cannot_ask_prowlarr_about_its_downloaders_points_to_the_log() {
    let (listener, addr) = start_listener().await;
    let host = "prowlarr.test";
    let responses = vec![
        http_response("500 Internal Server Error", "{}"),
        http_response("503 Service Unavailable", "{}"),
    ];
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by a_grab_that_cannot_ask_prowlarr_about_its_downloaders_points_to_the_log"
    )]
    let server = tokio::spawn(async move { serve_in_order(&listener, &responses).await });

    let adapter = ReqwestProwlarrAdapter::with_client(local_client(host, addr));
    let outcome = adapter
        .grab(
            &format!("http://{host}"),
            &SecretString::from("secret-key".to_string()),
            &release("Example", RemoteReleaseProtocol::Torrent),
            "test-grab",
        )
        .await
        .expect("grab");

    server.await.expect("server");
    assert!(!outcome.accepted);
    assert_eq!(
        outcome.message,
        "Prowlarr could not send this release. Details are in the log."
    );
}

#[tokio::test]
async fn grab_failures_that_name_their_own_side_ask_nothing_more() {
    for (status, expected) in [
        (
            "404 Not Found",
            "This search result has expired. Search again.",
        ),
        (
            "409 Conflict",
            "Example did not provide this release. Retry, or pick another release.",
        ),
        ("401 Unauthorized", "Indexer rejected the API key."),
        ("403 Forbidden", "Indexer rejected the API key."),
    ] {
        let (listener, addr) = start_listener().await;
        let response = http_response(status, r#"{"message":"raw"}"#);
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by grab_failures_that_name_their_own_side_ask_nothing_more"
        )]
        let server = tokio::spawn(async move {
            serve_one(&listener, &response).await;
            no_further_request(&listener).await
        });

        let adapter = ReqwestProwlarrAdapter::with_client(local_client("prowlarr.test", addr));
        let outcome = adapter
            .grab(
                "http://prowlarr.test",
                &SecretString::from("secret-key".to_string()),
                &release("Example", RemoteReleaseProtocol::Torrent),
                "test-grab",
            )
            .await
            .expect("grab");

        assert!(
            server.await.expect("server"),
            "{status} must not trigger a downloader check"
        );
        assert!(!outcome.accepted);
        assert_eq!(outcome.message, expected, "{status}");
    }
}

#[tokio::test]
async fn an_accepted_grab_never_asks_about_downloaders() {
    let (listener, addr) = start_listener().await;
    let response = http_response("200 OK", "{}");
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by an_accepted_grab_never_asks_about_downloaders"
    )]
    let server = tokio::spawn(async move {
        serve_one(&listener, &response).await;
        no_further_request(&listener).await
    });

    let adapter = ReqwestProwlarrAdapter::with_client(local_client("prowlarr.test", addr));
    let outcome = adapter
        .grab(
            "http://prowlarr.test",
            &SecretString::from("secret-key".to_string()),
            &release("Example", RemoteReleaseProtocol::Torrent),
            "test-grab",
        )
        .await
        .expect("grab");

    assert!(server.await.expect("server"), "no second request");
    assert!(outcome.accepted);
    assert_eq!(outcome.message, "Release sent to downloader.");
}

#[tokio::test]
async fn a_grab_that_cannot_connect_to_prowlarr_says_it_is_not_responding() {
    let (listener, addr) = start_listener().await;
    drop(listener);

    let adapter = ReqwestProwlarrAdapter::with_client(local_client("prowlarr.test", addr));
    let outcome = adapter
        .grab(
            "http://prowlarr.test",
            &SecretString::from("secret-key".to_string()),
            &release("Example", RemoteReleaseProtocol::Torrent),
            "test-grab",
        )
        .await
        .expect("grab");

    assert!(!outcome.accepted);
    assert_eq!(
        outcome.message,
        "Prowlarr is not responding. Check that it is running, then Retry."
    );
    assert_eq!(
        outcome.diagnostics[0].kind,
        RemoteAcquisitionFailureKind::ReleaseGrabFailed
    );
}

#[tokio::test]
async fn a_stored_api_key_that_cannot_be_sent_fails_every_request_without_sending_it() {
    let adapter = ReqwestProwlarrAdapter::with_client(local_client(
        "prowlarr.test",
        "127.0.0.1:9".parse().expect("address"),
    ));
    let base_url = "http://prowlarr.test";
    let key = SecretString::from("bad\nkey".to_string());
    let params = ProwlarrSearchParams {
        query: "test".to_string(),
        category_ids: vec![3030],
    };

    let errors = [
        adapter.search(base_url, &key, &params).await.err(),
        adapter.system_status(base_url, &key).await.err(),
        adapter
            .grab(
                base_url,
                &key,
                &release("Example", RemoteReleaseProtocol::Torrent),
                "request",
            )
            .await
            .err(),
    ];

    for error in errors {
        assert!(
            matches!(error, Some(AppError::InvalidInput(ref message)) if message.contains("API key")),
            "{error:?}"
        );
    }
}

#[test]
fn a_failed_grab_logs_only_a_capped_url_free_message() {
    let long = "x".repeat(300);
    let long_body = format!(r#"{{"message":"{long}"}}"#);
    let cases: [(&str, Option<String>); 9] = [
        (
            r#"{"message":"Unable to connect to qBittorrent","description":"System.Net.Http at http://tracker.example/dl?passkey=SECRET"}"#,
            Some("Unable to connect to qBittorrent".to_string()),
        ),
        (
            r#"{"message":"Download failed for https://tracker.example/download/7?passkey=SECRET123 (403)"}"#,
            Some("Download failed for <url> (403)".to_string()),
        ),
        (
            r#"{"message":"Bad link (http://tracker.example/a?passkey=SECRET123) and HTTPS://t.example/b?k=SECRET456"}"#,
            Some("Bad link <url> and <url>".to_string()),
        ),
        (
            r#"{"message":"No route to www.tracker.example/dl?passkey=SECRET123 or magnet:?xt=urn:btih:SECRET789"}"#,
            Some("No route to <url> or <url>".to_string()),
        ),
        (&long_body, Some("x".repeat(200))),
        (
            r#"{"description":"http://tracker.example/dl?passkey=SECRET"}"#,
            None,
        ),
        (r#"{"message":42}"#, None),
        (
            "<html>Bad gateway http://tracker.example/?passkey=SECRET</html>",
            None,
        ),
        ("", None),
    ];
    for (body, expected) in cases {
        let logged = failure_message_for_log(body);
        assert_eq!(logged, expected, "{body}");
        assert!(
            logged
                .as_deref()
                .is_none_or(|text| !text.contains("SECRET")),
            "{body}"
        );
    }
}

#[tokio::test]
async fn a_grab_that_times_out_may_already_be_sent_so_it_is_unconfirmed() {
    let (listener, addr) = start_listener().await;
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by a_grab_that_times_out_may_already_be_sent_so_it_is_unconfirmed"
    )]
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept request");
        tokio::time::sleep(Duration::from_millis(600)).await;
        drop(stream);
    });
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(200))
        .resolve("prowlarr.test", addr)
        .build()
        .expect("client");

    let outcome = ReqwestProwlarrAdapter::with_client(client)
        .grab(
            "http://prowlarr.test",
            &SecretString::from("secret-key".to_string()),
            &release("Example", RemoteReleaseProtocol::Torrent),
            "test-grab",
        )
        .await
        .expect("a timeout is an outcome");

    server.await.expect("server");
    assert!(!outcome.accepted);
    assert!(outcome.unconfirmed);
    assert_eq!(
        outcome.diagnostics[0].message,
        "Prowlarr did not answer in time, so this release may already be sent. Check your downloader before you Retry."
    );
}
