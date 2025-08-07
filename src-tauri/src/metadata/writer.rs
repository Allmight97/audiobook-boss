//! Metadata writing functionality

use super::AudiobookMetadata;
use crate::errors::{AppError, Result};
use lofty::file::AudioFile;
use lofty::prelude::{Accessor, ItemKey, TagExt, TaggedFileExt};
use lofty::probe::Probe;
use lofty::picture::{Picture, PictureType, MimeType};
use lofty::tag::{Tag, TagItem, ItemValue};
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
    
    let mut tagged_file = Probe::open(path)?
        .read()?;
    
    let tag = tagged_file.primary_tag_mut()
        .ok_or_else(|| AppError::Metadata(
            lofty::error::LoftyError::new(lofty::error::ErrorKind::UnknownFormat)
        ))?;
    
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
    if let Some(author) = &metadata.author {
        tag.set_artist(author.clone());
    }
    if let Some(album) = &metadata.album {
        tag.set_album(album.clone());
    }
    if let Some(narrator) = &metadata.narrator {
        tag.insert(TagItem::new(ItemKey::AlbumArtist, ItemValue::Text(narrator.clone())));
    }
    if let Some(year) = metadata.year {
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
    
    let picture = Picture::new_unchecked(
        PictureType::CoverFront,
        Some(MimeType::Jpeg),
        None,
        cover_data.to_vec(),
    );
    
    tag.push_picture(picture);
    tagged_file.save_to_path(path, Default::default())?;
    
    Ok(())
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
}