//! Metadata reading functionality

use super::AudiobookMetadata;
use crate::errors::{AppError, Result};
use lofty::prelude::{Accessor, ItemKey, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Tag, TagType};
use std::path::Path;

// Preferred tag types when multiple tags exist (highest priority first).
const TAG_READ_PRIORITY: &[TagType] = &[
    TagType::Id3v2,
    TagType::Mp4Ilst,
    TagType::Ape,
    TagType::VorbisComments,
    TagType::RiffInfo,
    TagType::Id3v1,
];

/// Reads metadata from an audio file
pub fn read_metadata<P: AsRef<Path>>(file_path: P) -> Result<AudiobookMetadata> {
    let path = file_path.as_ref();

    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            path.display()
        )));
    }

    let tagged_file = Probe::open(path)?.read()?;

    let mut metadata = AudiobookMetadata::new();

    // Merge tags in priority order so that metadata from externally edited tags (e.g., MP3Tag)
    // is preferred over stale atoms we previously wrote.
    let tags = tagged_file.tags();
    for tag_type in TAG_READ_PRIORITY {
        if let Some(tag) = tags.iter().rev().find(|t| t.tag_type() == *tag_type) {
            merge_tag_data(tag, &mut metadata);
        }
    }

    // If nothing was populated (rare), fall back to the primary/first tag.
    if is_metadata_empty(&metadata) {
        if let Some(tag) = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag())
        {
            merge_tag_data(tag, &mut metadata);
        }
    }

    Ok(metadata)
}

/// Extracts data from a tag into the metadata struct without overwriting
/// fields already set by higher-priority tags.
///
/// Maps tags according to audiobook conventions for Plex/Audiobookshelf:
/// - Artist (©ART) = Author
/// - Composer (©wrt) = Narrator
/// - MovementName (©mvn/MVNM) = Series
/// - MovementIndex (©mvi/MVIN) = Book #
pub fn merge_tag_data(tag: &Tag, metadata: &mut AudiobookMetadata) {
    // Basic fields
    if metadata.title.is_none() {
        metadata.title = tag.title().filter(|s| !s.is_empty()).map(|s| s.to_string());
    }
    if metadata.artist.is_none() {
        metadata.artist = tag
            .artist()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
    }
    if metadata.album.is_none() {
        metadata.album = tag.album().filter(|s| !s.is_empty()).map(|s| s.to_string());
    }
    if metadata.date.is_none() {
        metadata.date = tag.year();
    }
    if metadata.genre.is_none() {
        metadata.genre = tag.genre().filter(|s| !s.is_empty()).map(|s| s.to_string());
    }

    // Narrator from Composer field (©wrt) - NOT AlbumArtist
    if metadata.composer.is_none() {
        if let Some(item) = tag.get(&ItemKey::Composer) {
            let value = item.value().text().unwrap_or("").trim();
            if !value.is_empty() {
                metadata.composer = Some(value.to_string());
            }
        }
    }

    // Series: prefer freeform SERIES atom (MP3tag/external edits) over standard Movement atom
    if metadata.series.is_none() {
        // Try freeform SERIES atom first (written by MP3tag and other external tools)
        if let Some(item) = tag.get(&ItemKey::Unknown(
            "----:com.apple.iTunes:SERIES".to_string(),
        )) {
            let value = item.value().text().unwrap_or("").trim();
            if !value.is_empty() {
                metadata.series = Some(value.to_string());
            }
        }
        // Fallback to standard Movement atom (©mvn/MVNM)
        if metadata.series.is_none() {
            if let Some(item) = tag.get(&ItemKey::Movement) {
                let value = item.value().text().unwrap_or("").trim();
                if !value.is_empty() {
                    metadata.series = Some(value.to_string());
                }
            }
        }
    }

    // Book #: prefer freeform SERIES-PART atom over standard MovementNumber atom
    if metadata.series_part.is_none() {
        // Try freeform SERIES-PART atom first (written by MP3tag and other external tools)
        if let Some(item) = tag.get(&ItemKey::Unknown(
            "----:com.apple.iTunes:SERIES-PART".to_string(),
        )) {
            let value = item.value().text().unwrap_or("").trim();
            if !value.is_empty() {
                metadata.series_part = Some(value.to_string());
            }
        }
        // Fallback to standard MovementNumber atom (©mvi/MVIN)
        if metadata.series_part.is_none() {
            if let Some(item) = tag.get(&ItemKey::MovementNumber) {
                let value = item.value().text().unwrap_or("").trim();
                if !value.is_empty() {
                    metadata.series_part = Some(value.to_string());
                }
            }
        }
    }

    // Description from dedicated description field (no comment fallback)
    if metadata.description.is_none() {
        if let Some(item) = tag.get(&ItemKey::Description) {
            let value = item.value().text().unwrap_or("").trim();
            if !value.is_empty() {
                metadata.description = Some(value.to_string());
            }
        }
    }

    // Album sort order (TSOA/soal) when present
    if metadata.album_sort.is_none() {
        if let Some(item) = tag.get(&ItemKey::AlbumTitleSortOrder) {
            let value = item.value().text().unwrap_or("").trim();
            if !value.is_empty() {
                metadata.album_sort = Some(value.to_string());
            }
        }
    }

    // Extract cover art and optimize it
    if metadata.cover_art.is_none() {
        let pictures = tag.pictures();
        if let Some(picture) = pictures.first() {
            let raw_data = picture.data();
            // Optimize cover art: resize to max 800×800, flatten transparency, JPEG 85%
            match crate::commands::metadata::optimize_cover_art(raw_data) {
                Ok(optimized) => metadata.cover_art = Some(optimized),
                Err(e) => {
                    // Log warning but don't fail - use raw data as fallback
                    log::warn!(
                        "Failed to optimize auto-loaded cover art: {}. Using original.",
                        e
                    );
                    metadata.cover_art = Some(raw_data.to_vec());
                }
            }
        }
    }
}

