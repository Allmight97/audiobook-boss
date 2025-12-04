//! Metadata writing functionality

use super::AudiobookMetadata;
use crate::errors::{AppError, Result};
use lofty::file::AudioFile;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::{Accessor, ItemKey, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{ItemValue, Tag, TagItem, TagType};
use std::path::Path;

/// Returns the tag types we should write for the given file based on its extension.
/// Keeps read/write symmetry so external editors (e.g., Mp3tag) see the same data.
fn tag_types_for_path(path: &Path) -> &'static [TagType] {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match ext.as_deref() {
        // MPEG audio
        Some("mp3") => &[TagType::Id3v2, TagType::Id3v1, TagType::Ape],
        // MP4 family (m4a/m4b/mp4)
        Some("m4a") | Some("m4b") | Some("mp4") => &[TagType::Mp4Ilst],
        // FLAC / Ogg / Opus
        Some("flac") | Some("ogg") | Some("opus") => &[TagType::VorbisComments],
        // WAV / AIFF metadata blocks
        Some("wav") => &[TagType::RiffInfo],
        Some("aif") | Some("aiff") => &[TagType::AiffText],
        // Fallback to ID3v2 for unknown extensions we still allow
        _ => &[TagType::Id3v2],
    }
}

/// Writes metadata to an existing audio file (non-destructive)
///
/// This function preserves existing cover art and unknown atoms unless
/// explicitly overwritten by the provided metadata.
pub fn write_metadata<P: AsRef<Path>>(file_path: P, metadata: &AudiobookMetadata) -> Result<()> {
    let path = file_path.as_ref();

    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            path.display()
        )));
    }

    let mut tagged_file = Probe::open(path)?.read()?;

    let target_tag_types = tag_types_for_path(path);

    // Ensure a compatible tag exists; create the first matching type if needed.
    let has_target_tag = tagged_file
        .tags()
        .iter()
        .any(|tag| target_tag_types.contains(&tag.tag_type()));

    if !has_target_tag {
        let default_tag = *target_tag_types.first().unwrap_or(&TagType::Id3v2);
        log::debug!(
            "No target tag present – creating new {:?} tag for {}",
            default_tag,
            path.display()
        );
        tagged_file.insert_tag(Tag::new(default_tag));
    }

    let mut updated_any = false;

    for tag_type in target_tag_types {
        if let Some(tag) = tagged_file.tag_mut(*tag_type) {
            update_tag_data(tag, metadata)?;
            apply_cover_art(tag, metadata)?;
            updated_any = true;
        }
    }

    // Fallback: if we somehow still didn't touch a tag, update the primary tag.
    if !updated_any {
        let tag = tagged_file.primary_tag_mut().ok_or_else(|| {
            AppError::Metadata(lofty::error::LoftyError::new(
                lofty::error::ErrorKind::UnknownFormat,
            ))
        })?;
        update_tag_data(tag, metadata)?;
        apply_cover_art(tag, metadata)?;
    }

    tagged_file.save_to_path(path, Default::default())?;

    Ok(())
}

