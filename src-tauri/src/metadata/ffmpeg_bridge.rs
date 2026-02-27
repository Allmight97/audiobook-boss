//! FFmpeg-Next metadata integration bridge
//!
//! This module provides conversion and integration between our AudiobookMetadata
//! structures and ffmpeg-next metadata APIs, enabling direct metadata embedding
//! during the encoding process.

pub use super::cover_art::{
    add_cover_art_stream_pre_header, detect_cover_art_format, write_cover_art_packet_post_header,
    CoverFormat,
};
pub use super::ffmpeg_dict::{set_container_metadata, validate_metadata_compatibility};
pub use super::remux::rewrite_metadata_with_ffmpeg;

// EXCEPTION: requires private API access
#[cfg(test)]
mod tests {
    #[test]
    fn metadata_to_ffmpeg_conversion_includes_core_fields() {
        let metadata = super::super::AudiobookMetadata {
            title: Some("Test Audiobook".to_string()),
            artist: Some("Test Author".to_string()),
            album: Some("Test Series".to_string()),
            composer: Some("Test Narrator".to_string()),
            genre: Some("Audiobook".to_string()),
            date: Some("2025".to_string()),
            description: Some("A test audiobook for metadata integration".to_string()),
            cover_art: Some(vec![0xFF, 0xD8, 0xFF, 0xE0]),
            ..Default::default()
        };

        let dict_result = super::super::ffmpeg_dict::metadata_to_ffmpeg_dict(&metadata);
        assert!(dict_result.is_ok(), "Metadata conversion should succeed");

        let dict = dict_result.expect("metadata conversion should succeed");
        assert!(dict.get("title").is_some(), "Title should be present");
        assert!(dict.get("artist").is_some(), "Artist should be present");
        assert!(
            dict.get("media_type").is_some(),
            "Media type should be set for audiobooks"
        );
    }
}
