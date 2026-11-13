use super::auth::Auth;
use reqwest::header::{HeaderValue, ACCEPT, COOKIE, LOCATION, USER_AGENT};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use abb_audible_core::title_ref;
use abb_media_core::{
    SupplementalPdfIdentity, SupplementalPdfIdentityBuilder, MAX_SUPPLEMENTAL_PDF_BYTES,
};

use super::acquisition::generated_staging_path;
use super::http::no_redirect_client;
use super::{AUDIBLE_DOWNLOAD_USER_AGENT, DOMAIN, MAX_DOWNLOAD_REDIRECTS};
use crate::remote_source::scoped_output::{ProvisionalCommittedFile, StagedTempFile};
use crate::remote_source::StagedPdf;

const SIGN_IN_PATH: &str = "/ap/signin";
const SIGN_IN_PAGE_SCAN_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PdfFailure {
    InvalidLocation,
    MissingCookieAuth,
    InvalidCookieAuth,
    MissingLocation,
    RedirectNonHttps,
    Request,
    Cancelled,
    File,
    Empty,
    SignIn,
    PdfMagic,
    RedirectLimit,
    Status,
    SizeLimit,
    Read,
}

impl std::fmt::Display for PdfFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLocation => "invalid_location",
            Self::MissingCookieAuth => "missing_cookie_auth",
            Self::InvalidCookieAuth => "invalid_cookie_auth",
            Self::MissingLocation => "missing_location",
            Self::RedirectNonHttps => "redirect_non_https",
            Self::Request => "request",
            Self::Cancelled => "cancelled",
            Self::File => "file",
            Self::Empty => "empty",
            Self::SignIn => "sign_in",
            Self::PdfMagic => "pdf_magic",
            Self::RedirectLimit => "redirect_limit",
            Self::Status => "status",
            Self::SizeLimit => "size_limit",
            Self::Read => "read",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SupplementalPdfFailure {
    pub(super) category: PdfFailure,
    pub(super) status: Option<reqwest::StatusCode>,
}

impl SupplementalPdfFailure {
    fn new(category: PdfFailure, status: Option<reqwest::StatusCode>) -> Self {
        Self { category, status }
    }
}

fn companion_file_url(title_id: &str) -> std::result::Result<reqwest::Url, SupplementalPdfFailure> {
    let mut url = reqwest::Url::parse(&format!("https://www.audible.{DOMAIN}/"))
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::InvalidLocation, None))?;
    url.path_segments_mut()
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::InvalidLocation, None))?
        .extend(["companion-file", title_id]);
    Ok(url)
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
        return Err(SupplementalPdfFailure::new(
            PdfFailure::MissingCookieAuth,
            None,
        ));
    }

    HeaderValue::from_str(&values.join("; "))
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::InvalidCookieAuth, None))
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
    transport_policy: SupplementalPdfTransportPolicy,
) -> std::result::Result<reqwest::Url, SupplementalPdfFailure> {
    let location =
        location.ok_or_else(|| SupplementalPdfFailure::new(PdfFailure::MissingLocation, None))?;
    let location = location
        .to_str()
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::InvalidLocation, None))?;
    let next = current_url
        .join(location)
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::InvalidLocation, None))?;
    if !transport_policy.allows(&next) {
        return Err(SupplementalPdfFailure::new(
            PdfFailure::RedirectNonHttps,
            None,
        ));
    }
    Ok(next)
}

#[derive(Clone, Copy)]
enum SupplementalPdfTransportPolicy {
    HttpsOnly,
    #[cfg(test)]
    HttpForTests,
}

impl SupplementalPdfTransportPolicy {
    fn allows(self, url: &reqwest::Url) -> bool {
        match self {
            Self::HttpsOnly => url.scheme() == "https",
            #[cfg(test)]
            Self::HttpForTests => matches!(url.scheme(), "http" | "https"),
        }
    }
}

pub(super) struct SupplementalPdfRequest<'a> {
    pub auth: &'a Auth,
    pub title_id: &'a str,
    pub job_id: &'a str,
    pub api_pdf_hint_present: bool,
    pub job_dir: &'a Path,
}

