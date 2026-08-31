use crate::audio::{validate_input_audio_path, validate_input_image_path};
use crate::commands::CommandResult;
use crate::errors::{AppError, Result};
use crate::metadata::{
    cover_art_view_for_display, optimize_cover_art,
    read_audio_cover_thumbnail as read_embedded_cover_thumbnail, read_metadata, stage_cover_art,
    AudiobookMetadata, CoverArtView, CoverStash, IpcMetadataIntentPatch,
    IpcMetadataIntentValidationResult,
};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::CONTENT_TYPE;
use std::io;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub(crate) mod save_batch;
pub use save_batch::{
    save_metadata_batch, MetadataSaveBatchResult, MetadataSaveRequest, MetadataSaveResultEntry,
    MetadataSaveResultStatus, MetadataSaveSummary,
};

/// Reads metadata from an audio file
/// Returns metadata as JSON-serializable struct
#[tauri::command]
#[specta::specta]
pub async fn read_audio_metadata(file_path: String) -> CommandResult<AudiobookMetadata> {
    let result: Result<AudiobookMetadata> = tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(&file_path);
        let validated_path = validate_input_audio_path(&path)?;
        let mut metadata = read_metadata(validated_path.to_string_lossy().as_ref())?;
        metadata.cover_art = None;
        Ok(metadata)
    })
    .await
    .map_err(|e| AppError::General(format!("Metadata read task failed: {e}")))?;

    Ok(result?)
}

/// Reads an audio file's embedded cover as a bounded JPEG thumbnail.
#[tauri::command]
#[specta::specta]
pub async fn read_audio_cover_thumbnail(file_path: String) -> CommandResult<Option<CoverArtView>> {
    let result: Result<Option<CoverArtView>> = tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(&file_path);
        let validated_path = validate_input_audio_path(&path)?;
        Ok(read_embedded_cover_thumbnail(&validated_path)?
            .as_deref()
            .map(cover_art_view_for_display))
    })
    .await
    .map_err(|error| AppError::General(format!("Cover thumbnail read task failed: {error}")))?;

    Ok(result?)
}

/// Saves metadata to an audio file using explicit write intent (metadata-only editing)
///
/// This command is designed for metadata-only editing (Cmd+S workflow):
/// 1. Preserves album sort unless explicit set, clear, or recompute intent is provided
/// 2. Writes metadata non-destructively (preserves existing cover art if not replaced)
/// 3. Handles cover art: preserves existing if not provided, replaces if new art given
#[tauri::command]
#[specta::specta]
pub async fn save_metadata_to_file(
    file_path: String,
    metadata_patch: IpcMetadataIntentPatch,
    stash: tauri::State<'_, CoverStash>,
) -> CommandResult<()> {
    let metadata_patch = metadata_patch.into_core(&stash)?;
    let result: Result<()> = tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(&file_path);
        let validated_path = validate_input_audio_path(&path)?;

        crate::metadata::save_metadata_intent(&validated_path, &metadata_patch)?;

        log::info!("Metadata saved to: {}", validated_path.display());
        Ok(())
    })
    .await
    .map_err(|e| AppError::General(format!("Metadata write task failed: {e}")))?;

    Ok(result?)
}

/// Validates and normalizes metadata intent without writing files.
#[tauri::command]
#[specta::specta]
pub fn validate_metadata_intent_patch(
    metadata_patch: IpcMetadataIntentPatch,
) -> CommandResult<IpcMetadataIntentValidationResult> {
    let cover_art = metadata_patch.cover_art.clone();
    let core_patch = metadata_patch.into_core_skipping_cover();
    Ok(IpcMetadataIntentPatch::from_validated(
        crate::metadata::validate_metadata_intent_patch(&core_patch),
        cover_art,
    ))
}

/// Writes cover art to an M4B file.
/// Idle command: production frontend stages through [`load_cover_art_file`] /
/// [`load_cover_art_from_url`] and hydrates handles at save/process ingress.
#[tauri::command]
#[specta::specta]
pub fn write_cover_art(file_path: String, cover_data: Vec<u8>) -> CommandResult<()> {
    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_audio_path(&path)?;
    crate::metadata::write_cover_art_to_file(&validated_path, cover_data)?;
    Ok(())
}

