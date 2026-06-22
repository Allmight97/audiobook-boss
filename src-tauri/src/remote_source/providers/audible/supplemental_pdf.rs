use audible_api::auth::Auth;
use reqwest::header::{HeaderValue, ACCEPT, COOKIE, LOCATION, USER_AGENT};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use abb_audible_core::title_ref;
use abb_media_core::{
    SupplementalPdfIdentity, SupplementalPdfIdentityBuilder, MAX_SUPPLEMENTAL_PDF_BYTES,
};

use super::acquisition::generated_staging_path;
use super::http::no_redirect_client;
use super::{AUDIBLE_DOWNLOAD_USER_AGENT, DOMAIN, MAX_DOWNLOAD_REDIRECTS};
use crate::remote_source::scoped_output::StagedTempFile;
use crate::remote_source::SupplementalAsset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SupplementalPdfFailure {
    pub(super) category: &'static str,
    pub(super) status: Option<reqwest::StatusCode>,
}

impl SupplementalPdfFailure {
    fn new(category: &'static str, status: Option<reqwest::StatusCode>) -> Self {
        Self { category, status }
    }
}

fn companion_file_url(title_id: &str) -> std::result::Result<reqwest::Url, SupplementalPdfFailure> {
    let encoded_title_id = urlencoding::encode(title_id);
    reqwest::Url::parse(&format!(
        "https://www.audible.{DOMAIN}/companion-file/{encoded_title_id}"
    ))
    .map_err(|_| SupplementalPdfFailure::new("invalid_location", None))
}

fn supplemental_pdf_cookie_header(
    auth: &Auth,
) -> std::result::Result<HeaderValue, SupplementalPdfFailure> {
    let mut cookie_fragments = auth
        .device_registration
        .website_cookies
        .iter()
        .filter(|(name, value)| !name.trim().is_empty() && !value.trim().is_empty())
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect::<Vec<_>>();
    cookie_fragments.sort_by(|left, right| left.0.cmp(&right.0));

    let mut values = cookie_fragments
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>();

    let store_cookie = auth.device_registration.store_authentication_cookie.trim();
    if !store_cookie.is_empty() {
        values.push(store_cookie.to_string());
    }

    if values.is_empty() {
        return Err(SupplementalPdfFailure::new("missing_cookie_auth", None));
    }

    HeaderValue::from_str(&values.join("; "))
        .map_err(|_| SupplementalPdfFailure::new("invalid_cookie_auth", None))
}

fn should_send_audible_website_cookie(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let audible_domain = format!("audible.{DOMAIN}");
    host == audible_domain || host.ends_with(&format!(".{audible_domain}"))
}

fn build_supplemental_pdf_get_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    cookie: &HeaderValue,
) -> reqwest::RequestBuilder {
    let request = client
        .get(url.clone())
        .header(USER_AGENT, AUDIBLE_DOWNLOAD_USER_AGENT)
        .header(ACCEPT, "application/pdf,*/*");

    if should_send_audible_website_cookie(&url) {
        request.header(COOKIE, cookie.clone())
    } else {
        request
    }
}

fn supplemental_pdf_redirect_target(
    current_url: &reqwest::Url,
    location: Option<&HeaderValue>,
    allow_insecure_for_test: bool,
) -> std::result::Result<reqwest::Url, SupplementalPdfFailure> {
    let location = location.ok_or_else(|| SupplementalPdfFailure::new("missing_location", None))?;
    let location = location
        .to_str()
        .map_err(|_| SupplementalPdfFailure::new("invalid_location", None))?;
    let next = current_url
        .join(location)
        .map_err(|_| SupplementalPdfFailure::new("invalid_location", None))?;
    if !url_is_allowed(&next, allow_insecure_for_test) {
        return Err(SupplementalPdfFailure::new("redirect_non_https", None));
    }
    Ok(next)
}

pub(super) struct SupplementalPdfRequest<'a> {
    pub auth: &'a Auth,
    pub title_id: &'a str,
    pub job_id: &'a str,
    pub input_id: &'a str,
    pub file_name: &'a str,
    pub api_pdf_hint_present: bool,
    pub job_dir: &'a Path,
}

#[derive(Clone, Copy)]
struct SupplementalPdfLog<'a> {
    job_id: &'a str,
    title_id: &'a str,
}

