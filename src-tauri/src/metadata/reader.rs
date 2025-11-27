//! Metadata reading functionality

use super::AudiobookMetadata;
use crate::errors::{AppError, Result};
use lofty::prelude::{Accessor, ItemKey, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Tag;
use std::path::Path;

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

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let mut metadata = AudiobookMetadata::new();

    if let Some(tag) = tag {
        extract_tag_data(tag, &mut metadata);
    }

    Ok(metadata)
}

/// Extracts data from a tag into the metadata struct
fn extract_tag_data(tag: &Tag, metadata: &mut AudiobookMetadata) {
    metadata.title = tag.title().map(|s| s.to_string());
    metadata.artist = tag.artist().map(|s| s.to_string());
    metadata.album = tag.album().map(|s| s.to_string());
    if let Some(item) = tag.get(&ItemKey::AlbumArtist) {
        metadata.composer = Some(item.value().text().unwrap_or("").to_string());
    }
    metadata.date = tag.year();
    metadata.genre = tag.genre().map(|s| s.to_string());

    // Extract description from comment
    metadata.description = tag.comment().map(|s| s.to_string());

    // Extract cover art and optimize it
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
