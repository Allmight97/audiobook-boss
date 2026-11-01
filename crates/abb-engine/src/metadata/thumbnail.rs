use super::cover_art::format::{detect_cover_art_format, detect_image_dimensions, CoverFormat};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::io::Cursor;
use std::path::Path;

const COVER_ART_MAX_DIMENSION: u32 = 800;
/// Twice the 64 px a thumbnail is shown at, so it stays sharp on Retina.
const THUMBNAIL_MAX_DIMENSION: u32 = 128;
const COVER_ART_JPEG_QUALITY: u8 = 85;
const COVER_ART_MAX_INPUT_DIMENSION: u32 = 4096;
const THUMBNAIL_MAX_INPUT_DIMENSION: u32 = 4096;
pub(crate) const THUMBNAIL_MAX_ENCODED_BYTES: usize = 10 * 1024 * 1024;
const THUMBNAIL_MAX_DECODER_ALLOC_BYTES: u64 = 96 * 1024 * 1024;

pub fn read_audio_cover_thumbnail(path: &Path) -> Result<Option<Vec<u8>>> {
    render_cover_thumbnail(read_embedded_cover_bytes(path)?)
}

fn read_embedded_cover_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            sanitize_path_for_display(path)
        )));
    }

    // MP4-family: bound artwork via atom walk without opening FFmpeg (which can
    // materialize a full attached-pic / covr packet during demuxer open).
    if super::mp4_covr::looks_like_mp4_family_file(path)? {
        return super::mp4_covr::read_bounded_mp4_cover_art(path, THUMBNAIL_MAX_ENCODED_BYTES);
    }

    super::embedded_cover::read_bounded_non_mp4_cover_art(path)
}

pub(crate) fn optimize_cover_art(bytes: &[u8]) -> Result<Vec<u8>> {
    encode_jpeg_with_limits(
        bytes,
        COVER_ART_MAX_DIMENSION,
        COVER_ART_MAX_INPUT_DIMENSION,
        image::Limits::default().max_alloc,
    )
}

pub(crate) fn prepare_cover_art_for_write(bytes: &[u8]) -> Result<Vec<u8>> {
    if cover_art_meets_write_target(bytes) {
        log::info!(
            "cover_art_plan decision=passthrough format=jpeg bytes={}",
            bytes.len()
        );
        return Ok(bytes.to_vec());
    }
    let prepared = optimize_cover_art(bytes)?;
    log::info!(
        "cover_art_plan decision=optimize input_bytes={} output_bytes={}",
        bytes.len(),
        prepared.len()
    );
    Ok(prepared)
}

fn cover_art_meets_write_target(bytes: &[u8]) -> bool {
    matches!(detect_cover_art_format(bytes), Some(CoverFormat::Jpeg))
        && detect_image_dimensions(bytes, CoverFormat::Jpeg).is_some_and(|(width, height)| {
            width > 0
                && height > 0
                && (width as u32) <= COVER_ART_MAX_DIMENSION
                && (height as u32) <= COVER_ART_MAX_DIMENSION
        })
}

fn render_cover_thumbnail(cover_art: Option<Vec<u8>>) -> Result<Option<Vec<u8>>> {
    cover_art
        .as_deref()
        .map(render_display_thumbnail)
        .transpose()
}

pub(crate) fn render_display_thumbnail(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure_thumbnail_encoded_size(bytes.len())?;
    encode_jpeg_with_limits(
        bytes,
        THUMBNAIL_MAX_DIMENSION,
        THUMBNAIL_MAX_INPUT_DIMENSION,
        Some(THUMBNAIL_MAX_DECODER_ALLOC_BYTES),
    )
}

fn ensure_thumbnail_encoded_size(byte_len: usize) -> Result<()> {
    if byte_len > THUMBNAIL_MAX_ENCODED_BYTES {
        return Err(AppError::ImageProcessing(format!(
            "Embedded cover exceeds the {} MiB thumbnail input limit",
            THUMBNAIL_MAX_ENCODED_BYTES / (1024 * 1024)
        )));
    }
    Ok(())
}

fn encode_jpeg_with_limits(
    bytes: &[u8],
    max_dimension: u32,
    max_input_dimension: u32,
    max_alloc: Option<u64>,
) -> Result<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::ImageReader;

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| {
            AppError::ImageProcessing(format!("Failed to detect image format: {error}"))
        })?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_input_dimension);
    limits.max_image_height = Some(max_input_dimension);
    limits.max_alloc = max_alloc;
    reader.limits(limits);

    let image = reader
        .decode()
        .map_err(|error| AppError::ImageProcessing(format!("Failed to decode image: {error}")))?;
    let image = if image.width() > max_dimension || image.height() > max_dimension {
        image.thumbnail(max_dimension, max_dimension)
    } else {
        image
    };
    let image = flatten_transparency_to_white(image);
    let mut output = Vec::new();
    image
        .write_with_encoder(JpegEncoder::new_with_quality(
            &mut output,
            COVER_ART_JPEG_QUALITY,
        ))
        .map_err(|error| AppError::ImageProcessing(format!("Failed to encode JPEG: {error}")))?;
    Ok(output)
}

fn flatten_transparency_to_white(img: image::DynamicImage) -> image::DynamicImage {
    use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
    if !img.color().has_alpha() {
        return DynamicImage::ImageRgb8(img.to_rgb8());
    }
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let Rgba([red, green, blue, alpha]) = *pixel;
        let alpha = f32::from(alpha) / 255.0;
        let background = 255.0 * (1.0 - alpha);
        rgb_img.put_pixel(
            x,
            y,
            Rgb([
                (f32::from(red) * alpha + background) as u8,
                (f32::from(green) * alpha + background) as u8,
                (f32::from(blue) * alpha + background) as u8,
            ]),
        );
    }
    DynamicImage::ImageRgb8(rgb_img)
}

#[cfg(test)]
#[path = "thumbnail_tests.rs"]
mod tests;
