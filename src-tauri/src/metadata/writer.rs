//! Metadata writing functionality

use super::AudiobookMetadata;
use crate::errors::{AppError, Result};
use lofty::file::AudioFile;
use lofty::prelude::{Accessor, ItemKey, TagExt, TaggedFileExt};
use lofty::probe::Probe;
use lofty::picture::{Picture, PictureType, MimeType};
use lofty::tag::{Tag, TagItem, ItemValue, TagType};
use std::path::Path;

/// Writes metadata to an existing M4B file
pub fn write_metadata<P: AsRef<Path>>(
    file_path: P,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    let path = file_path.as_ref();
    
    if !path.exists() {
        return Err(AppError::FileValidation(
            format!("File not found: {}", path.display())
        ));
    }
    
    let mut tagged_file = Probe::open(path)?.read()?;

    // Ensure a primary tag exists – some freshly muxed MP4/M4B files may have
    // no tag atoms yet, in which case Lofty returns None. We create an MP4 iTunes
    // list (MP4ILST) tag so metadata writing does not abort the finalize stage
    // leaving the temp output un‑moved.
    if tagged_file.primary_tag().is_none() {
    log::debug!("No primary tag present – creating new Mp4Ilst tag");
    tagged_file.insert_tag(Tag::new(TagType::Mp4Ilst));
    }
    let tag = tagged_file.primary_tag_mut().ok_or_else(|| {
        AppError::Metadata(lofty::error::LoftyError::new(
            lofty::error::ErrorKind::UnknownFormat,
        ))
    })?;
    
    update_tag_data(tag, metadata)?;
    tagged_file.save_to_path(path, Default::default())?;
    
    Ok(())
}

/// Updates tag data from metadata struct
fn update_tag_data(tag: &mut Tag, metadata: &AudiobookMetadata) -> Result<()> {
    // Clear existing metadata
    tag.clear();
    
    // Set basic metadata
    if let Some(title) = &metadata.title {
        tag.set_title(title.clone());
    }
    if let Some(author) = &metadata.artist {
        tag.set_artist(author.clone());
    }
    if let Some(album) = &metadata.album {
        tag.set_album(album.clone());
    }
    if let Some(narrator) = &metadata.composer {
        tag.insert(TagItem::new(ItemKey::AlbumArtist, ItemValue::Text(narrator.clone())));
    }
    if let Some(year) = metadata.date {
        tag.set_year(year);
    }
    if let Some(genre) = &metadata.genre {
        tag.set_genre(genre.clone());
    }
    if let Some(description) = &metadata.description {
        tag.set_comment(description.clone());
    }
    
    Ok(())
}

/// Writes cover art to an M4B file
pub fn write_cover_art<P: AsRef<Path>>(
    file_path: P,
    cover_data: &[u8],
) -> Result<()> {
    let path = file_path.as_ref();
    
    if !path.exists() {
        return Err(AppError::FileValidation(
            format!("File not found: {}", path.display())
        ));
    }
    
    let mut tagged_file = Probe::open(path)?
        .read()?;
    
    let tag = tagged_file.primary_tag_mut()
        .ok_or_else(|| AppError::Metadata(
            lofty::error::LoftyError::new(lofty::error::ErrorKind::UnknownFormat)
        ))?;
    
    // Detect the correct MIME type based on image format
    let mime_type = detect_image_mime_type(cover_data)?;
    
    let picture = Picture::new_unchecked(
        PictureType::CoverFront,
        Some(mime_type),
        None,
        cover_data.to_vec(),
    );
    
    tag.push_picture(picture);
    tagged_file.save_to_path(path, Default::default())?;
    
    Ok(())
}

/// Detects MIME type from image data headers
fn detect_image_mime_type(data: &[u8]) -> Result<MimeType> {
    if data.len() < 8 {
        return Err(AppError::InvalidInput("Image data too small to determine format".to_string()));
    }
    
    // Check for JPEG (starts with FF D8 FF)
    if data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
        return Ok(MimeType::Jpeg);
    }
    
    // Check for PNG (starts with 89 50 4E 47 0D 0A 1A 0A)
    if data.len() >= 8 && data[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Ok(MimeType::Png);
    }
    
    // Check for GIF (GIF87a or GIF89a)
    if data.len() >= 6 && (&data[0..6] == b"GIF87a" || &data[0..6] == b"GIF89a") {
        return Ok(MimeType::Gif);
    }
    
    // For WebP and other formats, default to JPEG for compatibility
    // (lofty may not support all MIME types we want to detect)
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        log::info!("WebP format detected, using JPEG MIME type for compatibility");
        return Ok(MimeType::Jpeg);
    }
    
    // Default to JPEG for unknown formats (maintains backward compatibility)
    log::warn!("Unknown image format, defaulting to JPEG MIME type");
    Ok(MimeType::Jpeg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_write_to_nonexistent_file() {
        let metadata = AudiobookMetadata::new();
        let result = write_metadata("nonexistent.m4b", &metadata);
        assert!(matches!(result, Err(AppError::FileValidation(_))));
    }

    #[test]
    fn test_write_cover_to_nonexistent_file() {
        let cover_data = vec![0u8; 100];
        let result = write_cover_art("nonexistent.m4b", &cover_data);
        assert!(matches!(result, Err(AppError::FileValidation(_))));
    }

    #[test]
    fn test_write_metadata_invalid_file() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("invalid.txt");
        fs::write(&file_path, b"not audio").expect("write temp file");
        
        let metadata = AudiobookMetadata::new();
        let result = write_metadata(&file_path, &metadata);
        assert!(matches!(result, Err(AppError::Metadata(_))));
    }

    #[test]
    fn test_detect_image_mime_type() {
        // Test JPEG detection
        let jpeg_data = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";
        let mime_type = detect_image_mime_type(jpeg_data).expect("Should detect JPEG");
        assert!(matches!(mime_type, MimeType::Jpeg));

        // Test PNG detection
        let png_data = b"\x89PNG\r\n\x1A\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xDE";
        let mime_type = detect_image_mime_type(png_data).expect("Should detect PNG");
        assert!(matches!(mime_type, MimeType::Png));

        // Test WebP detection (should default to JPEG for compatibility)
        let webp_data = b"RIFF\x00\x00\x00\x00WEBP\x00\x00\x00\x00";
        let mime_type = detect_image_mime_type(webp_data).expect("Should detect WebP");
        assert!(matches!(mime_type, MimeType::Jpeg)); // WebP defaults to JPEG for compatibility

        // Test GIF detection
        let gif_data = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
        let mime_type = detect_image_mime_type(gif_data).expect("Should detect GIF");
        assert!(matches!(mime_type, MimeType::Gif));

        // Test unknown format (should default to JPEG)
        let unknown_data = b"UNKNOWN_FORMAT_DATA";
        let mime_type = detect_image_mime_type(unknown_data).expect("Should default to JPEG");
        assert!(matches!(mime_type, MimeType::Jpeg));

        // Test insufficient data
        let short_data = b"123";
        let result = detect_image_mime_type(short_data);
        assert!(result.is_err());
    }
}