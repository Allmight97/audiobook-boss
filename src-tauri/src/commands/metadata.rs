use crate::audio::path_validation::{validate_input_audio_path, validate_input_image_path};
use crate::errors::{AppError, Result};
use crate::metadata::{read_metadata, AudiobookMetadata};
use std::path::PathBuf;

/// Reads metadata from an audio file
/// Returns metadata as JSON-serializable struct
#[tauri::command]
pub fn read_audio_metadata(file_path: String) -> Result<AudiobookMetadata> {
    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_audio_path(&path)?;
    read_metadata(validated_path.to_string_lossy().as_ref())
}

/// Saves metadata to an audio file with TSOA computation (metadata-only editing)
///
/// This command is designed for metadata-only editing (Cmd+S workflow):
/// 1. Computes TSOA (Album Sort) from series + series_part + title
/// 2. Writes metadata non-destructively (preserves existing cover art if not replaced)
/// 3. Handles cover art: preserves existing if not provided, replaces if new art given
#[tauri::command]
pub fn save_metadata_to_file(file_path: String, metadata: AudiobookMetadata) -> Result<()> {
    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_audio_path(&path)?;

    // Compute TSOA (Album Sort) if series and title are present
    let mut metadata_with_tsoa = metadata;
    if let (Some(series), Some(title)) = (&metadata_with_tsoa.series, &metadata_with_tsoa.title) {
        metadata_with_tsoa.album_sort =
            compute_tsoa(series, metadata_with_tsoa.series_part.as_deref(), title);
    }

    if crate::metadata::mp4ameta_bridge::is_mp4_container(&validated_path) {
        crate::metadata::mp4ameta_bridge::write_metadata(&validated_path, &metadata_with_tsoa)?;
    } else {
        // Re-mux with ffmpeg-next: copy streams, set container metadata, copy chapters and attached_pic
        crate::metadata::ffmpeg_bridge::rewrite_metadata_with_ffmpeg(
            &validated_path,
            &metadata_with_tsoa,
        )?;
    }

    log::info!("Metadata saved to: {}", validated_path.display());
    Ok(())
}

/// Computes TSOA (Album Sort) from series + part + title.
/// Returns None if series_part is missing or cannot be parsed to a positive integer.
pub fn compute_tsoa(series: &str, series_part: Option<&str>, title: &str) -> Option<String> {
    let raw_part = series_part?;

    let part_num = raw_part
        .split('/')
        .next()
        .and_then(|p| p.trim().parse::<u32>().ok())?;

    if part_num == 0 {
        return None;
    }

    if series.is_empty() || title.is_empty() {
        return None;
    }

    Some(format!("{} {:02} - {}", series, part_num, title))
}

/// Writes cover art to an M4B file
/// Accepts file path and base64-encoded image data
#[tauri::command]
pub fn write_cover_art(file_path: String, cover_data: Vec<u8>) -> Result<()> {
    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_audio_path(&path)?;
    if crate::metadata::mp4ameta_bridge::is_mp4_container(&validated_path) {
        crate::metadata::mp4ameta_bridge::write_metadata(
            &validated_path,
            &AudiobookMetadata {
                cover_art: Some(cover_data),
                ..Default::default()
            },
        )?;
    } else {
        crate::metadata::ffmpeg_bridge::rewrite_metadata_with_ffmpeg(
            &validated_path,
            &AudiobookMetadata {
                cover_art: Some(cover_data),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

/// Loads image file from disk and returns as byte array
/// Supports common image formats: jpg, jpeg, png, webp
#[tauri::command]
pub async fn load_cover_art_file(file_path: String) -> Result<Vec<u8>> {
    use std::fs;

    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_image_path(&path)?;

    // Read file contents using the validated canonical path
    let image_data = fs::read(&validated_path).map_err(AppError::Io)?;

    // Validate it's not empty
    if image_data.is_empty() {
        return Err(AppError::InvalidInput(
            "Image file appears to be empty".to_string(),
        ));
    }

    // Optimize cover art: resize, flatten transparency, convert to JPEG
    // Format validation is handled by with_guessed_format() in optimize_cover_art (#32)
    let optimized = optimize_cover_art(&image_data)?;

    Ok(optimized)
}

/// Maximum dimension for cover art (width or height)
const COVER_ART_MAX_DIMENSION: u32 = 800;
/// JPEG quality for cover art (0-100)
const COVER_ART_JPEG_QUALITY: u8 = 85;

/// Maximum input image dimension allowed (DoS prevention)
const COVER_ART_MAX_INPUT_DIMENSION: u32 = 4096;

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
