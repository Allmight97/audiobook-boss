//! Metadata handling for audiobook files
//!
//! This module provides functionality to read and write metadata
//! from/to audio files through container-aware metadata strategies.

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

pub mod reader;
pub(crate) mod tag_registry;

mod container;
mod cover_art;
mod ffi;
mod ffmpeg_dict;
mod remux;
// Mp4ameta integration for reliable MP4/M4B metadata handling
pub mod mp4ameta_bridge;
// Passthrough helpers for chapter/cover preservation
pub mod passthrough;

/// Represents audiobook metadata.
///
/// `track`, `disk`, and `comment` are preserved for read compatibility with files
/// in the wild, but ABB does not expose them as supported UI draft write fields.
/// `album_sort` is writable only through explicit backend intent.
///
/// Field mapping for Plex/Audiobookshelf compatibility:
/// - `artist` = Author (also written to AlbumArtist)
/// - `composer` = Narrator (also mirrored to freeform NARRATOR)
/// - `series` = Series name (freeform SERIES)
/// - `series_part` = Series sequence / book # in series (freeform SERIES-PART)
/// - `album_sort` = TSOA library sort value; preserved unless explicit set/clear/recompute intent is sent
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
    /// Publication date as YYYY or YYYY-MM (©day)
    pub date: Option<String>,
    /// Track number (chapter number, total chapters)
    pub track: Option<(u32, Option<u32>)>,
    /// Disk number (rarely used for audiobooks)
    pub disk: Option<(u32, Option<u32>)>,
    /// Comment field (©cmt) - short note, distinct from description
    pub comment: Option<String>,
    /// Description or synopsis (desc)
    pub description: Option<String>,
    /// Series name (freeform SERIES)
    pub series: Option<String>,
    /// Series sequence / book # in series (freeform SERIES-PART)
    pub series_part: Option<String>,
    /// Sub-series name (secondary series)
    pub subseries: Option<String>,
    /// Series sequence / book # in sub-series (secondary series part)
    pub subseries_part: Option<String>,
    /// Album sort order for library sorting (soal/TSOA)
    pub album_sort: Option<String>,
    /// Cover art as raw bytes
    pub cover_art: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
#[serde(tag = "op", rename_all = "snake_case", content = "value")]
pub enum PatchOp<T> {
    Set(T),
    Clear,
    #[default]
    Noop,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
#[serde(tag = "op", rename_all = "snake_case", content = "value")]
pub enum AlbumSortPatchOp {
    Set(String),
    Clear,
    Recompute,
    #[default]
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AlbumSortWriteAction {
    Preserve,
    Set(String),
    Clear,
    Recompute,
}

#[derive(Debug, Clone)]
pub(crate) struct MetadataWritePlan {
    pub(crate) metadata: AudiobookMetadata,
    pub(crate) album_sort: AlbumSortWriteAction,
}

impl MetadataWritePlan {
    pub(crate) fn from_metadata(metadata: AudiobookMetadata) -> Self {
        let album_sort = match metadata.album_sort.as_deref() {
            Some(value) if value.trim().is_empty() => AlbumSortWriteAction::Clear,
            Some(value) => AlbumSortWriteAction::Set(value.to_string()),
            None => AlbumSortWriteAction::Preserve,
        };

        Self {
            metadata,
            album_sort,
        }
    }
}

/// Writable metadata-intent contract for ABB.
///
/// UI drafts support title, author, album, narrator, genre, publication date,
/// description, series, subseries, and cover-art intent. `album_sort` is included
/// as an explicit backend operation for set, clear, preserve, or recompute.
/// Read-compatible `track`, `disk`, and `comment` fields remain outside this
/// write-intent contract.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
pub struct MetadataIntentPatch {
    #[serde(default)]
    pub title: PatchOp<String>,
    #[serde(default)]
    pub artist: PatchOp<String>,
    #[serde(default)]
    pub album: PatchOp<String>,
    #[serde(default)]
    pub composer: PatchOp<String>,
    #[serde(default)]
    pub genre: PatchOp<String>,
    #[serde(default)]
    pub date: PatchOp<String>,
    #[serde(default)]
    pub description: PatchOp<String>,
    #[serde(default)]
    pub series: PatchOp<String>,
    #[serde(default)]
    pub series_part: PatchOp<String>,
    #[serde(default)]
    pub subseries: PatchOp<String>,
    #[serde(default)]
    pub subseries_part: PatchOp<String>,
    #[serde(default)]
    pub album_sort: AlbumSortPatchOp,
    #[serde(default)]
    pub cover_art: PatchOp<Vec<u8>>,
}

impl MetadataIntentPatch {
    pub(crate) fn clears_cover_art(&self) -> bool {
        matches!(self.cover_art, PatchOp::Clear)
    }

