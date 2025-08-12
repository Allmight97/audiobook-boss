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
pub fn write_audio_metadata(
    file_path: String,
    metadata: AudiobookMetadata,
) -> Result<()> {
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

    Ok(image_data)
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
                Err(AppError::InvalidInput("Invalid JPEG file format".to_string()))
            }
        }
        "png" => {
            if data.len() >= MIN_PNG_SIZE && data[..PNG_HEADER.len()] == PNG_HEADER {
                Ok(())
            } else {
                Err(AppError::InvalidInput("Invalid PNG file format".to_string()))
            }
        }
        "webp" => {
            if data.len() >= MIN_WEBP_SIZE && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
                Ok(())
            } else {
                Err(AppError::InvalidInput("Invalid WebP file format".to_string()))
            }
        }
        _ => Ok(()),
    }
}


