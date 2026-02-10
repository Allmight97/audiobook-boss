//! Metadata handling for audiobook files
//!
//! This module provides functionality to read and write metadata
//! from/to audio files using ffmpeg-next.

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

pub mod reader;

// FFmpeg-next integration bridge for direct metadata embedding during encoding (always included after cleanup)
mod cover_art;
mod ffi;
pub mod ffmpeg_bridge;
mod ffmpeg_dict;
mod remux;
// Mp4ameta integration for reliable MP4/M4B metadata handling
pub mod mp4ameta_bridge;
// Passthrough helpers for chapter/cover preservation
pub mod passthrough;

/// Represents audiobook metadata
///
/// Field mapping for Plex/Audiobookshelf compatibility:
/// - `artist` = Author (also written to AlbumArtist)
/// - `composer` = Narrator (also mirrored to freeform NARRATOR)
/// - `series` = Series name (freeform SERIES, mirrored to MVNM)
/// - `series_part` = Book number in series (freeform SERIES-PART, mirrored to MVIN)
/// - `album_sort` = Computed TSOA for sorting ("SERIES PP - TITLE")
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
    /// Series name (freeform SERIES, mirrored to MVNM)
    pub series: Option<String>,
    /// Book number in series (freeform SERIES-PART, mirrored to MVIN)
    pub series_part: Option<String>,
    /// Sub-series name (secondary series)
    pub subseries: Option<String>,
    /// Book number in sub-series (secondary series part)
    pub subseries_part: Option<String>,
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
            subseries: None,
            subseries_part: None,
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

/// Validates series part input (no slashes).
pub(crate) fn validate_series_part(series_part: &str) -> Result<()> {
    if series_part.contains('/') {
        return Err(AppError::InvalidInput(
            "Series part must not include '/'. Use a plain number like 24.".to_string(),
        ));
    }
    Ok(())
}

/// Splits a semicolon-separated series list into primary and secondary values.
pub(crate) fn split_series_list(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(raw) = value else {
        return (None, None);
    };
    let mut parts = raw
        .split(';')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty());
    let primary = parts.next().map(|part| part.to_string());
    let secondary = parts.next().map(|part| part.to_string());
    (primary, secondary)
}

/// Builds ABS/Plex series tag values, including secondary series when complete.
pub(crate) fn build_series_list(
    series: Option<&str>,
    series_part: Option<&str>,
    subseries: Option<&str>,
    subseries_part: Option<&str>,
) -> (Option<String>, Option<String>) {
    let normalize = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
    };

    let primary_series = normalize(series);
    let primary_part = normalize(series_part);
    let secondary_series = normalize(subseries);
    let secondary_part = normalize(subseries_part);

    if let (Some(series), Some(part), Some(subseries), Some(subpart)) = (
        primary_series.as_deref(),
        primary_part.as_deref(),
        secondary_series.as_deref(),
        secondary_part.as_deref(),
    ) {
        return (
            Some(format!("{}; {}", series, subseries)),
            Some(format!("{}; {}", part, subpart)),
        );
    }

    (primary_series, primary_part)
}

/// Computes album sort (TSOA) from series + part + title.
/// Returns None if series_part is missing or cannot be parsed to a positive integer.
pub(crate) fn compute_album_sort(
    series: &str,
    series_part: Option<&str>,
    title: &str,
) -> Option<String> {
    let raw_part = series_part?.trim();
    if raw_part.is_empty() {
        return None;
    }

    let part_num = raw_part.parse::<u32>().ok()?;
    if part_num == 0 {
        return None;
    }

    if series.trim().is_empty() || title.trim().is_empty() {
        return None;
    }

    Some(format!(
        "{} {:02} - {}",
        series.trim(),
        part_num,
        title.trim()
    ))
}

// Re-export main functions for convenience
pub use reader::read_metadata;

// Re-export ffmpeg-next bridge functions for native metadata and cover art embedding
pub use ffmpeg_bridge::{
    add_cover_art_stream_pre_header, set_container_metadata, validate_metadata_compatibility,
    write_cover_art_packet_post_header, CoverFormat,
};