pub(super) async fn download_supplemental_pdf(
    request: SupplementalPdfRequest<'_>,
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<SupplementalAsset, SupplementalPdfFailure> {
    let client = no_redirect_client().map_err(|_| SupplementalPdfFailure::new("request", None))?;
    let url = companion_file_url(request.title_id)?;
    download_supplemental_pdf_with_client(&client, request, url, false, is_cancelled).await
}

async fn download_supplemental_pdf_with_client(
    client: &reqwest::Client,
    request: SupplementalPdfRequest<'_>,
    start_url: reqwest::Url,
    allow_insecure_for_test: bool,
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<SupplementalAsset, SupplementalPdfFailure> {
    if is_cancelled() {
        return Err(SupplementalPdfFailure::new("cancelled", None));
    }
    tokio::fs::create_dir_all(request.job_dir)
        .await
        .map_err(|_| SupplementalPdfFailure::new("file", None))?;
    let path = generated_staging_path(request.job_dir, "pdf");
    // Guard cleans the partial + (uncommitted) final on every early return / `?`.
    let mut staged = StagedTempFile::new(&path);
    staged
        .prepare()
        .map_err(|_| SupplementalPdfFailure::new("file", None))?;
    log_supplemental_pdf_download_start(
        request.job_id,
        request.title_id,
        request.api_pdf_hint_present,
    );

    let cookie = supplemental_pdf_cookie_header(request.auth)?;
    let log = SupplementalPdfLog {
        job_id: request.job_id,
        title_id: request.title_id,
    };
    let identity = fetch_pdf_to_partial(
        client,
        &cookie,
        log,
        start_url,
        allow_insecure_for_test,
        staged.partial_path(),
        is_cancelled,
    )
    .await?;

    if identity.size_bytes == 0 {
        return Err(SupplementalPdfFailure::new("empty", None));
    }
    if !identity.has_pdf_magic {
        return Err(SupplementalPdfFailure::new("pdf_magic", None));
    }
    staged
        .rename_and_commit(is_cancelled)
        .await
        .map_err(|_| SupplementalPdfFailure::new("file", None))?;
    let canonical = canonicalize_staged_pdf(&path)?;
    log_supplemental_pdf_download_complete(request.job_id, request.title_id, identity.size_bytes);
    Ok(SupplementalAsset {
        asset_id: uuid::Uuid::new_v4().to_string(),
        input_id: request.input_id.to_string(),
        title_id: request.title_id.to_string(),
        path: canonical,
        file_name: request.file_name.to_string(),
        size_bytes: identity.size_bytes,
        sha256: identity.sha256,
    })
}

/// Follow redirects and stream the companion-file PDF into `partial_path`,
/// returning the bytes/hash/header facts. Cleanup of `partial_path` on failure is
/// the caller's [`StagedTempFile`] guard responsibility.
async fn fetch_pdf_to_partial(
    client: &reqwest::Client,
    cookie: &HeaderValue,
    log: SupplementalPdfLog<'_>,
    start_url: reqwest::Url,
    allow_insecure_for_test: bool,
    partial_path: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<SupplementalPdfIdentity, SupplementalPdfFailure> {
    let mut url = start_url;
    let mut redirect_count = 0_usize;
    loop {
        if is_cancelled() {
            return Err(SupplementalPdfFailure::new("cancelled", None));
        }
        if !url_is_allowed(&url, allow_insecure_for_test) {
            return Err(SupplementalPdfFailure::new("redirect_non_https", None));
        }
        let response = build_supplemental_pdf_get_request(client, url.clone(), cookie)
            .send()
            .await
            .map_err(|_| SupplementalPdfFailure::new("request", None))?;
        if is_cancelled() {
            return Err(SupplementalPdfFailure::new("cancelled", None));
        }

        let status = response.status();
        let response_url = response.url().clone();
        let final_https = response_url.scheme() == "https";
        if status.is_redirection() {
            log_supplemental_pdf_request_status(
                log.job_id,
                log.title_id,
                status,
                redirect_count,
                final_https,
                0,
                false,
            );
            if redirect_count >= MAX_DOWNLOAD_REDIRECTS {
                return Err(SupplementalPdfFailure::new("redirect_limit", Some(status)));
            }
            let next = supplemental_pdf_redirect_target(
                &url,
                response.headers().get(LOCATION),
                allow_insecure_for_test,
            )?;
            redirect_count += 1;
            url = next;
            continue;
        }

        if !status.is_success() || !url_is_allowed(&response_url, allow_insecure_for_test) {
            log_supplemental_pdf_request_status(
                log.job_id,
                log.title_id,
                status,
                redirect_count,
                final_https,
                0,
                false,
            );
            return Err(SupplementalPdfFailure::new("status", Some(status)));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_SUPPLEMENTAL_PDF_BYTES)
        {
            log_supplemental_pdf_request_status(
                log.job_id,
                log.title_id,
                status,
                redirect_count,
                final_https,
                0,
                false,
            );
            return Err(SupplementalPdfFailure::new("size_limit", None));
        }

        let identity = stream_pdf_body(response, partial_path, is_cancelled).await?;
        log_supplemental_pdf_request_status(
            log.job_id,
            log.title_id,
            status,
            redirect_count,
            final_https,
            identity.size_bytes,
            true,
        );
        return Ok(identity);
    }
}

/// Stream the response body to the staged partial path, enforcing the size cap
/// and cancellation. Cleanup of the partial on failure is the caller's
/// [`StagedTempFile`] guard responsibility.
async fn stream_pdf_body(
    mut response: reqwest::Response,
    partial_path: &Path,
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<SupplementalPdfIdentity, SupplementalPdfFailure> {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(partial_path)
        .await
        .map_err(|_| SupplementalPdfFailure::new("file", None))?;
    let mut identity = SupplementalPdfIdentityBuilder::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| SupplementalPdfFailure::new("read", None))?
    {
        if is_cancelled() {
            return Err(SupplementalPdfFailure::new("cancelled", None));
        }
        if chunk.is_empty() {
            continue;
        }
        let next_size = identity
            .size_bytes()
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| SupplementalPdfFailure::new("size_limit", None))?;
        if next_size > MAX_SUPPLEMENTAL_PDF_BYTES {
            return Err(SupplementalPdfFailure::new("size_limit", None));
        }
        identity.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|_| SupplementalPdfFailure::new("file", None))?;
    }
    file.sync_all()
        .await
        .map_err(|_| SupplementalPdfFailure::new("file", None))?;
    Ok(identity.finalize())
}

fn canonicalize_staged_pdf(path: &Path) -> std::result::Result<PathBuf, SupplementalPdfFailure> {
    path.canonicalize()
        .map_err(|_| SupplementalPdfFailure::new("canonicalize", None))
}

fn url_is_allowed(url: &reqwest::Url, allow_insecure_for_test: bool) -> bool {
    url.scheme() == "https" || allow_insecure_for_test && url.scheme() == "http"
}

pub(super) fn supplemental_pdf_failure_message(failure: SupplementalPdfFailure) -> String {
    match failure.category {
        "missing_cookie_auth" | "invalid_cookie_auth" => {
            "Audible Supplemental PDF authentication material was unavailable. Provider-private details were withheld from UI and logs.".to_string()
        }
        _ => "Audible Supplemental PDF could not be downloaded through the authenticated companion-file route. Provider-private details were withheld from UI and logs.".to_string(),
    }
}

fn log_supplemental_pdf_download_start(job_id: &str, title_id: &str, api_pdf_hint_present: bool) {
    log::info!(
        "remote_source audible stage=supplemental_pdf_download_start job_id={} title_ref={} endpoint=companion_file method=get api_pdf_hint_present={} auth=cookie",
        job_id,
        title_ref(title_id),
        api_pdf_hint_present
    );
}

fn log_supplemental_pdf_request_status(
    job_id: &str,
    title_id: &str,
    status: reqwest::StatusCode,
    redirect_count: usize,
    final_https: bool,
    bytes: u64,
    downloaded: bool,
) {
    log::info!(
        "remote_source audible stage=supplemental_pdf_download_status job_id={} title_ref={} endpoint=companion_file http_status={} redirect_count={} final_https={} bytes={} downloaded={}",
        job_id,
        title_ref(title_id),
        status.as_u16(),
        redirect_count,
        final_https,
        bytes,
        downloaded
    );
}

fn log_supplemental_pdf_download_complete(job_id: &str, title_id: &str, bytes: u64) {
    log::info!(
        "remote_source audible stage=supplemental_pdf_download_complete job_id={} title_ref={} bytes={}",
        job_id,
        title_ref(title_id),
        bytes
    );
}

pub(super) fn log_supplemental_pdf_failed(
    job_id: &str,
    title_id: &str,
    failure: SupplementalPdfFailure,
) {
    log::warn!(
        "remote_source audible stage=supplemental_pdf_failed job_id={} title_ref={} category={} http_status={}",
        job_id,
        title_ref(title_id),
        failure.category,
        failure.status.map(|status| status.as_u16()).unwrap_or(0)
    );
}

#[cfg(test)]
mod tests {
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

        let failure = supplemental_pdf_redirect_target(&current, Some(&location), false)
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
        let server = tokio::spawn(async move { serve_one(&listener, &response).await });
        let auth =
            fixture_auth_with_cookies(&[("at-main", "cookie-a"), ("sess-at-main", "cookie-b")], "");
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
                file_name: "Being You - A New Science of Consciousness - Supplemental PDF.pdf",
                api_pdf_hint_present: true,
                job_dir: root.path(),
            },
            start_url,
            true,
            &|| false,
        )
        .await
        .expect("download pdf");
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
            true,
            &|| false,
        )
        .await;
        let (first, second) = server.await.expect("server task");

        assert!(asset.is_ok(), "redirected PDF should download: {asset:?}");
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
            true,
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
            true,
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
            true,
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
            true,
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
}
