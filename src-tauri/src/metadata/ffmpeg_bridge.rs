//! FFmpeg-Next metadata integration bridge
//!
//! This module provides conversion and integration between our AudiobookMetadata
//! structures and ffmpeg-next metadata APIs, enabling direct metadata embedding
//! during the encoding process.

use super::AudiobookMetadata;
use crate::errors::Result;
use ffmpeg_next as ff;

/// Converts AudiobookMetadata to ffmpeg-next metadata dictionary
/// Maps our standardized metadata fields to container-appropriate metadata keys
pub fn metadata_to_ffmpeg_dict(metadata: &AudiobookMetadata) -> Result<ff::Dictionary<'_>> {
    let mut dict = ff::Dictionary::new();
    
    // Standard audiobook metadata fields
    if let Some(ref title) = metadata.title {
        dict.set("title", title);
    }
    
    if let Some(ref artist) = metadata.artist {
        dict.set("artist", artist);
        dict.set("album_artist", artist); // For audiobooks, artist = album_artist
    }
    
    if let Some(ref album) = metadata.album {
        dict.set("album", album);
    }
    
    if let Some(ref composer) = metadata.composer {
        dict.set("composer", composer);
    }
    
    if let Some(ref genre) = metadata.genre {
        dict.set("genre", genre);
    }
    
    if let Some(date) = metadata.date {
        dict.set("date", &date.to_string());
        dict.set("year", &date.to_string()); // Some containers prefer year
    }
    
    if let Some(ref comment) = metadata.comment {
        dict.set("comment", comment);
    }
    
    if let Some(ref description) = metadata.description {
        dict.set("description", description);
    }
    
    // M4B-specific audiobook metadata
    dict.set("media_type", "2"); // Audiobook media type for iTunes
    
    Ok(dict)
}

/// Embeds cover art using ffmpeg-next attachment streams
/// This is the preferred method for embedding cover art during encoding
pub fn embed_cover_art_ffmpeg(
    _octx: &mut ff::format::context::Output,
    cover_data: &[u8],
) -> Result<()> {
    // TODO: Implement cover art embedding via ffmpeg-next
    // Current implementation is a placeholder - research correct API for:
    // 1. Creating attachment streams in ffmpeg-next
    // 2. Setting stream disposition for cover art
    // 3. Writing cover art packets to output format
    
    log::warn!("Cover art embedding via ffmpeg-next not yet implemented");
    log::info!("Cover art size: {} bytes (will be embedded via finalize stage)", cover_data.len());
    
    // For now, return success - cover art will be embedded via lofty in finalize stage
    // This ensures no regression while we complete the ffmpeg-next integration
    Ok(())
}

/// Sets global metadata on output format context
/// This applies metadata at the container level
pub fn set_container_metadata(
    octx: &mut ff::format::context::Output,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    let dict = metadata_to_ffmpeg_dict(metadata)?;
    octx.set_metadata(dict);
    
    log::debug!("Container metadata set via ffmpeg-next");
    Ok(())
}

/// Validates that ffmpeg-next can handle the provided metadata
/// Returns warnings for unsupported fields
pub fn validate_metadata_compatibility(metadata: &AudiobookMetadata) -> Vec<String> {
    let mut warnings = Vec::new();
    
    // Check for fields that might not be well-supported by ffmpeg-next
    if metadata.track.is_some() {
        warnings.push("Track number metadata may not be preserved in M4B format".to_string());
    }
    
    if metadata.disk.is_some() {
        warnings.push("Disk number metadata may not be preserved in M4B format".to_string());
    }
    
    // Validate cover art size (ffmpeg-next has different limits than lofty)
    if let Some(ref cover_data) = metadata.cover_art {
        if cover_data.len() > 10 * 1024 * 1024 { // 10MB limit
            warnings.push("Cover art exceeds recommended size limit (10MB)".to_string());
        }
    }
    
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metadata_conversion() {
        let metadata = AudiobookMetadata {
            title: Some("Test Book".to_string()),
            artist: Some("Test Author".to_string()),
            album: Some("Test Series".to_string()),
            genre: Some("Audiobook".to_string()),
            date: Some(2025),
            description: Some("Test description".to_string()),
            ..Default::default()
        };
        
        let dict = metadata_to_ffmpeg_dict(&metadata).expect("Metadata conversion should work");
        
        // Verify essential fields are present
        assert!(dict.get("title").is_some());
        assert!(dict.get("artist").is_some());
        assert!(dict.get("album").is_some());
        assert!(dict.get("media_type").is_some());
    }
    
    #[test]
    fn test_metadata_validation() {
        let metadata = AudiobookMetadata {
            title: Some("Test".to_string()),
            track: Some((1, Some(12))),
            cover_art: Some(vec![0u8; 15 * 1024 * 1024]), // 15MB - too large
            ..Default::default()
        };
        
        let warnings = validate_metadata_compatibility(&metadata);
        assert!(warnings.len() >= 2); // Should warn about track and cover art size
    }
}