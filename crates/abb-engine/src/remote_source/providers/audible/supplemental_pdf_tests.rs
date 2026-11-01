use super::*;
use audible_api::auth::{localization, Auth};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

const LOCAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

fn fixture_auth_with_cookies(cookies: &[(&str, &str)], store_cookie: &str) -> Auth {
    let website_cookies = cookies
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<std::collections::HashMap<_, _>>();
    Auth {
        locale: localization::find_by_country_code(super::super::COUNTRY_CODE).expect("locale"),
        device_registration: audible_api::auth::register::Registration {
            device_serial: "device-serial".to_string(),
            client_id: "client-id".to_string(),
            adp_token: "adp-token".to_string(),
            device_private_key: "device-private-key".to_string(),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            expires: 0,
            website_cookies,
            store_authentication_cookie: store_cookie.to_string(),
            device_info: json!({ "device_type": super::super::AUDIBLE_IOS_DEVICE_TYPE }),
            customer_info: json!({ "user_id": "account-1" }),
        },
        authorization_code: "authorization-code".to_string(),
        code_verifier: "code-verifier".to_string(),
    }
}

#[test]
fn companion_file_url_uses_https_www_endpoint_and_encodes_title_id() {
    let url = companion_file_url("B000/unsafe title").expect("url");

    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("www.audible.com"));
    assert_eq!(url.path(), "/companion-file/B000%2Funsafe%20title");
}

fn local_client(addr: SocketAddr, extra_host: Option<&str>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(LOCAL_REQUEST_TIMEOUT)
        .resolve("www.audible.com", addr);
    if let Some(host) = extra_host {
        builder = builder.resolve(host, addr);
    }
    builder.build().expect("client")
}

async fn serve_one(listener: &TcpListener, response: &[u8]) -> String {
    let (mut stream, _) = timeout(LOCAL_REQUEST_TIMEOUT, listener.accept())
        .await
        .expect("request should arrive")
        .expect("accept request");
    let mut buffer = vec![0_u8; 4096];
    let bytes = timeout(LOCAL_REQUEST_TIMEOUT, stream.read(&mut buffer))
        .await
        .expect("request bytes should arrive")
        .expect("read request");
    let request = String::from_utf8_lossy(&buffer[..bytes]).to_string();
    timeout(LOCAL_REQUEST_TIMEOUT, stream.write_all(response))
        .await
        .expect("response should write")
        .expect("write response");
    request
}