#[derive(Clone, Copy)]
struct SupplementalPdfLog<'a> {
    job_id: &'a str,
    title_id: &'a str,
}

/// `on_bytes` hears the bytes received so far and the reply's length.
pub(super) async fn download_supplemental_pdf(
    request: SupplementalPdfRequest<'_>,
    on_bytes: &mut impl FnMut(u64, Option<u64>),
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<(StagedPdf, ProvisionalCommittedFile), SupplementalPdfFailure> {
    let client =
        no_redirect_client().map_err(|_| SupplementalPdfFailure::new(PdfFailure::Request, None))?;
    let url = companion_file_url(request.title_id)?;
    download_supplemental_pdf_with_client(
        &client,
        request,
        url,
        SupplementalPdfTransportPolicy::HttpsOnly,
        on_bytes,
        is_cancelled,
    )
    .await
}

async fn download_supplemental_pdf_with_client(
    client: &reqwest::Client,
    request: SupplementalPdfRequest<'_>,
    start_url: reqwest::Url,
    transport_policy: SupplementalPdfTransportPolicy,
    on_bytes: &mut impl FnMut(u64, Option<u64>),
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<(StagedPdf, ProvisionalCommittedFile), SupplementalPdfFailure> {
    if is_cancelled() {
        return Err(SupplementalPdfFailure::new(PdfFailure::Cancelled, None));
    }
    tokio::fs::create_dir_all(request.job_dir)
        .await
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::File, None))?;
    let path = generated_staging_path(request.job_dir, "pdf");
    let staged = StagedTempFile::create(&path).map_err(|error| {
        log::warn!(
            "remote_source audible stage=supplemental_pdf_failed job_id={} title_ref={} category=create error={error}",
            request.job_id,
            title_ref(request.title_id),
        );
        SupplementalPdfFailure::new(PdfFailure::File, None)
    })?;
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
    let fetch = PdfFetch {
        client,
        cookie: &cookie,
        transport_policy,
    };
    let identity = fetch_pdf_to_partial(
        fetch,
        log,
        start_url,
        staged.partial_path(),
        on_bytes,
        is_cancelled,
    )
    .await?;

    if identity.size_bytes == 0 {
        return Err(SupplementalPdfFailure::new(PdfFailure::Empty, None));
    }
    if !identity.has_pdf_magic {
        let category = if is_sign_in_page(staged.partial_path()).await {
            PdfFailure::SignIn
        } else {
            PdfFailure::PdfMagic
        };
        return Err(SupplementalPdfFailure::new(category, None));
    }
    staged.rename_and_commit(is_cancelled).await.map_err(|error| {
        log::warn!(
            "remote_source audible stage=supplemental_pdf_failed job_id={} title_ref={} category=output_commit error={error}",
            request.job_id,
            title_ref(request.title_id),
        );
        SupplementalPdfFailure::new(PdfFailure::File, None)
    })?;
    let committed = ProvisionalCommittedFile::new(path.clone());
    log_supplemental_pdf_download_complete(request.job_id, request.title_id, identity.size_bytes);
    Ok((
        StagedPdf {
            path,
            size_bytes: identity.size_bytes,
            sha256: identity.sha256,
        },
        committed,
    ))
}

#[derive(Clone, Copy)]
struct PdfFetch<'a> {
    client: &'a reqwest::Client,
    cookie: &'a HeaderValue,
    transport_policy: SupplementalPdfTransportPolicy,
}

