//! Cover image ingestion: bounded local reads and the guarded remote fetch.
//! Both return write-ready JPEG bytes.
//!
//! URL covers are HTTPS only. The host must be a domain or a public IP literal
//! at entry and on every redirect; resolved addresses drop private ranges,
//! environment proxies are ignored, and logs keep only the URL origin.
//! Downloads cap at 10 MB and local files at 32 MB. HTTP 401/403 tells the
//! user to load the image from a file instead.

use crate::audio::validate_input_image_path;
use crate::errors::{AppError, Result};
use crate::metadata::optimize_cover_art;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;
use std::io::{self, Read};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use url::Host;

/// Prefer a supported-looking image in a multi-file drop. An unsupported
/// first file still goes through ingestion so the user receives its diagnostic.
pub(crate) fn dropped_cover_path(paths: Vec<String>) -> Option<String> {
    paths
        .iter()
        .find(|path| {
            std::path::Path::new(path)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    crate::audio::ALLOWED_IMAGE_EXTENSIONS
                        .iter()
                        .any(|allowed| ext.eq_ignore_ascii_case(allowed))
                })
        })
        .cloned()
        .or_else(|| paths.into_iter().next())
}

/// Loads a cover image from disk and returns write-ready JPEG bytes.
#[expect(clippy::disallowed_methods, reason = "joined by load_cover_art_file")]
pub(crate) async fn load_cover_art_file(file_path: String) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let validated_path = validate_input_image_path(&PathBuf::from(&file_path))?;
        let image_data = read_bounded_image(&validated_path)?;
        optimize_cover_art(&image_data)
    })
    .await
    .map_err(|e| AppError::General(format!("Cover art load task failed: {e}")))?
}

fn read_bounded_image(path: &std::path::Path) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(AppError::Io)?;
    let mut image_data = Vec::new();
    file.take(COVER_ART_MAX_FILE_BYTES + 1)
        .read_to_end(&mut image_data)
        .map_err(AppError::Io)?;
    if image_data.len() as u64 > COVER_ART_MAX_FILE_BYTES {
        return Err(AppError::InvalidInput(
            "Image file exceeds 32 MB limit".to_string(),
        ));
    }
    if image_data.is_empty() {
        return Err(AppError::InvalidInput(
            "Image file appears to be empty".to_string(),
        ));
    }
    Ok(image_data)
}

/// Downloads cover art from a remote URL and returns the image as served.
///
/// HTTPS-only with size and content-type validation. SSRF protection: literal
/// hosts must be public addresses, resolved domains drop private/reserved
/// addresses, every redirect is rechecked, and environment proxies are ignored
/// so the destination is always resolved here.
pub(crate) async fn download_cover_art(url: String) -> Result<Vec<u8>> {
    let validated_url = validate_cover_art_url(&url)?;
    let origin = url_origin_for_log(&validated_url);
    let client = cover_art_http_client()?;

    let mut response = client.get(validated_url).send().await.map_err(|e| {
        log::error!(
            "Failed to fetch cover image from {origin}: {}",
            e.without_url()
        );
        AppError::General("Failed to fetch image URL".to_string())
    })?;

    let status = response.status();
    log::debug!(
        "cover_download origin={origin} status={} version={:?}",
        status.as_u16(),
        response.version()
    );
    if !status.is_success() {
        return Err(AppError::InvalidInput(cover_status_message(status)));
    }

    if let Some(content_length) = response.content_length() {
        if content_length > COVER_ART_MAX_DOWNLOAD_BYTES as u64 {
            return Err(AppError::InvalidInput(
                "Image exceeds 10 MB limit".to_string(),
            ));
        }
    }

    if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
        let content_type = content_type
            .to_str()
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !is_supported_image_content_type(content_type.as_str()) {
            return Err(AppError::InvalidInput(
                "Unsupported image format. Use JPEG, PNG, or WebP.".to_string(),
            ));
        }
    }

    let mut downloaded = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        log::error!(
            "Failed to read cover image from {origin}: {}",
            e.without_url()
        );
        AppError::General("Failed to read image data".to_string())
    })? {
        if downloaded.len() + chunk.len() > COVER_ART_MAX_DOWNLOAD_BYTES {
            return Err(AppError::InvalidInput(
                "Image exceeds 10 MB limit".to_string(),
            ));
        }
        downloaded.extend_from_slice(&chunk);
    }

    if downloaded.is_empty() {
        return Err(AppError::InvalidInput(
            "Image response was empty".to_string(),
        ));
    }
    Ok(downloaded)
}

