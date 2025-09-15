//! Metadata handling for audiobook files
//!
//! This module provides functionality to read and write metadata
//! from/to audio files using the Lofty crate.

use serde::{Deserialize, Serialize};

pub mod reader;
pub mod writer;

// FFmpeg-next integration bridge for direct metadata embedding during encoding (always included after cleanup)
pub mod ffmpeg_bridge;

/// Represents audiobook metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookMetadata {
    /// Title of the audiobook
    pub title: Option<String>,
    /// Author of the book (mapped to artist in containers)
    pub artist: Option<String>,
    /// Album name (book/series name)
    pub album: Option<String>,
    /// Composer (often same as author for audiobooks)
    pub composer: Option<String>,
    /// Genre of the book
    pub genre: Option<String>,
    /// Publication year/date
    pub date: Option<u32>,
    /// Track number (chapter number, total chapters)
    pub track: Option<(u32, Option<u32>)>,
    /// Disk number (rarely used for audiobooks)
    pub disk: Option<(u32, Option<u32>)>,
    /// Comment field
    pub comment: Option<String>,
    /// Description or synopsis
    pub description: Option<String>,
    /// Cover art as raw bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<Vec<u8>>,
}

impl AudiobookMetadata {
    /// Creates a new empty metadata instance
    pub fn new() -> Self {
        Self {
            title: None,
            artist: None,
            album: None,
            composer: None,
            genre: None,
            date: None,
            track: None,
            disk: None,
            comment: None,
            description: None,
            cover_art: None,
        }
    }
}

impl Default for AudiobookMetadata {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export main functions for convenience
pub use reader::read_metadata;
pub use writer::write_metadata;

// Re-export ffmpeg-next bridge functions for native metadata and cover art embedding
pub use ffmpeg_bridge::{
    add_cover_art_stream_pre_header, set_container_metadata, validate_metadata_compatibility,
    write_cover_art_packet_post_header, CoverFormat,
};