#[test]
fn supplemental_pdf_get_request_uses_cookie_auth_only_for_audible_website() {
    let client = reqwest::Client::new();
    let auth =
        fixture_auth_with_cookies(&[("at-main", "cookie-a"), ("sess-at-main", "cookie-b")], "");
    let cookie = supplemental_pdf_cookie_header(&auth).expect("cookie header");
    let request = build_supplemental_pdf_get_request(
        &client,
        reqwest::Url::parse("https://www.audible.com/companion-file/B000000001").expect("url"),
        &cookie,
    )
    .build()
    .expect("request");

    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(
        request
            .headers()
            .get(USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        Some(AUDIBLE_DOWNLOAD_USER_AGENT)
    );
    assert_eq!(
        request
            .headers()
            .get(ACCEPT)
            .and_then(|value| value.to_str().ok()),
        Some("application/pdf,*/*")
    );
    assert_eq!(
        request
            .headers()
            .get(COOKIE)
            .and_then(|value| value.to_str().ok()),
        Some("at-main=cookie-a; sess-at-main=cookie-b")
    );

    let cdn_request = build_supplemental_pdf_get_request(
        &client,
        reqwest::Url::parse("https://cdn.example.test/book.pdf").expect("url"),
        &cookie,
    )
    .build()
    .expect("cdn request");
    assert!(cdn_request.headers().get(COOKIE).is_none());
}

#[test]
fn supplemental_pdf_downloader_requires_cookie_auth() {
    let auth = fixture_auth_with_cookies(&[], "");

    let failure = supplemental_pdf_cookie_header(&auth).expect_err("missing cookies");

    assert_eq!(failure.category, "missing_cookie_auth");
}

#[test]
fn supplemental_pdf_downloader_rejects_cleartext_redirects() {
    let current =
        reqwest::Url::parse("https://www.audible.com/companion-file/B000000001").expect("url");
    let location = HeaderValue::from_static("http://cdn.example.test/book.pdf");

    let failure = supplemental_pdf_redirect_target(
        &current,
        Some(&location),
        SupplementalPdfTransportPolicy::HttpsOnly,
    )
    .expect_err("cleartext redirect");

    assert_eq!(failure.category, "redirect_non_https");
}

#[test]
fn supplemental_pdf_failure_message_omits_provider_details() {
    let failure = SupplementalPdfFailure::new("status", Some(reqwest::StatusCode::FORBIDDEN));
    let message = supplemental_pdf_failure_message(failure);

    assert_eq!(failure.category, "status");
    assert_eq!(failure.status, Some(reqwest::StatusCode::FORBIDDEN));
    assert!(!message.contains("B000000001"));
    assert!(!message.contains("https://"));
}

#[tokio::test]
async fn supplemental_pdf_get_download_succeeds_when_head_would_fail() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let client = local_client(addr, None);
    let pdf_bytes = b"%PDF-1.7\nbody";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
        pdf_bytes.len()
    );
    let response = [response.as_bytes(), pdf_bytes].concat();
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by supplemental_pdf_get_download_succeeds_when_head_would_fail"
    )]
    let server = tokio::spawn(async move { serve_one(&listener, &response).await });
    let auth =
        fixture_auth_with_cookies(&[("at-main", "cookie-a"), ("sess-at-main", "cookie-b")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");

    let (asset, committed) = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            input_id: "input-1",
            file_name: "Being You - A New Science of Consciousness - Supplemental PDF.pdf",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &|| false,
    )
    .await
    .expect("download pdf");
    committed.permanent();
    let request = server.await.expect("server task");

    assert!(
        request.starts_with("GET /companion-file/B000000001 "),
        "unexpected request: {request}"
    );
    assert!(
        !request.starts_with("HEAD "),
        "supplemental PDF downloader must not use HEAD"
    );
    let request_lower = request.to_ascii_lowercase();
    assert!(request_lower.contains("cookie: at-main=cookie-a; sess-at-main=cookie-b"));
    assert_eq!(asset.input_id, "input-1");
    assert_eq!(asset.title_id, "B000000001");
    assert_eq!(
        asset.file_name,
        "Being You - A New Science of Consciousness - Supplemental PDF.pdf"
    );
    assert_eq!(asset.size_bytes, pdf_bytes.len() as u64);
    assert_eq!(asset.sha256, abb_media_core::sha256_hex(pdf_bytes));
    assert_eq!(std::fs::read(&asset.path).expect("read pdf"), pdf_bytes);
}

