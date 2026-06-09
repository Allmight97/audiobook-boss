use crate::audio::{validate_input_audio_path, validate_input_image_path};
use crate::commands::CommandResult;
use crate::errors::{AppError, Result};
use crate::metadata::{
    plan_metadata_write, read_metadata, AudiobookMetadata, MetadataIntentPatch,
    MetadataIntentValidationResult,
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
        read_metadata(validated_path.to_string_lossy().as_ref())
    })
    .await
    .map_err(|e| AppError::General(format!("Metadata read task failed: {e}")))?;

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
    metadata_patch: MetadataIntentPatch,
) -> CommandResult<()> {
    let result: Result<()> = tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(&file_path);
        let validated_path = validate_input_audio_path(&path)?;
        let write_plan = plan_metadata_write(&metadata_patch)?;

        crate::metadata::save_metadata_with_plan(&validated_path, &write_plan)?;

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
    metadata_patch: MetadataIntentPatch,
) -> CommandResult<MetadataIntentValidationResult> {
    Ok(crate::metadata::validate_metadata_intent_patch(
        &metadata_patch,
    ))
}

/// Writes cover art to an M4B file
/// Accepts file path and base64-encoded image data
#[tauri::command]
#[specta::specta]
pub fn write_cover_art(file_path: String, cover_data: Vec<u8>) -> CommandResult<()> {
    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_audio_path(&path)?;
    crate::metadata::write_cover_art_to_file(&validated_path, cover_data)?;
    Ok(())
}

/// Loads image file from disk and returns as byte array
/// Supports common image formats: jpg, jpeg, png, webp
#[tauri::command]
#[specta::specta]
pub async fn load_cover_art_file(file_path: String) -> CommandResult<Vec<u8>> {
    use std::fs;

    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_image_path(&path)?;

    // Read file contents using the validated canonical path
    let image_data = fs::read(&validated_path).map_err(AppError::Io)?;

    // Validate it's not empty
    if image_data.is_empty() {
        return Err(AppError::InvalidInput("Image file appears to be empty".to_string()).into());
    }

    // Optimize cover art: resize, flatten transparency, convert to JPEG
    // Format validation is handled by with_guessed_format() in optimize_cover_art (#32)
    let optimized = optimize_cover_art(&image_data)?;

    Ok(optimized)
}

/// Loads cover art from a remote URL and returns optimized image bytes
/// HTTPS-only with size and content-type validation for safety.
/// Includes SSRF protection: blocks requests to private/loopback/link-local IPs.
#[tauri::command]
#[specta::specta]
pub async fn load_cover_art_from_url(url: String) -> CommandResult<Vec<u8>> {
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
        ))
        .into());
    }

    if let Some(content_length) = response.content_length() {
        if content_length > COVER_ART_MAX_DOWNLOAD_BYTES as u64 {
            return Err(AppError::InvalidInput("Image exceeds 10 MB limit".to_string()).into());
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
            )
            .into());
        }
    }

    let mut downloaded = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        log::error!("Failed to read image data from URL {}: {}", url_for_log, e);
        AppError::General("Failed to read image data".to_string())
    })? {
        if downloaded.len() + chunk.len() > COVER_ART_MAX_DOWNLOAD_BYTES {
            return Err(AppError::InvalidInput("Image exceeds 10 MB limit".to_string()).into());
        }
        downloaded.extend_from_slice(&chunk);
    }

    if downloaded.is_empty() {
        return Err(AppError::InvalidInput("Image response was empty".to_string()).into());
    }

    let optimized = optimize_cover_art(&downloaded)?;
    Ok(optimized)
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
    let resolver = Arc::new(BogonFilteringResolver::new());
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

/// Maximum dimension for cover art (width or height)
const COVER_ART_MAX_DIMENSION: u32 = 800;
/// JPEG quality for cover art (0-100)
const COVER_ART_JPEG_QUALITY: u8 = 85;

/// Maximum input image dimension allowed (DoS prevention)
const COVER_ART_MAX_INPUT_DIMENSION: u32 = 4096;
/// Max download size for remote cover art (DoS prevention)
const COVER_ART_MAX_DOWNLOAD_BYTES: usize = 10 * 1024 * 1024;
/// HTTP fetch timeout for cover art
const COVER_ART_FETCH_TIMEOUT_SECS: u64 = 30;
/// Max redirects for cover art URL fetch
const COVER_ART_MAX_REDIRECTS: usize = 5;

/// Optimizes cover art: resize to max 800×800, flatten transparency, encode as JPEG 85%
///
/// This standardizes all cover art for consistent metadata embedding and reduces
/// file sizes for large images while maintaining good visual quality.
pub fn optimize_cover_art(bytes: &[u8]) -> Result<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::ImageReader;
    use std::io::Cursor;

    // Set up reader with resource limits to prevent DoS attacks from large images
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AppError::ImageProcessing(format!("Failed to detect image format: {}", e)))?;

    // Limit max dimensions to prevent memory exhaustion from malicious images
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(COVER_ART_MAX_INPUT_DIMENSION);
    limits.max_image_height = Some(COVER_ART_MAX_INPUT_DIMENSION);
    reader.limits(limits);

    let img = reader
        .decode()
        .map_err(|e| AppError::ImageProcessing(format!("Failed to decode image: {}", e)))?;

    // Resize if needed using thumbnail() which preserves aspect ratio
    let img = if img.width() > COVER_ART_MAX_DIMENSION || img.height() > COVER_ART_MAX_DIMENSION {
        img.thumbnail(COVER_ART_MAX_DIMENSION, COVER_ART_MAX_DIMENSION)
    } else {
        img
    };

    // Flatten transparency to white background and convert to RGB
    let rgb_img = flatten_transparency_to_white(img);

    // Encode as JPEG with specified quality
    let mut output = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut output, COVER_ART_JPEG_QUALITY);
    rgb_img
        .write_with_encoder(encoder)
        .map_err(|e| AppError::ImageProcessing(format!("Failed to encode JPEG: {}", e)))?;

    Ok(output)
}

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

#[derive(Debug, Default)]
struct BogonFilteringResolver;

impl BogonFilteringResolver {
    fn new() -> Self {
        Self
    }
}

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

/// Flattens any alpha channel to white background
fn flatten_transparency_to_white(img: image::DynamicImage) -> image::DynamicImage {
    use image::{DynamicImage, ImageBuffer, Rgb, Rgba};

    // If no alpha channel, just convert to RGB8
    if !img.color().has_alpha() {
        return DynamicImage::ImageRgb8(img.to_rgb8());
    }

    // Convert to RGBA8 first to handle all alpha formats uniformly
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let Rgba([r, g, b, a]) = *pixel;
        let alpha = a as f32 / 255.0;
        let bg_channel = 255.0 * (1.0 - alpha);
        let new_r = (r as f32 * alpha + bg_channel) as u8;
        let new_g = (g as f32 * alpha + bg_channel) as u8;
        let new_b = (b as f32 * alpha + bg_channel) as u8;
        rgb_img.put_pixel(x, y, Rgb([new_r, new_g, new_b]));
    }

    DynamicImage::ImageRgb8(rgb_img)
}

#[cfg(test)]
mod metadata_tests;
