use super::*;
use audible_api::auth::localization;
use audible_api::auth::register::Registration;
use serde_json::json;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

fn test_device_key() -> String {
    static KEY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    KEY.get_or_init(|| {
        use rsa::pkcs1::{EncodeRsaPrivateKey, LineEnding};
        rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 1024)
            .expect("test device key")
            .to_pkcs1_pem(LineEnding::LF)
            .expect("PKCS#1 PEM")
            .to_string()
    })
    .clone()
}

pub(in crate::remote_source::providers::audible) fn signing_auth(country_code: &str) -> Auth {
    Auth {
        locale: localization::find_by_country_code(country_code).expect("locale"),
        device_registration: Registration {
            device_serial: "device-serial".to_string(),
            client_id: "client-id".to_string(),
            adp_token: "adp-token".to_string(),
            device_private_key: test_device_key(),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            expires: 0,
            website_cookies: HashMap::new(),
            store_authentication_cookie: String::new(),
            device_info: json!({}),
            customer_info: json!({ "user_id": "account-1" }),
        },
        authorization_code: "authorization-code".to_string(),
        code_verifier: "code-verifier".to_string(),
    }
}

pub(in crate::remote_source::providers::audible) async fn stub_api(
    replies: Vec<(u16, String)>,
) -> (AudibleApi, JoinHandle<Vec<String>>) {
    stub_http(replies.into_iter().map(|(status, body)| format!(
        "HTTP/1.1 {status} Stub\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )).collect()).await
}

async fn stub_http(replies: Vec<String>) -> (AudibleApi, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind stub");
    let address = listener.local_addr().expect("stub address");
    #[expect(clippy::disallowed_methods, reason = "joined by each stub_http caller")]
    let server = tokio::spawn(async move {
        let mut request_lines = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().await.expect("request arrives");
            request_lines.push(read_request_line(&mut stream).await);
            stream
                .write_all(reply.as_bytes())
                .await
                .expect("write reply");
            let _ = stream.shutdown().await;
        }
        request_lines
    });
    let client = AudibleClient::new(signing_auth("us")).expect("Audible client");
    (
        AudibleApi::with_client(client, format!("http://{address}")),
        server,
    )
}

async fn read_request_line(stream: &mut tokio::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let head_end = loop {
        if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut buffer).await.expect("read request");
        assert!(read > 0, "request ended early");
        request.extend_from_slice(&buffer[..read]);
    };
    let head = String::from_utf8_lossy(&request[..head_end]).to_string();
    let body_length = head
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .and_then(|length| length.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    while request.len() < head_end + body_length {
        let read = stream.read(&mut buffer).await.expect("read body");
        assert!(read > 0, "request body ended early");
        request.extend_from_slice(&buffer[..read]);
    }
    head.lines().next().unwrap_or_default().to_string()
}

fn library_page(first_id: usize, count: usize) -> String {
    json!({
        "items": (first_id..first_id + count)
            .map(|id| json!({ "asin": format!("B{id:09}"), "title": format!("Book {id}") }))
            .collect::<Vec<_>>()
    })
    .to_string()
}

#[test]
fn requests_go_to_the_signed_in_marketplace() {
    let api = AudibleApi::new(signing_auth("uk")).expect("Audible client");

    assert_eq!(api.base_url, "https://api.audible.co.uk");
}

#[tokio::test]
async fn a_refused_sign_in_with_a_json_body_is_not_an_empty_library() {
    let (api, server) = stub_api(vec![(
        401,
        r#"{"message":"Unauthorized","items":[]}"#.to_string(),
    )])
    .await;

    let result = super::super::load_all_library_titles(&api).await;
    server.await.expect("stub");

    assert!(
        matches!(result, Err(AudibleRequestError::AuthRejected)),
        "{result:?}"
    );
}

#[tokio::test]
async fn a_truncated_auth_reply_rejects_sign_in_just_like_a_complete_reply() {
    for status in [401, 403] {
        for length in [2, 100] {
            let (api, server) = stub_http(vec![format!(
                "HTTP/1.1 {status} Stub\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{{}}"
            )])
            .await;

            let result = super::super::load_all_library_titles(&api).await;
            server.await.expect("stub");

            assert!(
                matches!(result, Err(AudibleRequestError::AuthRejected)),
                "status={status} content_length={length}: {result:?}"
            );
        }
    }
}

#[tokio::test]
async fn a_failed_second_page_loads_no_partial_library() {
    let (api, server) = stub_api(vec![
        (
            200,
            library_page(0, super::super::AUDIBLE_LIBRARY_PAGE_SIZE as usize),
        ),
        (500, r#"{"items":[]}"#.to_string()),
    ])
    .await;

    let result = super::super::load_all_library_titles(&api).await;
    let requests = server.await.expect("stub");

    let Err(AudibleRequestError::Failed(error)) = result else {
        panic!("a failed page must fail the scan: {result:?}");
    };
    assert!(
        error.to_string().contains("No partial library was loaded"),
        "{error}"
    );
    assert!(requests[0].starts_with("GET /1.0/library?"), "{requests:?}");
    assert!(requests[1].contains("page=1"), "{requests:?}");
}

fn query_of(request_line: &str) -> HashMap<String, String> {
    let target = request_line.split_whitespace().nth(1).expect("target");
    reqwest::Url::parse(&format!("http://stub{target}"))
        .expect("url")
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

#[tokio::test]
async fn a_library_request_asks_for_availability_and_pdf_fields() {
    let (api, server) = stub_api(vec![(200, library_page(0, 3))]).await;

    let titles = super::super::load_all_library_titles(&api)
        .await
        .expect("library");
    let requests = server.await.expect("stub");

    assert_eq!(titles.len(), 3);
    assert_eq!(requests.len(), 1, "a short first page ends the scan");
    let query = query_of(&requests[0]);
    assert_eq!(
        query["num_results"],
        super::super::AUDIBLE_LIBRARY_PAGE_SIZE.to_string()
    );
    assert_eq!(query["sort_by"], "-PurchaseDate");
    assert!(!query.contains_key("page"));
    for group in ["is_playable", "is_visible", "is_downloaded", "pdf_url"] {
        assert!(
            query["response_groups"].split(',').any(|got| got == group),
            "library request must ask Audible for {group}"
        );
    }
}

#[tokio::test]
async fn a_paged_library_loads_every_title_once_and_stops() {
    const PAGE: usize = super::super::AUDIBLE_LIBRARY_PAGE_SIZE as usize;
    let cases = [
        (
            "overlapping pages",
            vec![
                library_page(0, PAGE),
                library_page(500, PAGE),
                library_page(1500, 10),
            ],
            1510,
        ),
        (
            "explicit page 1 repeating the default page",
            vec![
                library_page(0, PAGE),
                library_page(0, PAGE),
                library_page(PAGE, 5),
            ],
            PAGE + 5,
        ),
        (
            "a full page with nothing new",
            vec![
                library_page(0, PAGE),
                library_page(PAGE, PAGE),
                library_page(PAGE, PAGE),
            ],
            2 * PAGE,
        ),
    ];

    for (name, pages, expected_titles) in cases {
        let (api, server) = stub_api(pages.into_iter().map(|page| (200, page)).collect()).await;

        let titles = super::super::load_all_library_titles(&api)
            .await
            .expect("library");
        let requests = server.await.expect("stub");

        let ids = titles
            .iter()
            .map(|title| title.title_id.as_str())
            .collect::<Vec<_>>();
        let unique = ids.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), ids.len(), "{name}: a title loaded twice");
        assert_eq!(ids.len(), expected_titles, "{name}");
        let pages_asked = requests
            .iter()
            .map(|line| query_of(line).get("page").cloned())
            .collect::<Vec<_>>();
        assert_eq!(
            pages_asked,
            [None, Some("1".to_string()), Some("2".to_string())],
            "{name}"
        );
    }
}
