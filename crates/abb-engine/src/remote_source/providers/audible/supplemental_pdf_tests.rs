use super::super::auth::{Auth, Locale, Registration};
use super::*;
use crate::remote_source::test_support::{Reply, StubServer};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::Duration;

const LOCAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

fn fixture_auth_with_cookies(cookies: &[(&str, &str)], store_cookie: &str) -> Auth {
    let website_cookies = cookies
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<std::collections::HashMap<_, _>>();
    Auth {
        locale: Locale::united_states(),
        device_registration: Registration {
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

#[test]
fn supplemental_pdf_downloader_requires_cookie_auth() {
    let auth = fixture_auth_with_cookies(&[], "");

    let failure = supplemental_pdf_cookie_header(&auth).expect_err("missing cookies");

    assert_eq!(failure.category, PdfFailure::MissingCookieAuth);
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

    assert_eq!(failure.category, PdfFailure::RedirectNonHttps);
}

#[tokio::test]
async fn supplemental_pdf_get_download_succeeds_when_head_would_fail() {
    let pdf_bytes = b"%PDF-1.7\nbody";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
        pdf_bytes.len()
    );
    let response = [response.as_bytes(), pdf_bytes].concat();
    let stub = StubServer::start(vec![Reply::bytes(response)]).await;
    let addr = stub.address();
    let client = local_client(addr, None);
    let auth =
        fixture_auth_with_cookies(&[("at-main", "cookie-a"), ("sess-at-main", "cookie-b")], "");
    let root = tempfile::TempDir::new().expect("temp root");
    let start_url = reqwest::Url::parse(&format!(
        "http://www.audible.com:{}/companion-file/B000000001",
        addr.port()
    ))
    .expect("url");
    let mut heard = Vec::new();

    let (pdf, committed) = download_supplemental_pdf_with_client(
        &client,
        SupplementalPdfRequest {
            auth: &auth,
            title_id: "B000000001",
            job_id: "job-1",
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &mut |bytes, total| heard.push((bytes, total)),
        &|| false,
    )
    .await
    .expect("download pdf");
    committed.permanent();
    let request = stub.only_request().await.raw;

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
    assert!(request_lower.contains(&format!(
        "user-agent: {}",
        AUDIBLE_DOWNLOAD_USER_AGENT.to_ascii_lowercase()
    )));
    assert!(request_lower.contains("accept: application/pdf,*/*"));
    assert_eq!(pdf.size_bytes, pdf_bytes.len() as u64);
    assert_eq!(pdf.sha256, abb_media_core::sha256_hex(pdf_bytes));
    assert_eq!(std::fs::read(&pdf.path).expect("read pdf"), pdf_bytes);
    let length = pdf_bytes.len() as u64;
    assert_eq!(heard.last(), Some(&(length, Some(length))));
}

#[tokio::test]
async fn supplemental_pdf_redirect_does_not_send_cookies_to_non_audible_hosts() {
    let pdf_bytes = b"%PDF-1.7\nbody";
    let stub = StubServer::start_addressed(|address| {
        let pdf_head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
            pdf_bytes.len()
        );
        vec![
            Reply::bytes(format!(
                "HTTP/1.1 302 Found\r\nLocation: http://cdn.example.test:{}/book.pdf\r\nContent-Length: 0\r\n\r\n",
                address.port()
            )),
            Reply::bytes([pdf_head.as_bytes(), pdf_bytes].concat()),
        ]
    })
    .await;
    let addr = stub.address();
    let client = local_client(addr, Some("cdn.example.test"));
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
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &mut |_, _| {},
        &|| false,
    )
    .await;
    let requests = stub.finished().await;
    let (first, second) = (&requests[0].raw, &requests[1].raw);

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
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nnot-pdf";
    let stub = StubServer::start(vec![Reply::bytes(response)]).await;
    let addr = stub.address();
    let client = local_client(addr, None);
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
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &mut |_, _| {},
        &|| false,
    )
    .await
    .expect_err("non-pdf should fail");
    stub.finished().await;
    let message = supplemental_pdf_failure_message(failure);

    assert_eq!(failure.category, PdfFailure::PdfMagic);
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
    let stub = StubServer::start_addressed(|address| {
        let reply = String::from_utf8(reply)
            .expect("utf8 reply")
            .replace("{port}", &address.port().to_string());
        vec![Reply::bytes(reply)]
    })
    .await;
    let addr = stub.address();
    let client = local_client(addr, extra_host);
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
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &mut |_, _| {},
        &|| false,
    )
    .await
    .expect_err("no PDF arrives");
    stub.finished().await;
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

    assert_eq!(failure.category, PdfFailure::PdfMagic);
    assert_ne!(
        supplemental_pdf_failure_message(failure),
        "Sign in to Audible again to get the PDF."
    );
}

#[tokio::test]
async fn supplemental_pdf_download_rejects_oversized_content_length() {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
        MAX_SUPPLEMENTAL_PDF_BYTES + 1
    );
    let stub = StubServer::start(vec![Reply::bytes(response)]).await;
    let addr = stub.address();
    let client = local_client(addr, None);
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
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &mut |_, _| {},
        &|| false,
    )
    .await
    .expect_err("oversized content should fail");
    stub.finished().await;

    assert_eq!(failure.category, PdfFailure::SizeLimit);
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
    let pdf_bytes = b"%PDF-1.7\nbody";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
        pdf_bytes.len()
    );
    let response = [response.as_bytes(), pdf_bytes].concat();
    let stub = StubServer::start(vec![Reply::bytes(response)]).await;
    let addr = stub.address();
    let client = local_client(addr, None);
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
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &mut |_, _| {},
        &|| cancel_checks.fetch_add(1, Ordering::SeqCst) >= 3,
    )
    .await
    .expect_err("stream cancellation should fail");
    stub.finished().await;

    assert_eq!(failure.category, PdfFailure::Cancelled);
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
    let response = b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n";
    let stub = StubServer::start(vec![Reply::bytes(response)]).await;
    let addr = stub.address();
    let client = local_client(addr, None);
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
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        start_url,
        SupplementalPdfTransportPolicy::HttpForTests,
        &mut |_, _| {},
        &|| false,
    )
    .await
    .expect_err("forbidden should fail");
    stub.finished().await;
    let message = supplemental_pdf_failure_message(failure);

    assert_eq!(failure.category, PdfFailure::Status);
    assert_eq!(failure.status, Some(reqwest::StatusCode::FORBIDDEN));
    assert!(!message.contains("B000000001"));
    assert!(!message.contains("http://"));
}

#[test]
fn failure_categories_log_under_their_snake_case_names() {
    for (category, logged) in [
        (PdfFailure::InvalidLocation, "invalid_location"),
        (PdfFailure::MissingCookieAuth, "missing_cookie_auth"),
        (PdfFailure::InvalidCookieAuth, "invalid_cookie_auth"),
        (PdfFailure::MissingLocation, "missing_location"),
        (PdfFailure::RedirectNonHttps, "redirect_non_https"),
        (PdfFailure::Request, "request"),
        (PdfFailure::Cancelled, "cancelled"),
        (PdfFailure::File, "file"),
        (PdfFailure::Empty, "empty"),
        (PdfFailure::SignIn, "sign_in"),
        (PdfFailure::PdfMagic, "pdf_magic"),
        (PdfFailure::RedirectLimit, "redirect_limit"),
        (PdfFailure::Status, "status"),
        (PdfFailure::SizeLimit, "size_limit"),
        (PdfFailure::Read, "read"),
    ] {
        assert_eq!(category.to_string(), logged);
    }
}