    pub(crate) fn apply_to_metadata(
        &self,
        mut base: AudiobookMetadata,
    ) -> Result<AudiobookMetadata> {
        apply_processing_string_patch(&self.title, &mut base.title);
        apply_processing_string_patch(&self.artist, &mut base.artist);
        apply_processing_string_patch(&self.album, &mut base.album);
        apply_processing_string_patch(&self.composer, &mut base.composer);
        apply_processing_string_patch(&self.genre, &mut base.genre);
        apply_processing_string_patch(&self.description, &mut base.description);
        apply_processing_string_patch(&self.series, &mut base.series);
        apply_processing_string_patch(&self.series_part, &mut base.series_part);
        apply_processing_string_patch(&self.subseries, &mut base.subseries);
        apply_processing_string_patch(&self.subseries_part, &mut base.subseries_part);

        match &self.date {
            PatchOp::Set(date) => {
                base.date = Some(validate_publication_date(date)?);
            }
            PatchOp::Clear => {
                base.date = None;
            }
            PatchOp::Noop => {}
        }

        match &self.cover_art {
            PatchOp::Set(bytes) => {
                base.cover_art = Some(bytes.clone());
            }
            PatchOp::Clear => {
                base.cover_art = None;
            }
            PatchOp::Noop => {}
        }

        if let Some(series_part) = base.series_part.as_deref() {
            let trimmed = series_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
        }

        if let Some(subseries_part) = base.subseries_part.as_deref() {
            let trimmed = subseries_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
        }

        apply_album_sort_patch(&self.album_sort, &mut base);

        Ok(base)
    }

    pub(crate) fn to_processing_overlay(&self) -> Result<AudiobookMetadata> {
        self.apply_to_metadata(AudiobookMetadata::new())
    }

    pub(crate) fn to_write_plan(&self) -> Result<MetadataWritePlan> {
        let mut metadata = AudiobookMetadata::new();

        apply_string_patch(&self.title, &mut metadata.title);
        apply_string_patch(&self.artist, &mut metadata.artist);
        apply_string_patch(&self.album, &mut metadata.album);
        apply_string_patch(&self.composer, &mut metadata.composer);
        apply_string_patch(&self.genre, &mut metadata.genre);
        apply_string_patch(&self.description, &mut metadata.description);
        apply_string_patch(&self.series, &mut metadata.series);
        apply_string_patch(&self.series_part, &mut metadata.series_part);
        apply_string_patch(&self.subseries, &mut metadata.subseries);
        apply_string_patch(&self.subseries_part, &mut metadata.subseries_part);

        match &self.date {
            PatchOp::Set(date) => {
                metadata.date = Some(validate_publication_date(date)?);
            }
            PatchOp::Clear => {
                // Metadata backends clear year/date tags when date is empty.
                metadata.date = Some(String::new());
            }
            PatchOp::Noop => {}
        }

        match &self.cover_art {
            PatchOp::Set(bytes) => {
                metadata.cover_art = Some(bytes.clone());
            }
            PatchOp::Clear => {
                metadata.cover_art = Some(Vec::new());
            }
            PatchOp::Noop => {}
        }

        if let Some(series_part) = metadata.series_part.as_deref() {
            let trimmed = series_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
        }

        if let Some(subseries_part) = metadata.subseries_part.as_deref() {
            let trimmed = subseries_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
        }

        let album_sort = match &self.album_sort {
            AlbumSortPatchOp::Set(value) if value.trim().is_empty() => AlbumSortWriteAction::Clear,
            AlbumSortPatchOp::Set(value) => {
                metadata.album_sort = Some(value.clone());
                AlbumSortWriteAction::Set(value.clone())
            }
            AlbumSortPatchOp::Clear => {
                metadata.album_sort = Some(String::new());
                AlbumSortWriteAction::Clear
            }
            AlbumSortPatchOp::Recompute => AlbumSortWriteAction::Recompute,
            AlbumSortPatchOp::Noop => AlbumSortWriteAction::Preserve,
        };

        Ok(MetadataWritePlan {
            metadata,
            album_sort,
        })
    }

