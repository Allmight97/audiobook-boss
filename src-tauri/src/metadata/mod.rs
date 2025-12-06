//! Metadata handling for audiobook files
//!
//! This module provides functionality to read and write metadata
//! from/to audio files using ffmpeg-next.

use serde::{Deserialize, Serialize};

pub mod reader;

// FFmpeg-next integration bridge for direct metadata embedding during encoding (always included after cleanup)
pub mod ffmpeg_bridge;
// Passthrough helpers for chapter/cover preservation
pub mod passthrough;

/// Represents audiobook metadata
///
/// Field mapping for Plex/Audiobookshelf compatibility:
/// - `artist` = Author (also written to AlbumArtist)
/// - `composer` = Narrator (also mirrored to freeform NARRATOR)
/// - `series` = Series name (MVNM, mirrored to freeform SERIES)
/// - `series_part` = Book number in series (MVIN, mirrored to freeform SERIES-PART)
/// - `album_sort` = Computed TSOA for sorting ("SERIES PP - TITLE")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookMetadata {
    /// Title of the audiobook (©nam)
    pub title: Option<String>,
    /// Author of the book (©ART, also written to aART/AlbumArtist)
    pub artist: Option<String>,
    /// Album name - typically same as title for audiobooks (©alb)
    pub album: Option<String>,
    /// Narrator of the audiobook (©wrt/Composer, mirrored to freeform NARRATOR)
    pub composer: Option<String>,
    /// Genre of the book (©gen)
    pub genre: Option<String>,
    /// Publication year/date (©day)
    pub date: Option<u32>,
    /// Track number (chapter number, total chapters)
    pub track: Option<(u32, Option<u32>)>,
    /// Disk number (rarely used for audiobooks)
    pub disk: Option<(u32, Option<u32>)>,
    /// Comment field (©cmt) - short note, distinct from description
    pub comment: Option<String>,
    /// Description or synopsis (desc)
    pub description: Option<String>,
    /// Series name (©mvn/MVNM, mirrored to freeform SERIES)
    pub series: Option<String>,
    /// Book number in series as string to support "1/5" format (©mvi/MVIN, mirrored to freeform SERIES-PART)
    pub series_part: Option<String>,
    /// Album sort order for library sorting (soal/TSOA) - computed as "SERIES PP - TITLE"
    pub album_sort: Option<String>,
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
            series: None,
            series_part: None,
            album_sort: None,
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

// Re-export ffmpeg-next bridge functions for native metadata and cover art embedding
pub use ffmpeg_bridge::{
    add_cover_art_stream_pre_header, set_container_metadata, validate_metadata_compatibility,
    write_cover_art_packet_post_header, CoverFormat,
};