async fn fetch_pdf_to_partial(
    fetch: PdfFetch<'_>,
    log: SupplementalPdfLog<'_>,
    start_url: reqwest::Url,
    partial_path: &Path,
    on_bytes: &mut impl FnMut(u64, Option<u64>),
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<SupplementalPdfIdentity, SupplementalPdfFailure> {
    let mut url = start_url;
    let mut redirect_count = 0_usize;
    loop {
        if is_cancelled() {
            return Err(SupplementalPdfFailure::new(PdfFailure::Cancelled, None));
        }
        if !fetch.transport_policy.allows(&url) {
            return Err(SupplementalPdfFailure::new(
                PdfFailure::RedirectNonHttps,
                None,
            ));
        }
        let response = build_supplemental_pdf_get_request(fetch.client, url.clone(), fetch.cookie)
            .send()
            .await
            .map_err(|_| SupplementalPdfFailure::new(PdfFailure::Request, None))?;
        if is_cancelled() {
            return Err(SupplementalPdfFailure::new(PdfFailure::Cancelled, None));
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
                return Err(SupplementalPdfFailure::new(
                    PdfFailure::RedirectLimit,
                    Some(status),
                ));
            }
            let next = supplemental_pdf_redirect_target(
                &url,
                response.headers().get(LOCATION),
                fetch.transport_policy,
            )?;
            if is_sign_in_url(&next) {
                return Err(SupplementalPdfFailure::new(
                    PdfFailure::SignIn,
                    Some(status),
                ));
            }
            redirect_count += 1;
            url = next;
            continue;
        }

        if !status.is_success() || !fetch.transport_policy.allows(&response_url) {
            log_supplemental_pdf_request_status(
                log.job_id,
                log.title_id,
                status,
                redirect_count,
                final_https,
                0,
                false,
            );
            return Err(SupplementalPdfFailure::new(
                PdfFailure::Status,
                Some(status),
            ));
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
            return Err(SupplementalPdfFailure::new(PdfFailure::SizeLimit, None));
        }

        let identity = stream_pdf_body(response, partial_path, on_bytes, is_cancelled).await?;
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

fn is_sign_in_url(url: &reqwest::Url) -> bool {
    url.host_str()
        .is_some_and(|host| host.split('.').any(|label| label == "amazon"))
        && url.path().starts_with(SIGN_IN_PATH)
}

async fn is_sign_in_page(path: &Path) -> bool {
    let mut head = Vec::new();
    let Ok(file) = tokio::fs::File::open(path).await else {
        return false;
    };
    if file
        .take(SIGN_IN_PAGE_SCAN_BYTES)
        .read_to_end(&mut head)
        .await
        .is_err()
    {
        return false;
    }
    head.windows(SIGN_IN_PATH.len())
        .any(|window| window == SIGN_IN_PATH.as_bytes())
}

async fn stream_pdf_body(
    mut response: reqwest::Response,
    partial_path: &Path,
    on_bytes: &mut impl FnMut(u64, Option<u64>),
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<SupplementalPdfIdentity, SupplementalPdfFailure> {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(partial_path)
        .await
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::File, None))?;
    let total = response.content_length();
    let mut identity = SupplementalPdfIdentityBuilder::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::Read, None))?
    {
        if is_cancelled() {
            return Err(SupplementalPdfFailure::new(PdfFailure::Cancelled, None));
        }
        if chunk.is_empty() {
            continue;
        }
        let next_size = identity
            .size_bytes()
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| SupplementalPdfFailure::new(PdfFailure::SizeLimit, None))?;
        if next_size > MAX_SUPPLEMENTAL_PDF_BYTES {
            return Err(SupplementalPdfFailure::new(PdfFailure::SizeLimit, None));
        }
        identity.update(&chunk);
        on_bytes(identity.size_bytes(), total);
        file.write_all(&chunk)
            .await
            .map_err(|_| SupplementalPdfFailure::new(PdfFailure::File, None))?;
    }
    file.sync_all()
        .await
        .map_err(|_| SupplementalPdfFailure::new(PdfFailure::File, None))?;
    Ok(identity.finalize())
}

pub(super) fn supplemental_pdf_failure_message(failure: SupplementalPdfFailure) -> String {
    match failure.category {
        PdfFailure::SignIn => "Sign in to Audible again to get the PDF.".to_string(),
        PdfFailure::MissingCookieAuth | PdfFailure::InvalidCookieAuth => {
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
#[path = "supplemental_pdf_tests.rs"]
mod tests;