/// Updates tag data from metadata struct
///
/// Maps audiobook fields to tags for Plex/Audiobookshelf compatibility:
/// - Artist (©ART) + AlbumArtist (aART) = Author
/// - Composer (©wrt) = Narrator (also mirrored to freeform NARRATOR)
/// - MovementName (©mvn) = Series (also mirrored to freeform SERIES)
/// - MovementIndex (©mvi) = Book # (also mirrored to freeform SERIES-PART)
/// - AlbumTitleSortOrder (soal) = TSOA for library sorting
///
/// When a field is None, the corresponding tag is removed from the file.
/// This allows users to clear fields by leaving them empty in the UI.
pub fn update_tag_data(tag: &mut Tag, metadata: &AudiobookMetadata) -> Result<()> {
    // NOTE: We do NOT call tag.clear() to preserve unknown atoms and cover art.
    // Instead, we explicitly set or remove each field based on whether it has a value.
    let tag_type = tag.tag_type();

    // Basic metadata - Title (©nam)
    if let Some(title) = &metadata.title {
        tag.set_title(title.clone());
    } else {
        tag.remove_title();
    }

    // Album (©alb) - typically same as title for audiobooks
    if let Some(album) = &metadata.album {
        tag.set_album(album.clone());
    } else {
        tag.remove_album();
    }

    // Author → Artist (©ART) + AlbumArtist (aART)
    if let Some(author) = &metadata.artist {
        tag.set_artist(author.clone());
        // Also set AlbumArtist for library grouping
        tag.insert(TagItem::new(
            ItemKey::AlbumArtist,
            ItemValue::Text(author.clone()),
        ));
    } else {
        tag.remove_artist();
        tag.remove_key(&ItemKey::AlbumArtist);
    }

    // Narrator → Composer (©wrt) - NOT AlbumArtist!
    if let Some(narrator) = &metadata.composer {
        tag.insert(TagItem::new(
            ItemKey::Composer,
            ItemValue::Text(narrator.clone()),
        ));
    } else {
        tag.remove_key(&ItemKey::Composer);
    }

    // Year (©day)
    if let Some(year) = metadata.date {
        tag.set_year(year);
    } else {
        tag.remove_year();
    }

    // Genre (©gen)
    if let Some(genre) = &metadata.genre {
        tag.set_genre(genre.clone());
    } else {
        tag.remove_genre();
    }

    // Description (desc) - long synopsis (no comment mirroring)
    if let Some(description) = &metadata.description {
        tag.insert(TagItem::new(
            ItemKey::Description,
            ItemValue::Text(description.clone()),
        ));
    } else {
        tag.remove_key(&ItemKey::Description);
    }

    // Series → Movement (©mvn/MVNM)
    if let Some(series) = &metadata.series {
        tag.insert(TagItem::new(
            ItemKey::Movement,
            ItemValue::Text(series.clone()),
        ));
        // Also set ShowName for compatibility with some players
        tag.insert(TagItem::new(
            ItemKey::ShowName,
            ItemValue::Text(series.clone()),
        ));

        // Mirror to format-specific freeform fields to improve compatibility with external editors
        match tag_type {
            TagType::Id3v2 => {
                // Mp3tag uses TXXX:SERIES for series
                tag.insert(TagItem::new(
                    ItemKey::Unknown("SERIES".to_string()),
                    ItemValue::Text(series.clone()),
                ));
            }
            TagType::Mp4Ilst => {
                // Apple-style freeform atom used by MP3tag on MP4/M4B
                tag.insert(TagItem::new(
                    ItemKey::Unknown("----:com.apple.iTunes:SERIES".to_string()),
                    ItemValue::Text(series.clone()),
                ));
            }
            TagType::VorbisComments => {
                tag.insert(TagItem::new(
                    ItemKey::Unknown("SERIES".to_string()),
                    ItemValue::Text(series.clone()),
                ));
            }
            _ => {}
        }
    } else {
        tag.remove_key(&ItemKey::Movement);
        tag.remove_key(&ItemKey::ShowName);
        match tag_type {
            TagType::Id3v2 => {
                tag.remove_key(&ItemKey::Unknown("SERIES".to_string()));
            }
            TagType::Mp4Ilst => {
                tag.remove_key(&ItemKey::Unknown(
                    "----:com.apple.iTunes:SERIES".to_string(),
                ));
            }
            TagType::VorbisComments => {
                tag.remove_key(&ItemKey::Unknown("SERIES".to_string()));
            }
            _ => {}
        }
    }

    // Book # → MovementNumber (©mvi/MVIN)
    if let Some(series_part) = &metadata.series_part {
        tag.insert(TagItem::new(
            ItemKey::MovementNumber,
            ItemValue::Text(series_part.clone()),
        ));

        match tag_type {
            TagType::Id3v2 => {
                // Mp3tag uses TXXX:SERIES-PART for book number
                tag.insert(TagItem::new(
                    ItemKey::Unknown("SERIES-PART".to_string()),
                    ItemValue::Text(series_part.clone()),
                ));
            }
            TagType::Mp4Ilst => {
                tag.insert(TagItem::new(
                    ItemKey::Unknown("----:com.apple.iTunes:SERIES-PART".to_string()),
                    ItemValue::Text(series_part.clone()),
                ));
            }
            TagType::VorbisComments => {
                tag.insert(TagItem::new(
                    ItemKey::Unknown("SERIES-PART".to_string()),
                    ItemValue::Text(series_part.clone()),
                ));
            }
            _ => {}
        }
    } else {
        tag.remove_key(&ItemKey::MovementNumber);
        match tag_type {
            TagType::Id3v2 => {
                tag.remove_key(&ItemKey::Unknown("SERIES-PART".to_string()));
            }
            TagType::Mp4Ilst => {
                tag.remove_key(&ItemKey::Unknown(
                    "----:com.apple.iTunes:SERIES-PART".to_string(),
                ));
            }
            TagType::VorbisComments => {
                tag.remove_key(&ItemKey::Unknown("SERIES-PART".to_string()));
            }
            _ => {}
        }
    }

    // TSOA → AlbumTitleSortOrder (soal) for library sorting
    if let Some(album_sort) = &metadata.album_sort {
        tag.insert(TagItem::new(
            ItemKey::AlbumTitleSortOrder,
            ItemValue::Text(album_sort.clone()),
        ));
    } else {
        tag.remove_key(&ItemKey::AlbumTitleSortOrder);
    }

    Ok(())
}

