//! FFmpeg-Next metadata integration bridge
//!
//! This module provides conversion and integration between our AudiobookMetadata
//! structures and ffmpeg-next metadata APIs, enabling direct metadata embedding
//! during the encoding process.

use super::AudiobookMetadata;
use crate::errors::Result;
use ffmpeg_next as ff;

/// Converts `AudiobookMetadata` into an ffmpeg-next `Dictionary`.
///
/// Field mapping strategy:
/// - Standard textual fields (title, artist, album, composer, genre, comment, description)
///   map directly to container keys of the same name.
/// - `artist` is also duplicated to `album_artist` (common audiobook convention).
/// - `date` is written both as `date` and `year` to maximize player compatibility.
/// - A constant `media_type = 2` (iTunes audiobook) is always set for m4b targets.
///
/// Returns a populated `ffmpeg_next::Dictionary` ready to attach to an output
/// format context via `set_metadata`.
///
/// This function intentionally ignores fields currently unsupported by the
/// native embedding path (track, disk, cover art) – these are validated via
/// `validate_metadata_compatibility` beforehand so callers can surface warnings.
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







#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CoverFormat { Jpeg, Png }

/// Detects JPEG or PNG format from raw bytes
pub fn detect_cover_art_format(cover_data: &[u8]) -> Option<CoverFormat> {
    if cover_data.len() >= 3 && cover_data[0] == 0xFF && cover_data[1] == 0xD8 && cover_data[2] == 0xFF {
        return Some(CoverFormat::Jpeg);
    }
    if cover_data.len() >= 8 && cover_data[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(CoverFormat::Png);
    }
    None
}

/// Adds a cover art stream prior to header writing. Returns output stream index if added.
pub fn add_cover_art_stream_pre_header(
    octx: &mut ff::format::context::Output,
    cover_data: &[u8],
) -> Option<(usize, CoverFormat)> {
    if cover_data.is_empty() { return None; }
    let Some(format) = detect_cover_art_format(cover_data) else {
        log::warn!("Unsupported cover art format (only JPEG/PNG). Deferring to finalize stage");
        return None;
    };
    let codec_id = match format { CoverFormat::Jpeg => ff::codec::Id::MJPEG, CoverFormat::Png => ff::codec::Id::PNG };
    let Some(codec) = ff::encoder::find(codec_id) else {
        log::warn!("Cover art codec {:?} missing in ffmpeg build; fallback to finalize stage", format);
        return None;
    };
    match octx.add_stream(codec) {
        Ok(stream) => {
            let idx = stream.index();
            log::info!("Added cover art stream pre-header (index={}, format={:?}, bytes={})", idx, format, cover_data.len());
            Some((idx, format))
        }
        Err(e) => {
            log::warn!("Failed adding cover art stream ({}); fallback to finalize stage", e);
            None
        }
    }
}

/// Writes the cover art packet after header if a stream was added.
pub fn write_cover_art_packet_post_header(
    octx: &mut ff::format::context::Output,
    stream_index: usize,
    cover_data: &[u8],
    format: CoverFormat,
) {
    if cover_data.is_empty() { return; }
    let mut pkt = ff::Packet::copy(cover_data);
    pkt.set_stream(stream_index);
    pkt.set_flags(ff::packet::flag::Flags::KEY);
    if let Err(e) = pkt.write_interleaved(octx) {
        log::warn!("Failed writing native cover art packet ({}); finalize stage will attempt embedding", e);
    } else {
        log::info!("Native cover art packet written (stream={}, format={:?}, size={} bytes)", stream_index, format, cover_data.len());
    }
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
    use ffmpeg_next as ff;
    
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

    #[test]
    fn test_detect_cover_art_format() {
        let jpeg = b"\xFF\xD8\xFFrest"; // minimal marker
        let png = b"\x89PNG\r\n\x1A\nrest"; // minimal signature
        let bad = b"GIF89a";
        assert_eq!(detect_cover_art_format(jpeg), Some(CoverFormat::Jpeg));
        assert_eq!(detect_cover_art_format(png), Some(CoverFormat::Png));
        assert_eq!(detect_cover_art_format(bad), None);
    }

    #[test]
    fn test_add_cover_art_stream_pre_header_supported_and_unsupported() {
        ff::init().expect("ffmpeg init");
        let temp = tempfile::TempDir::new().expect("temp");
        let output = temp.path().join("test.m4b");
        let mut octx = ff::format::output(&output).expect("create output");

        // Unsupported should return None
        let unsupported = b"GIF89a"; // triggers log warning path
        assert!(add_cover_art_stream_pre_header(&mut octx, unsupported).is_none());

        // Supported JPEG returns Some
        let jpeg = b"\xFF\xD8\xFF\xE0data";
        let added = add_cover_art_stream_pre_header(&mut octx, jpeg);
        assert!(matches!(added, Some((_idx, CoverFormat::Jpeg))));
    }
}