#[tokio::test]
async fn supplemental_pdf_redirect_does_not_send_cookies_to_non_audible_hosts() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let client = local_client(addr, Some("cdn.example.test"));
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by supplemental_pdf_redirect_does_not_send_cookies_to_non_audible_hosts"
    )]
    let server = tokio::spawn(async move {
        let (mut first_stream, _) = timeout(LOCAL_REQUEST_TIMEOUT, listener.accept())
            .await
            .expect("first request should arrive")
            .expect("accept first");
        let mut first_buffer = vec![0_u8; 4096];
        let first_bytes = timeout(LOCAL_REQUEST_TIMEOUT, first_stream.read(&mut first_buffer))
            .await
            .expect("first request bytes should arrive")
            .expect("read first request");
        let first = String::from_utf8_lossy(&first_buffer[..first_bytes]).to_string();
        let redirect = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://cdn.example.test:{}/book.pdf\r\nContent-Length: 0\r\n\r\n",
            addr.port()
        );
        timeout(
            LOCAL_REQUEST_TIMEOUT,
            first_stream.write_all(redirect.as_bytes()),
        )
        .await
        .expect("redirect should write")
        .expect("write redirect");

        let pdf_bytes = b"%PDF-1.7\nbody";
        let (mut second_stream, _) = timeout(LOCAL_REQUEST_TIMEOUT, listener.accept())
            .await
            .expect("second request should arrive")
            .expect("accept second");
        let mut second_buffer = vec![0_u8; 4096];
        let second_bytes = timeout(
            LOCAL_REQUEST_TIMEOUT,
            second_stream.read(&mut second_buffer),
        )
        .await
        .expect("second request bytes should arrive")
        .expect("read second request");
        let second = String::from_utf8_lossy(&second_buffer[..second_bytes]).to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
            pdf_bytes.len()
        );
        timeout(
            LOCAL_REQUEST_TIMEOUT,
            second_stream.write_all(response.as_bytes()),
        )
        .await
        .expect("headers should write")
        .expect("write headers");
        timeout(LOCAL_REQUEST_TIMEOUT, second_stream.write_all(pdf_bytes))
            .await
            .expect("body should write")
            .expect("write body");
        (first, second)
    });
    let auth = fixture_auth_with_cookies(&[("at-main", "cookie-a")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");

    let asset = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            input_id: "input-1",
            file_name: "Supplemental PDF.pdf",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &|| false,
    )
    .await;
    let (first, second) = server.await.expect("server task");

    assert!(asset.is_ok(), "redirected PDF should download: {asset:?}");
    if let Ok((_, committed)) = asset {
        committed.permanent();
    }
    assert!(first
        .to_ascii_lowercase()
        .contains("cookie: at-main=cookie-a"));
    assert!(
        !second.to_ascii_lowercase().contains("cookie:"),
        "redirected non-Audible request must not receive cookies: {second}"
    );
}

#[tokio::test]
async fn supplemental_pdf_download_rejects_non_pdf_without_provider_details() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let client = local_client(addr, None);
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nnot-pdf";
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by supplemental_pdf_download_rejects_non_pdf_without_provider_details"
    )]
    let server = tokio::spawn(async move { serve_one(&listener, response).await });
    let auth = fixture_auth_with_cookies(&[("at-main", "cookie-a")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");

    let failure = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            input_id: "input-1",
            file_name: "Supplemental PDF.pdf",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &|| false,
    )
    .await
    .expect_err("non-pdf should fail");
    let _ = server.await.expect("server task");
    let message = supplemental_pdf_failure_message(failure);

    assert_eq!(failure.category, "pdf_magic");
    assert!(!message.contains("B000000001"));
    assert!(!message.contains("http://"));
    assert!(
        std::fs::read_dir(root.path())
            .expect("read temp root")
            .all(|entry| {
                let path = entry.expect("dir entry").path();
                let path = path.to_string_lossy();
                !path.ends_with(".pdf") && !path.ends_with(".partial")
            }),
        "non-PDF response should not leave staged PDF files"
    );
}