/// Loads an image file, optimizes it to JPEG, and stages the bytes for commit.
#[tauri::command]
#[specta::specta]
pub async fn load_cover_art_file(
    file_path: String,
    stash: tauri::State<'_, CoverStash>,
) -> CommandResult<CoverArtView> {
    use std::fs;

    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_image_path(&path)?;

    let image_data = fs::read(&validated_path).map_err(AppError::Io)?;
    if image_data.is_empty() {
        return Err(AppError::InvalidInput("Image file appears to be empty".to_string()).into());
    }

    let optimized = optimize_cover_art(&image_data)?;
    Ok(stage_cover_art(&stash, optimized)?)
}

/// Fetches cover art from a remote URL, optimizes it, and stages the bytes for commit.
///
/// HTTPS-only with size and content-type validation. SSRF protection blocks
/// private/loopback/link-local IPs. Lookup and library grids must use
/// [`preview_cover_art_from_url`] so preview traffic cannot evict staged covers.
#[tauri::command]
#[specta::specta]
pub async fn load_cover_art_from_url(
    url: String,
    stash: tauri::State<'_, CoverStash>,
) -> CommandResult<CoverArtView> {
    let optimized = fetch_optimized_cover_from_url(url).await?;
    Ok(stage_cover_art(&stash, optimized)?)
}

/// Fetches and optimizes remote cover art for display only. Does not stage a
/// commit handle. Same HTTPS/SSRF policy as [`load_cover_art_from_url`].
#[tauri::command]
#[specta::specta]
pub async fn preview_cover_art_from_url(url: String) -> CommandResult<CoverArtView> {
    let optimized = fetch_optimized_cover_from_url(url).await?;
    Ok(cover_art_view_for_display(&optimized))
}

async fn fetch_optimized_cover_from_url(url: String) -> Result<Vec<u8>> {
    let validated_url = validate_cover_art_url(&url)?;
    let url_for_log = validated_url.as_str().to_string();
    let client = cover_art_http_client()?;

    let mut response = client.get(validated_url).send().await.map_err(|e| {
        log::error!("Failed to fetch image URL {}: {}", url_for_log, e);
        AppError::General("Failed to fetch image URL".to_string())
    })?;

    if !response.status().is_success() {
        return Err(AppError::InvalidInput(format!(
            "Image request failed with status {}",
            response.status()
        )));
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
        log::error!("Failed to read image data from URL {}: {}", url_for_log, e);
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

    optimize_cover_art(&downloaded)
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
    let resolver = Arc::new(BogonFilteringResolver);
    reqwest::Client::builder()
        .timeout(Duration::from_secs(COVER_ART_FETCH_TIMEOUT_SECS))
        .dns_resolver(resolver)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let redirect_count = attempt.previous().len();
            if redirect_count >= COVER_ART_MAX_REDIRECTS {
                log::warn!(
                    "Blocked redirect due to limit (count={}): {}",
                    redirect_count,
                    attempt.url()
                );
                return attempt.error("too many redirects");
            }

            let url = attempt.url();
            if url.scheme() != "https" {
                log::warn!("Blocked redirect to non-HTTPS URL: {}", url);
                return attempt.error("Only HTTPS URLs are supported");
            }

            let Some(host) = url.host_str() else {
                log::warn!("Blocked redirect without host: {}", url);
                return attempt.error("Redirect URL has no host");
            };

            if let Ok(ip) = host.parse::<IpAddr>() {
                if bogon::is_bogon(ip) {
                    log::warn!("Blocked redirect to bogon IP {} for host {}", ip, host);
                    return attempt.error("Redirect to private IP blocked");
                }
            }

            attempt.follow()
        }))
        .user_agent("audiobook-boss/cover-art")
        .build()
        .map_err(|error| error.to_string())
}

/// Max download size for remote cover art (DoS prevention)
const COVER_ART_MAX_DOWNLOAD_BYTES: usize = 10 * 1024 * 1024;
/// HTTP fetch timeout for cover art
const COVER_ART_FETCH_TIMEOUT_SECS: u64 = 30;
/// Max redirects for cover art URL fetch
const COVER_ART_MAX_REDIRECTS: usize = 5;

fn validate_cover_art_url(url: &str) -> Result<reqwest::Url> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| AppError::InvalidInput("Invalid URL".to_string()))?;

    if parsed.scheme() != "https" {
        return Err(AppError::InvalidInput(
            "Only HTTPS URLs are supported".to_string(),
        ));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::InvalidInput("URL must include a host".to_string()))?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        if bogon::is_bogon(ip) {
            return Err(AppError::InvalidInput(
                "URL resolves to a private or reserved IP address".to_string(),
            ));
        }
    }

    Ok(parsed)
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
mod metadata_tests;