fn is_metadata_empty(metadata: &AudiobookMetadata) -> bool {
    metadata.title.is_none()
        && metadata.artist.is_none()
        && metadata.album.is_none()
        && metadata.composer.is_none()
        && metadata.genre.is_none()
        && metadata.date.is_none()
        && metadata.description.is_none()
        && metadata.series.is_none()
        && metadata.series_part.is_none()
        && metadata.album_sort.is_none()
        && metadata.cover_art.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::tag::{ItemValue, TagItem};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_nonexistent_file() {
        let result = read_metadata("nonexistent.m4b");
        assert!(matches!(result, Err(AppError::FileValidation(_))));
    }

    #[test]
    fn test_read_metadata_empty_file() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("empty.txt");
        fs::write(&file_path, b"").expect("write empty file");

        let result = read_metadata(&file_path);
        assert!(matches!(result, Err(AppError::Metadata(_))));
    }

    #[test]
    fn test_merge_prefers_higher_priority_tag() {
        let mut mp4_tag = Tag::new(TagType::Mp4Ilst);
        mp4_tag.set_title("Old Title".to_string());
        mp4_tag.set_artist("Old Author".to_string());

        let mut id3_tag = Tag::new(TagType::Id3v2);
        id3_tag.set_title("New Title".to_string());
        id3_tag.set_artist("New Author".to_string());

        let tags = [mp4_tag, id3_tag];
        let mut metadata = AudiobookMetadata::new();

        for tag_type in TAG_READ_PRIORITY {
            if let Some(tag) = tags.iter().find(|t| t.tag_type() == *tag_type) {
                merge_tag_data(tag, &mut metadata);
            }
        }

        assert_eq!(metadata.title.as_deref(), Some("New Title"));
        assert_eq!(metadata.artist.as_deref(), Some("New Author"));
    }

    #[test]
    fn test_merge_fills_missing_fields_from_lower_priority() {
        let mut id3_tag = Tag::new(TagType::Id3v2);
        id3_tag.set_title("Title".to_string());

        let mut mp4_tag = Tag::new(TagType::Mp4Ilst);
        mp4_tag.insert(TagItem::new(
            ItemKey::Movement,
            ItemValue::Text("Series".to_string()),
        ));

        let tags = [id3_tag, mp4_tag];
        let mut metadata = AudiobookMetadata::new();

        for tag_type in TAG_READ_PRIORITY {
            if let Some(tag) = tags.iter().find(|t| t.tag_type() == *tag_type) {
                merge_tag_data(tag, &mut metadata);
            }
        }

        assert_eq!(metadata.title.as_deref(), Some("Title"));
        assert_eq!(metadata.series.as_deref(), Some("Series"));
    }
}