    pub fn to_write_metadata(&self) -> Result<AudiobookMetadata> {
        Ok(self.to_write_plan()?.metadata)
    }
}

impl From<AudiobookMetadata> for MetadataIntentPatch {
    fn from(metadata: AudiobookMetadata) -> Self {
        let to_string_patch = |value: Option<String>| match value {
            Some(text) if text.trim().is_empty() => PatchOp::Clear,
            Some(text) => PatchOp::Set(text),
            None => PatchOp::Noop,
        };

        let date = match metadata.date {
            Some(date) => {
                let trimmed = date.trim();
                if trimmed.is_empty() {
                    PatchOp::Clear
                } else if let Some(normalized) = normalize_publication_date(trimmed) {
                    PatchOp::Set(normalized)
                } else {
                    PatchOp::Noop
                }
            }
            None => PatchOp::Noop,
        };

        let cover_art = match metadata.cover_art {
            Some(bytes) if bytes.is_empty() => PatchOp::Clear,
            Some(bytes) => PatchOp::Set(bytes),
            None => PatchOp::Noop,
        };
        let album_sort = match metadata.album_sort {
            Some(text) if text.trim().is_empty() => AlbumSortPatchOp::Clear,
            Some(text) => AlbumSortPatchOp::Set(text),
            None => AlbumSortPatchOp::Noop,
        };

        Self {
            title: to_string_patch(metadata.title),
            artist: to_string_patch(metadata.artist),
            album: to_string_patch(metadata.album),
            composer: to_string_patch(metadata.composer),
            genre: to_string_patch(metadata.genre),
            date,
            description: to_string_patch(metadata.description),
            series: to_string_patch(metadata.series),
            series_part: to_string_patch(metadata.series_part),
            subseries: to_string_patch(metadata.subseries),
            subseries_part: to_string_patch(metadata.subseries_part),
            album_sort,
            cover_art,
        }
    }
}

fn apply_string_patch(patch: &PatchOp<String>, output: &mut Option<String>) {
    match patch {
        PatchOp::Set(value) => {
            *output = Some(value.clone());
        }
        PatchOp::Clear => {
            *output = Some(String::new());
        }
        PatchOp::Noop => {}
    }
}

fn apply_processing_string_patch(patch: &PatchOp<String>, output: &mut Option<String>) {
    match patch {
        PatchOp::Set(value) => {
            *output = Some(value.clone());
        }
        PatchOp::Clear => {
            *output = None;
        }
        PatchOp::Noop => {}
    }
}

fn apply_album_sort_patch(patch: &AlbumSortPatchOp, metadata: &mut AudiobookMetadata) {
    match patch {
        AlbumSortPatchOp::Set(value) => {
            if value.trim().is_empty() {
                metadata.album_sort = None;
            } else {
                metadata.album_sort = Some(value.clone());
            }
        }
        AlbumSortPatchOp::Clear => {
            metadata.album_sort = None;
        }
        AlbumSortPatchOp::Recompute => {
            metadata.album_sort = recompute_album_sort(metadata);
        }
        AlbumSortPatchOp::Noop => {}
    }
}

fn recompute_album_sort(metadata: &AudiobookMetadata) -> Option<String> {
    compute_album_sort(
        metadata.series.as_deref()?,
        metadata.series_part.as_deref(),
        metadata.title.as_deref()?,
    )
}

fn validate_publication_date(value: &str) -> Result<String> {
    normalize_publication_date(value).ok_or_else(|| {
        AppError::InvalidInput(
            "Publication date must be YYYY or YYYY-MM with month 01-12.".to_string(),
        )
    })
}

pub(crate) fn normalize_publication_date(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.len() == 4 && raw.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(raw.to_string());
    }