fn cover_status_message(status: StatusCode) -> String {
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return format!(
            "That URL blocked the image request (HTTP {}). Download the image and load it from a file.",
            status.as_u16()
        );
    }
    format!("Image request failed with status {status}")
}

static COVER_ART_HTTP_CLIENT: OnceLock<std::result::Result<reqwest::Client, String>> =
    OnceLock::new();

fn cover_art_http_client() -> Result<&'static reqwest::Client> {
    COVER_ART_HTTP_CLIENT
        .get_or_init(build_cover_art_http_client)
        .as_ref()
        .map_err(|message| {
            log::error!("Failed to configure HTTP client: {}", message);
            AppError::General("Failed to configure HTTP client".to_string())
        })
}

fn build_cover_art_http_client() -> std::result::Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(COVER_ART_FETCH_TIMEOUT_SECS))
        .dns_resolver(Arc::new(BogonFilteringResolver))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::custom(
            |attempt| match check_cover_redirect(attempt.url(), attempt.previous().len()) {
                Ok(()) => attempt.follow(),
                Err(reason) => {
                    log::warn!(
                        "Blocked cover redirect to {}: {reason}",
                        url_origin_for_log(attempt.url())
                    );
                    attempt.error(reason)
                }
            },
        ))
        .user_agent("audiobook-boss/cover-art")
        .build()
        .map_err(|error| error.to_string())
}

/// Max download size for remote cover art (DoS prevention)
const COVER_ART_MAX_DOWNLOAD_BYTES: usize = 10 * 1024 * 1024;
/// Max size of a local cover image read before decoding
const COVER_ART_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// HTTP fetch timeout for cover art
const COVER_ART_FETCH_TIMEOUT_SECS: u64 = 30;
/// Max redirects for cover art URL fetch
const COVER_ART_MAX_REDIRECTS: usize = 5;

fn validate_cover_art_url(url: &str) -> Result<reqwest::Url> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| AppError::InvalidInput("Invalid URL".to_string()))?;
    check_cover_url(&parsed).map_err(|reason| AppError::InvalidInput(reason.to_string()))?;
    Ok(parsed)
}

/// Accepts HTTPS URLs whose host is a domain (checked again at resolution) or
/// a public IP literal. IP literals never reach the resolver, so they are
/// judged here from the typed host rather than its bracketed text.
fn check_cover_url(url: &reqwest::Url) -> std::result::Result<(), &'static str> {
    if url.scheme() != "https" {
        return Err("Only HTTPS URLs are supported");
    }
    let ip = match url.host() {
        None => return Err("URL must include a host"),
        Some(Host::Domain(_)) => return Ok(()),
        Some(Host::Ipv4(ip)) => IpAddr::V4(ip),
        Some(Host::Ipv6(ip)) => IpAddr::V6(ip),
    };
    if bogon::is_bogon(ip) {
        return Err("URL resolves to a private or reserved IP address");
    }
    Ok(())
}

fn check_cover_redirect(
    url: &reqwest::Url,
    previous_redirects: usize,
) -> std::result::Result<(), &'static str> {
    if previous_redirects >= COVER_ART_MAX_REDIRECTS {
        return Err("too many redirects");
    }
    check_cover_url(url)
}

/// Scheme, host, and port only: paths, queries, and credentials stay out of logs.
fn url_origin_for_log(url: &reqwest::Url) -> String {
    url.origin().ascii_serialization()
}

#[derive(Debug)]
struct BogonFilteringResolver;

impl Resolve for BogonFilteringResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            // Port 0 is intentional; reqwest replaces it with the URL/scheme port.
            let addrs = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|e| {
                    log::warn!("DNS resolution failed for {}: {}", host, e);
                    let err: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
                    err
                })?;
            let mut filtered = Vec::new();
            for addr in addrs {
                if bogon::is_bogon(addr.ip()) {
                    log::warn!("Blocked bogon IP {} for host {}", addr.ip(), host);
                    continue;
                }
                filtered.push(addr);
            }

            if filtered.is_empty() {
                let err: Box<dyn std::error::Error + Send + Sync> = Box::new(io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    "URL resolves to a private or reserved IP address",
                ));
                return Err(err);
            }

            Ok(Box::new(filtered.into_iter()) as Addrs)
        })
    }
}

fn is_supported_image_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp"
    )
}

#[cfg(test)]
#[path = "cover_source_tests.rs"]
mod tests;
