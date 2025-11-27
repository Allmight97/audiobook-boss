use crate::errors::{AppError, Result};
use crate::metadata::{read_metadata, write_metadata, AudiobookMetadata};
use std::path::PathBuf;

/// Reads metadata from an audio file
/// Returns metadata as JSON-serializable struct
#[tauri::command]
pub fn read_audio_metadata(file_path: String) -> Result<AudiobookMetadata> {
    read_metadata(&file_path)
}

/// Writes metadata to an existing M4B file
/// Accepts file path and metadata object
#[tauri::command]
pub fn write_audio_metadata(file_path: String, metadata: AudiobookMetadata) -> Result<()> {
    write_metadata(&file_path, &metadata)
}

/// Writes cover art to an M4B file
/// Accepts file path and base64-encoded image data
#[tauri::command]
pub fn write_cover_art(file_path: String, cover_data: Vec<u8>) -> Result<()> {
    use crate::metadata::writer::write_cover_art as write_cover;
    write_cover(&file_path, &cover_data)
}

/// Loads image file from disk and returns as byte array
/// Supports common image formats: jpg, jpeg, png, webp
#[tauri::command]
pub async fn load_cover_art_file(file_path: String) -> Result<Vec<u8>> {
    use std::fs;

    let path = PathBuf::from(&file_path);

    // Validate file exists
    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "Image file not found: {file_path}"
        )));
    }

    if !path.is_file() {
        return Err(AppError::FileValidation(format!(
            "Path is not a file: {file_path}"
        )));
    }

    // Validate file extension
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| AppError::InvalidInput("File has no extension".to_string()))?;

    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "webp" => {}
        _ => {
            return Err(AppError::InvalidInput(format!(
                "Unsupported image format: {extension}. Supported formats: jpg, jpeg, png, webp"
            )))
        }
    }

    // Read file contents
    let image_data = fs::read(&path).map_err(AppError::Io)?;

    // Validate it's not empty
    if image_data.is_empty() {
        return Err(AppError::InvalidInput(
            "Image file appears to be empty".to_string(),
        ));
    }

    // Basic format validation by checking file headers
    validate_image_format(&image_data, &extension)?;

    // Optimize cover art: resize, flatten transparency, convert to JPEG
    let optimized = optimize_cover_art(image_data)?;

    Ok(optimized)
}

/// Validates image format by checking file headers
fn validate_image_format(data: &[u8], extension: &str) -> Result<()> {
    use crate::audio::constants::{
        JPEG_HEADER, MIN_IMAGE_SIZE, MIN_PNG_SIZE, MIN_WEBP_SIZE, PNG_HEADER,
    };

    if data.len() < MIN_IMAGE_SIZE {
        return Err(AppError::InvalidInput(
            "Image file too small to validate".to_string(),
        ));
    }

    match extension {
        "jpg" | "jpeg" => {
            if data.len() >= JPEG_HEADER.len() && data[..JPEG_HEADER.len()] == JPEG_HEADER {
                Ok(())
            } else {
                Err(AppError::InvalidInput(
                    "Invalid JPEG file format".to_string(),
                ))
            }
        }
        "png" => {
            if data.len() >= MIN_PNG_SIZE && data[..PNG_HEADER.len()] == PNG_HEADER {
                Ok(())
            } else {
                Err(AppError::InvalidInput(
                    "Invalid PNG file format".to_string(),
                ))
            }
        }
        "webp" => {
            if data.len() >= MIN_WEBP_SIZE && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
                Ok(())
            } else {
                Err(AppError::InvalidInput(
                    "Invalid WebP file format".to_string(),
                ))
            }
        }
        _ => Ok(()),
    }
}

/// Maximum dimension for cover art (width or height)
const COVER_ART_MAX_DIMENSION: u32 = 800;
/// JPEG quality for cover art (0-100)
const COVER_ART_JPEG_QUALITY: u8 = 85;

/// Optimizes cover art: resize to max 800×800, flatten transparency, encode as JPEG 85%
///
/// This standardizes all cover art for consistent metadata embedding and reduces
/// file sizes for large images while maintaining good visual quality.
pub fn optimize_cover_art(bytes: Vec<u8>) -> Result<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::imageops::FilterType;
    use image::ImageReader;
    use std::io::Cursor;

    // Decode image from bytes
    let img = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| AppError::ImageProcessing(format!("Failed to detect image format: {}", e)))?
        .decode()
        .map_err(|e| AppError::ImageProcessing(format!("Failed to decode image: {}", e)))?;

    // Resize if needed (fit within max dimensions, preserve aspect ratio)
    let img = if img.width() > COVER_ART_MAX_DIMENSION || img.height() > COVER_ART_MAX_DIMENSION {
        img.resize(
            COVER_ART_MAX_DIMENSION,
            COVER_ART_MAX_DIMENSION,
            FilterType::Lanczos3,
        )
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

    match img {
        DynamicImage::ImageRgba8(rgba) => {
            let (width, height) = rgba.dimensions();
            let mut rgb: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
            for (x, y, pixel) in rgba.enumerate_pixels() {
                let Rgba([r, g, b, a]) = *pixel;
                let alpha = a as f32 / 255.0;
                let new_r = (r as f32 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_g = (g as f32 * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_b = (b as f32 * alpha + 255.0 * (1.0 - alpha)) as u8;
                rgb.put_pixel(x, y, Rgb([new_r, new_g, new_b]));
            }
            DynamicImage::ImageRgb8(rgb)
        }
        DynamicImage::ImageRgba16(rgba) => {
            let (width, height) = rgba.dimensions();
            let mut rgb: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
            for (x, y, pixel) in rgba.enumerate_pixels() {
                let alpha = pixel[3] as f32 / 65535.0;
                let new_r = ((pixel[0] as f32 / 257.0) * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_g = ((pixel[1] as f32 / 257.0) * alpha + 255.0 * (1.0 - alpha)) as u8;
                let new_b = ((pixel[2] as f32 / 257.0) * alpha + 255.0 * (1.0 - alpha)) as u8;
                rgb.put_pixel(x, y, Rgb([new_r, new_g, new_b]));
            }
            DynamicImage::ImageRgb8(rgb)
        }
        // For non-RGBA images, just convert to RGB8
        _ => DynamicImage::ImageRgb8(img.to_rgb8()),
    }
}