fn apply_cover_art(tag: &mut Tag, metadata: &AudiobookMetadata) -> Result<()> {
    if let Some(cover_data) = &metadata.cover_art {
        if cover_data.is_empty() {
            tag.remove_picture_type(PictureType::CoverFront);
            log::debug!("Cover art removed per request");
        } else {
            // Remove existing cover art before adding new one
            tag.remove_picture_type(PictureType::CoverFront);

            let mime_type = detect_image_mime_type(cover_data)?;
            let picture = Picture::new_unchecked(
                PictureType::CoverFront,
                Some(mime_type),
                None,
                cover_data.clone(),
            );
            tag.push_picture(picture);
            log::debug!("Cover art updated ({} bytes)", cover_data.len());
        }
    }

    // If metadata.cover_art is None, existing cover art is preserved
    Ok(())
}

/// Writes cover art to an M4B file
pub fn write_cover_art<P: AsRef<Path>>(file_path: P, cover_data: &[u8]) -> Result<()> {
    let path = file_path.as_ref();

    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            path.display()
        )));
    }

    let mut tagged_file = Probe::open(path)?.read()?;

    let tag = tagged_file.primary_tag_mut().ok_or_else(|| {
        AppError::Metadata(lofty::error::LoftyError::new(
            lofty::error::ErrorKind::UnknownFormat,
        ))
    })?;

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
        return Err(AppError::InvalidInput(
            "Image data too small to determine format".to_string(),
        ));
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
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn picks_id3_for_mp3() {
        let types = tag_types_for_path(std::path::Path::new("sample.MP3"));
        assert_eq!(types, &[TagType::Id3v2, TagType::Id3v1, TagType::Ape]);
    }

    #[test]
    fn picks_mp4_for_m4b() {
        let types = tag_types_for_path(std::path::Path::new("sample.m4b"));
        assert_eq!(types, &[TagType::Mp4Ilst]);
    }

    #[test]
    fn picks_vorbis_for_flac() {
        let types = tag_types_for_path(std::path::Path::new("sample.flac"));
        assert_eq!(types, &[TagType::VorbisComments]);
    }

    #[test]
    fn falls_back_to_id3_for_unknown_extension() {
        let types = tag_types_for_path(std::path::Path::new("sample.xyz"));
        assert_eq!(types, &[TagType::Id3v2]);
    }

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
        let jpeg_data =
            b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";
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