    let bytes = raw.as_bytes();
    if bytes.len() < 7 {
        return None;
    }
    if !bytes[0..4].iter().all(u8::is_ascii_digit) || bytes[4] != b'-' {
        return None;
    }
    if !bytes[5..7].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let month = std::str::from_utf8(&bytes[5..7]).ok()?.parse::<u8>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    if bytes.len() > 7 && !matches!(bytes[7], b'-' | b'T' | b' ') {
        return None;
    }

    Some(format!("{}-{}", &raw[0..4], &raw[5..7]))
}

pub(crate) fn publication_year_from_date(value: Option<&str>) -> Option<i32> {
    let raw = value?.trim();
    if raw.len() < 4 {
        return None;
    }
    let year = &raw[0..4];
    if !year.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    year.parse::<i32>().ok()
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
            "Series sequence must not include '/'. Use a plain number like 24.".to_string(),
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

pub use reader::read_metadata;

pub use cover_art::{
    add_cover_art_stream_pre_header, detect_cover_art_format, write_cover_art_packet_post_header,
    CoverFormat,
};
pub use ffmpeg_dict::{set_container_metadata, validate_metadata_compatibility};
pub use remux::rewrite_metadata_with_ffmpeg;

pub(crate) fn save_metadata_with_plan(
    path: &std::path::Path,
    plan: &MetadataWritePlan,
) -> Result<()> {
    match container::classify(path)? {
        container::ContainerRoute::Mp4Family => {
            mp4ameta_bridge::write_metadata_with_plan(path, plan)
        }
        route => remux::rewrite_metadata_with_ffmpeg_plan_as(
            path,
            Some(plan),
            None,
            route.remux_output_format(),
        ),
    }
}

pub(crate) fn write_cover_art_to_file(path: &std::path::Path, cover_data: Vec<u8>) -> Result<()> {
    let metadata = AudiobookMetadata {
        cover_art: Some(cover_data),
        ..Default::default()
    };
    let plan = MetadataWritePlan::from_metadata(metadata);
    save_metadata_with_plan(path, &plan)
}

pub(crate) fn should_write_finalized_metadata(path: &std::path::Path) -> Result<bool> {
    Ok(matches!(
        container::classify(path)?,
        container::ContainerRoute::Mp4Family
    ))
}

pub(crate) fn write_finalized_metadata(
    path: &std::path::Path,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    mp4ameta_bridge::write_metadata(path, metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::AppError;

    #[test]
    fn metadata_intent_patch_applies_set_and_clear_ops() {
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Project Hail Mary".to_string()),
            artist: PatchOp::Clear,
            date: PatchOp::Clear,
            album_sort: AlbumSortPatchOp::Clear,
            cover_art: PatchOp::Clear,
            ..Default::default()
        };

        let metadata = patch
            .to_write_metadata()
            .expect("patch conversion should succeed");

        assert_eq!(metadata.title.as_deref(), Some("Project Hail Mary"));
        assert_eq!(metadata.artist.as_deref(), Some(""));
        assert_eq!(metadata.date.as_deref(), Some(""));
        assert_eq!(metadata.album_sort.as_deref(), Some(""));
        assert_eq!(metadata.cover_art, Some(Vec::new()));
    }

    #[test]
    fn metadata_intent_patch_keeps_album_sort_noop_explicit() {
        let patch = MetadataIntentPatch {
            genre: PatchOp::Set("Sci-Fi".to_string()),
            ..Default::default()
        };

        let plan = patch
            .to_write_plan()
            .expect("write plan should preserve album sort");

        assert_eq!(plan.metadata.genre.as_deref(), Some("Sci-Fi"));
        assert_eq!(plan.metadata.album_sort, None);
        assert_eq!(plan.album_sort, AlbumSortWriteAction::Preserve);
    }

    #[test]
    fn metadata_intent_patch_supports_album_sort_set_clear_and_recompute() {
        let set_plan = MetadataIntentPatch {
            album_sort: AlbumSortPatchOp::Set("Custom Sort".to_string()),
            ..Default::default()
        }
        .to_write_plan()
        .expect("album sort set should compile");
        assert_eq!(set_plan.metadata.album_sort.as_deref(), Some("Custom Sort"));
        assert_eq!(
            set_plan.album_sort,
            AlbumSortWriteAction::Set("Custom Sort".to_string())
        );

        let clear_plan = MetadataIntentPatch {
            album_sort: AlbumSortPatchOp::Clear,
            ..Default::default()
        }
        .to_write_plan()
        .expect("album sort clear should compile");
        assert_eq!(clear_plan.metadata.album_sort.as_deref(), Some(""));
        assert_eq!(clear_plan.album_sort, AlbumSortWriteAction::Clear);

        let recompute_plan = MetadataIntentPatch {
            album_sort: AlbumSortPatchOp::Recompute,
            ..Default::default()
        }
        .to_write_plan()
        .expect("album sort recompute should compile");
        assert_eq!(recompute_plan.metadata.album_sort, None);
        assert_eq!(recompute_plan.album_sort, AlbumSortWriteAction::Recompute);
    }

    #[test]
    fn metadata_intent_patch_rejects_invalid_publication_date() {
        let patch = MetadataIntentPatch {
            date: PatchOp::Set("2024-13".to_string()),
            ..Default::default()
        };

        let err = patch
            .to_write_metadata()
            .expect_err("invalid year should be rejected");

        match err {
            AppError::InvalidInput(message) => {
                assert!(message.contains("YYYY"), "unexpected message: {message}");
            }
            other => panic!("expected invalid input error, got: {other:?}"),
        }
    }

    #[test]
    fn metadata_intent_patch_rejects_series_part_with_slash() {
        let patch = MetadataIntentPatch {
            series_part: PatchOp::Set("7/8".to_string()),
            ..Default::default()
        };

        let err = patch
            .to_write_metadata()
            .expect_err("series part with slash should be rejected");

        match err {
            AppError::InvalidInput(message) => {
                assert!(
                    message.contains("must not include '/'"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected invalid input error, got: {other:?}"),
        }
    }

    #[test]
    fn metadata_intent_patch_from_metadata_maps_set_clear_and_noop() {
        let patch = MetadataIntentPatch::from(AudiobookMetadata {
            title: Some("The Way of Kings".to_string()),
            artist: Some(String::new()),
            genre: None,
            date: Some(String::new()),
            album_sort: Some("Stormlight 01 - The Way of Kings".to_string()),
            cover_art: Some(Vec::new()),
            ..Default::default()
        });

        match patch.title {
            PatchOp::Set(value) => assert_eq!(value, "The Way of Kings"),
            other => panic!("expected title set patch, got: {other:?}"),
        }
        match patch.artist {
            PatchOp::Clear => {}
            other => panic!("expected artist clear patch, got: {other:?}"),
        }
        match patch.genre {
            PatchOp::Noop => {}
            other => panic!("expected genre noop patch, got: {other:?}"),
        }
        match patch.date {
            PatchOp::Clear => {}
            other => panic!("expected date clear patch, got: {other:?}"),
        }
        match patch.cover_art {
            PatchOp::Clear => {}
            other => panic!("expected cover art clear patch, got: {other:?}"),
        }
        match patch.album_sort {
            AlbumSortPatchOp::Set(value) => {
                assert_eq!(value, "Stormlight 01 - The Way of Kings");
            }
            other => panic!("expected album sort set patch, got: {other:?}"),
        }
    }

    #[test]
    fn metadata_intent_patch_write_contract_keeps_read_compatible_fields_out_of_write_intent() {
        let patch = MetadataIntentPatch::from(AudiobookMetadata {
            title: Some("Read Compatible".to_string()),
            track: Some((3, Some(12))),
            disk: Some((1, Some(2))),
            comment: Some("Reader note".to_string()),
            ..Default::default()
        });

        let resolved = patch
            .to_write_metadata()
            .expect("write metadata should still compile without read-compatible fields");

        assert_eq!(resolved.title.as_deref(), Some("Read Compatible"));
        assert_eq!(resolved.track, None);
        assert_eq!(resolved.disk, None);
        assert_eq!(resolved.comment, None);
    }

    #[test]
    fn processing_patch_apply_to_metadata_preserves_values_for_noop() {
        let base = AudiobookMetadata {
            title: Some("Base Title".to_string()),
            artist: Some("Base Artist".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch::default();

        let resolved = patch
            .apply_to_metadata(base.clone())
            .expect("noop patch should preserve metadata");

        assert_eq!(resolved.title, base.title);
        assert_eq!(resolved.artist, base.artist);
    }

    #[test]
    fn processing_patch_apply_to_metadata_handles_set_and_clear() {
        let base = AudiobookMetadata {
            title: Some("Old Title".to_string()),
            artist: Some("Old Artist".to_string()),
            album_sort: Some("Old Sort".to_string()),
            cover_art: Some(vec![1, 2, 3]),
            date: Some("2020".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("New Title".to_string()),
            artist: PatchOp::Clear,
            album_sort: AlbumSortPatchOp::Clear,
            date: PatchOp::Set("2024-09-01".to_string()),
            cover_art: PatchOp::Clear,
            ..Default::default()
        };

        let resolved = patch
            .apply_to_metadata(base)
            .expect("set and clear patch should apply");

        assert_eq!(resolved.title.as_deref(), Some("New Title"));
        assert_eq!(resolved.artist, None);
        assert_eq!(resolved.album_sort, None);
        assert_eq!(resolved.date.as_deref(), Some("2024-09"));
        assert_eq!(resolved.cover_art, None);
    }

    #[test]
    fn processing_patch_can_recompute_album_sort_from_effective_metadata() {
        let base = AudiobookMetadata {
            title: Some("Old Title".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("1".to_string()),
            album_sort: Some("Custom Sort".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("New Title".to_string()),
            series_part: PatchOp::Set("2".to_string()),
            album_sort: AlbumSortPatchOp::Recompute,
            ..Default::default()
        };

        let resolved = patch
            .apply_to_metadata(base)
            .expect("recompute patch should apply");

        assert_eq!(
            resolved.album_sort.as_deref(),
            Some("Series 02 - New Title")
        );
    }

    #[test]
    fn computes_album_sort_with_numeric_part() {
        let result = compute_album_sort("Series", Some("3"), "Title");
        assert_eq!(result.as_deref(), Some("Series 03 - Title"));
    }

    #[test]
    fn skips_album_sort_with_fractional_part() {
        let result = compute_album_sort("Series", Some("1/5"), "Title");
        assert!(result.is_none());
    }

    #[test]
    fn skips_album_sort_when_part_missing_or_invalid() {
        assert!(compute_album_sort("Series", None, "Title").is_none());
        assert!(compute_album_sort("Series", Some(""), "Title").is_none());
        assert!(compute_album_sort("Series", Some("abc"), "Title").is_none());
        assert!(compute_album_sort("Series", Some("0"), "Title").is_none());
    }

    #[test]
    fn processing_patch_into_overlay_applies_without_source_metadata() {
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Overlay Title".to_string()),
            series: PatchOp::Set("Series Name".to_string()),
            ..Default::default()
        };

        let resolved = patch
            .to_processing_overlay()
            .expect("overlay-only patch should resolve");

        assert_eq!(resolved.title.as_deref(), Some("Overlay Title"));
        assert_eq!(resolved.series.as_deref(), Some("Series Name"));
    }

    #[test]
    fn normalize_publication_date_accepts_year_month_and_full_date_prefix() {
        assert_eq!(normalize_publication_date("2024"), Some("2024".to_string()));
        assert_eq!(
            normalize_publication_date("2024-07"),
            Some("2024-07".to_string())
        );
        assert_eq!(
            normalize_publication_date("2024-07-15"),
            Some("2024-07".to_string())
        );
        assert_eq!(
            normalize_publication_date("2024-07-15T10:00:00Z"),
            Some("2024-07".to_string())
        );
        assert_eq!(normalize_publication_date("2024-13"), None);
        assert_eq!(normalize_publication_date("2024-00"), None);
        assert_eq!(normalize_publication_date("abcd"), None);
    }
}
