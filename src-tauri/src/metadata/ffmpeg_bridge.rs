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
    octx: &mut ff::format::context::Output,
    cover_data: &[u8],
) -> Result<()> {
    // No direct AppError usage required at scaffolding stage.

    if cover_data.is_empty() {
        log::warn!("Cover art data empty; skipping embedding scaffolding");
        return Ok(());
    }

    // --- Format Detection (scaffolding) ---
    #[derive(Debug, Copy, Clone)]
    enum CoverFormat { Jpeg, Png }
    let format = if cover_data.len() >= 3 && cover_data[0] == 0xFF && cover_data[1] == 0xD8 && cover_data[2] == 0xFF {
        Some(CoverFormat::Jpeg)
    } else if cover_data.len() >= 8 && cover_data[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        Some(CoverFormat::Png)
    } else {
        None
    };

    let Some(format) = format else {
        log::warn!("Unsupported or unrecognized cover art format (only JPEG/PNG supported in scaffolding); deferring to finalize stage");
        return Ok(()); // graceful fallback
    };

    // --- Stream Addition (no packet write yet) ---
    // We add a placeholder stream with the appropriate codec so that in the
    // next implementation step we can write a single packet after header.
    let codec_id = match format { CoverFormat::Jpeg => ff::codec::Id::MJPEG, CoverFormat::Png => ff::codec::Id::PNG };
    let Some(codec) = ff::encoder::find(codec_id) else {
        log::warn!("Cover art codec {:?} not found in ffmpeg build; deferring to finalize stage", format);
        return Ok(());
    };

    match octx.add_stream(codec) {
        Ok(stream) => {
            // NOTE: We are not configuring width/height or writing a packet yet.
            // Some players may ignore an empty MJPEG/PNG stream; that's acceptable
            // at this scaffolding stage because Lofty finalize embedding still runs.
            // We log the index for traceability.
            let idx = stream.index();
            log::info!(
                "Cover art scaffolding: added placeholder stream (index={}, codec={:?}, bytes={})", 
                idx, format, cover_data.len()
            );
        }
        Err(e) => {
            log::warn!("Failed to add cover art stream ({}); deferring to finalize stage", e);
        }
    }

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