async fn fetch_with_one_reply(reply: Vec<u8>, extra_host: Option<&str>) -> SupplementalPdfFailure {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let client = local_client(addr, extra_host);
    let reply = String::from_utf8(reply)
        .expect("utf8 reply")
        .replace("{port}", &addr.port().to_string());
    #[expect(clippy::disallowed_methods, reason = "joined by fetch_with_one_reply")]
    let server = tokio::spawn(async move { serve_one(&listener, reply.as_bytes()).await });
    let auth = fixture_auth_with_cookies(&[("at-main", "cookie-a")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");

    let failure = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            input_id: "input-1",
            file_name: "Supplemental PDF.pdf",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &|| false,
    )
    .await
    .expect_err("no PDF arrives");
    let _ = server.await.expect("server task");
    failure
}

fn html_reply(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[tokio::test]
async fn an_amazon_sign_in_page_asks_the_user_to_sign_in_again() {
    let failure = fetch_with_one_reply(
        html_reply(
            r#"<html><form name="signIn" method="post" action="https://www.amazon.com/ap/signin"></form></html>"#,
        ),
        None,
    )
    .await;

    assert_eq!(
        supplemental_pdf_failure_message(failure),
        "Sign in to Audible again to get the PDF."
    );
}

#[tokio::test]
async fn a_redirect_to_amazon_sign_in_asks_the_user_to_sign_in_again() {
    let failure = fetch_with_one_reply(
        b"HTTP/1.1 302 Found\r\nLocation: http://www.amazon.com:{port}/ap/signin?openid.return_to=x\r\nContent-Length: 0\r\n\r\n".to_vec(),
        Some("www.amazon.com"),
    )
    .await;

    assert_eq!(
        supplemental_pdf_failure_message(failure),
        "Sign in to Audible again to get the PDF."
    );
}

#[tokio::test]
async fn other_html_keeps_the_not_a_pdf_failure() {
    let failure = fetch_with_one_reply(
        html_reply("<html><body>Service unavailable</body></html>"),
        None,
    )
    .await;

    assert_eq!(failure.category, "pdf_magic");
    assert_ne!(
        supplemental_pdf_failure_message(failure),
        "Sign in to Audible again to get the PDF."
    );
}

#[tokio::test]
async fn supplemental_pdf_download_rejects_oversized_content_length() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let client = local_client(addr, None);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
        MAX_SUPPLEMENTAL_PDF_BYTES + 1
    );
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by supplemental_pdf_download_rejects_oversized_content_length"
    )]
    let server = tokio::spawn(async move { serve_one(&listener, response.as_bytes()).await });
    let auth = fixture_auth_with_cookies(&[("at-main", "cookie-a")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");

    let failure = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            input_id: "input-1",
            file_name: "Supplemental PDF.pdf",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &|| false,
    )
    .await
    .expect_err("oversized content should fail");
    let _ = server.await.expect("server task");

    assert_eq!(failure.category, "size_limit");
    assert!(
        std::fs::read_dir(root.path())
            .expect("read temp root")
            .all(|entry| !entry
                .expect("dir entry")
                .path()
                .to_string_lossy()
                .ends_with(".partial")),
        "oversized PDF should not leave a partial file"
    );
}

#[tokio::test]
async fn supplemental_pdf_download_cleans_partial_on_cancelled_stream() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let client = local_client(addr, None);
    let pdf_bytes = b"%PDF-1.7\nbody";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
        pdf_bytes.len()
    );
    let response = [response.as_bytes(), pdf_bytes].concat();
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by supplemental_pdf_download_cleans_partial_on_cancelled_stream"
    )]
    let server = tokio::spawn(async move { serve_one(&listener, &response).await });
    let auth = fixture_auth_with_cookies(&[("at-main", "cookie-a")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");
    let cancel_checks = AtomicUsize::new(0);

    let failure = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            input_id: "input-1",
            file_name: "Supplemental PDF.pdf",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &|| cancel_checks.fetch_add(1, Ordering::SeqCst) >= 3,
    )
    .await
    .expect_err("stream cancellation should fail");
    let _ = server.await.expect("server task");

    assert_eq!(failure.category, "cancelled");
    assert!(
        std::fs::read_dir(root.path())
            .expect("read temp root")
            .all(|entry| !entry
                .expect("dir entry")
                .path()
                .to_string_lossy()
                .ends_with(".partial")),
        "cancelled PDF should not leave a partial file"
    );
}

#[tokio::test]
async fn supplemental_pdf_download_maps_http_failure_without_provider_details() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let client = local_client(addr, None);
    let response = b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n";
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by supplemental_pdf_download_maps_http_failure_without_provider_details"
    )]
    let server = tokio::spawn(async move { serve_one(&listener, response).await });
    let auth = fixture_auth_with_cookies(&[("at-main", "cookie-a")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");

    let failure = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            input_id: "input-1",
            file_name: "Supplemental PDF.pdf",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &|| false,
    )
    .await
    .expect_err("forbidden should fail");
    let _ = server.await.expect("server task");
    let message = supplemental_pdf_failure_message(failure);

    assert_eq!(failure.category, "status");
    assert_eq!(failure.status, Some(reqwest::StatusCode::FORBIDDEN));
    assert!(!message.contains("B000000001"));
    assert!(!message.contains("http://"));
}
