//! FFmpeg metadata dictionary helpers for AudiobookMetadata.

use super::{
    compute_album_sort, split_series_list, AlbumSortWriteAction, AudiobookMetadata,
    MetadataWritePlan,
};
use crate::errors::Result;
use crate::metadata::field_schema::TagField;
use crate::metadata::metadata_sinks::{
    apply_metadata_field_ops_to_ffmpeg_dict, should_remove_key_for_metadata,
};

use ffmpeg_next as ff;

use super::cover_art::format::{
    classify_cover_art_format, detect_image_dimensions, CoverArtFormatClassification, CoverFormat,
};

pub fn metadata_to_ffmpeg_dict(metadata: &AudiobookMetadata) -> Result<ff::Dictionary<'_>> {
    let mut dict = ff::Dictionary::new();
    apply_metadata_field_ops_to_ffmpeg_dict(&mut dict, metadata)?;
    dict.set("media_type", "2");

    // Album sort is intentionally excluded from field ops and handled by write plans.
    if let Some(ref album_sort) = metadata.album_sort {
        if !album_sort.trim().is_empty() {
            dict.set("sort_album", album_sort);
        }
    }

    Ok(dict)
}

pub(crate) fn merge_metadata_with_plan<'a>(
    existing: ff::Dictionary<'a>,
    plan: &MetadataWritePlan,
) -> Result<ff::Dictionary<'a>> {
    let metadata = &plan.metadata;
    let mut merged = ff::Dictionary::new();
    for (key, value) in existing.iter() {
        if should_remove_key(metadata, &plan.album_sort, key) {
            continue;
        }
        merged.set(key, value);
    }

    let overrides = metadata_to_ffmpeg_dict(metadata)?;
    for (key, value) in overrides.iter() {
        merged.set(key, value);
    }

    match &plan.album_sort {
        AlbumSortWriteAction::Preserve => {}
        AlbumSortWriteAction::Set(value) => {
            if !value.trim().is_empty() {
                merged.set("sort_album", value);
            }
        }
        AlbumSortWriteAction::Clear => {}
        AlbumSortWriteAction::Recompute => {
            if let Some(album_sort) = compute_album_sort_from_dict(&merged) {
                merged.set("sort_album", &album_sort);
            }
        }
    }

    Ok(merged)
}

fn should_remove_key(
    metadata: &AudiobookMetadata,
    album_sort: &AlbumSortWriteAction,
    key: &str,
) -> bool {
    if matches!(
        album_sort,
        AlbumSortWriteAction::Clear | AlbumSortWriteAction::Recompute
    ) && key == "sort_album"
    {
        return true;
    }

    should_remove_key_for_metadata(metadata, key)
}

fn compute_album_sort_from_dict(dict: &ff::Dictionary<'_>) -> Option<String> {
    let series = first_tag(dict, TagField::Series);
    let series_part = first_tag(dict, TagField::SeriesPart);
    let title = dict.get("title").map(str::to_string);

    let (primary_series, _) = split_series_list(series.as_deref());
    let (primary_part, _) = split_series_list(series_part.as_deref());

    match (primary_series.as_deref(), title.as_deref()) {
        (Some(series), Some(title)) => compute_album_sort(series, primary_part.as_deref(), title),
        _ => None,
    }
}

fn first_tag(dict: &ff::Dictionary<'_>, field: TagField) -> Option<String> {
    field
        .read_keys()
        .iter()
        .find_map(|key| dict.get(key).map(str::to_string))
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

/// Returns pre-encode warnings for cover-art compatibility (size, format, dimensions).
/// Track/disk preservation is validated via metadata sink proofs, not here.
pub fn validate_metadata_compatibility(metadata: &AudiobookMetadata) -> Vec<String> {
    let mut warnings = Vec::new();

    // Validate cover art comprehensively
    if let Some(ref cover_data) = metadata.cover_art {
        // Detect format up-front so we can tailor size heuristics
        let classification = classify_cover_art_format(cover_data);
        let detected_format = match classification {
            CoverArtFormatClassification::Supported(format) => Some(format),
            CoverArtFormatClassification::KnownUnsupported
            | CoverArtFormatClassification::Unrecognized => None,
        };

        // Size validation (allow tiny placeholder images if format is detectable)
        if cover_data.is_empty() {
            warnings.push("Cover art data is empty".to_string());
        } else if cover_data.len() > 10 * 1024 * 1024 {
            // 10MB limit
            warnings.push("Cover art exceeds recommended size limit (10MB)".to_string());
        } else if cover_data.len() < 100 {
            // Only warn about being too small if we *cannot* positively detect a supported format.
            // Rationale: test fixtures and some real feeds may supply minimal valid JPEG/PNG headers
            // (e.g. JFIF without SOF marker) for placeholder artwork. We treat those as acceptable.
            if detected_format.is_none() {
                warnings.push("Cover art data seems too small to be a valid image".to_string());
            }
        }

        // Format validation & dimension heuristics
        match classification {
            CoverArtFormatClassification::Supported(format) => {
                validate_supported_cover_art_format(format, cover_data, &mut warnings);
            }
            CoverArtFormatClassification::KnownUnsupported => {
                warnings.push("Cover art format not supported for native embedding (only JPEG and PNG are supported) - cover art will be skipped".to_string());
            }
            CoverArtFormatClassification::Unrecognized => {
                log::debug!("Cover art bytes not recognized as JPEG/PNG; proceeding without native embedding warning");
            }
        }
    }

    warnings
}

fn validate_supported_cover_art_format(
    format: CoverFormat,
    cover_data: &[u8],
    warnings: &mut Vec<String>,
) {
    log::debug!(
        "Cover art format validation: {} detected and supported",
        format.display_name()
    );
    check_cover_art_dimensions(format, cover_data, warnings);
    check_cover_art_codec(format, warnings);
}

fn check_cover_art_dimensions(format: CoverFormat, cover_data: &[u8], warnings: &mut Vec<String>) {
    if let Some((width, height)) = detect_image_dimensions(cover_data, format) {
        if width > 2000 || height > 2000 {
            warnings.push(format!(
                "Cover art dimensions ({}x{}) are very large and may cause compatibility issues",
                width, height
            ));
        } else if width < 100 || height < 100 {
            warnings.push(format!(
                "Cover art dimensions ({}x{}) are very small and may not display well",
                width, height
            ));
        }
    } else if cover_data.len() >= 100 {
        // Suppress dimension warning for tiny (<100B) placeholder images.
        warnings.push(format!(
            "Could not detect {} dimensions - file may be corrupted",
            format.display_name()
        ));
    }
}

fn check_cover_art_codec(format: CoverFormat, warnings: &mut Vec<String>) {
    if ff::encoder::find(format.codec_id()).is_none() {
        warnings.push(format!(
            "FFmpeg codec for {:?} format not available in this build - cover art will be skipped",
            format
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_dict() -> ff::Dictionary<'static> {
        let mut dict = ff::Dictionary::new();
        dict.set("title", "Existing Title");
        dict.set("series", "Existing Series");
        dict.set("series-part", "1");
        dict.set("sort_album", "Custom Sort");
        dict
    }

    fn png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&[0, 0, 0, 13]);
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    fn metadata_with_cover_art(cover_art: Vec<u8>) -> AudiobookMetadata {
        AudiobookMetadata {
            cover_art: Some(cover_art),
            ..Default::default()
        }
    }

    #[test]
    fn merge_metadata_preserves_album_sort_without_explicit_intent() {
        let plan = MetadataWritePlan {
            metadata: AudiobookMetadata {
                genre: Some("Sci-Fi".to_string()),
                ..Default::default()
            },
            album_sort: AlbumSortWriteAction::Preserve,
        };

        let merged = merge_metadata_with_plan(base_dict(), &plan).expect("merge metadata");

        assert_eq!(merged.get("genre"), Some("Sci-Fi"));
        assert_eq!(merged.get("sort_album"), Some("Custom Sort"));
    }

    #[test]
    fn merge_metadata_sets_and_clears_album_sort_explicitly() {
        let set_plan = MetadataWritePlan::from_metadata(AudiobookMetadata {
            album_sort: Some("Requested Sort".to_string()),
            ..Default::default()
        });
        let set = merge_metadata_with_plan(base_dict(), &set_plan).expect("set album sort");
        assert_eq!(set.get("sort_album"), Some("Requested Sort"));

        let clear_plan = MetadataWritePlan::from_metadata(AudiobookMetadata {
            album_sort: Some(String::new()),
            ..Default::default()
        });
        let cleared = merge_metadata_with_plan(base_dict(), &clear_plan).expect("clear album sort");
        assert_eq!(cleared.get("sort_album"), None);
    }

    #[test]
    fn merge_metadata_recomputes_album_sort_only_when_requested() {
        let plan = MetadataWritePlan {
            metadata: AudiobookMetadata {
                series_part: Some("2".to_string()),
                ..Default::default()
            },
            album_sort: AlbumSortWriteAction::Recompute,
        };

        let merged = merge_metadata_with_plan(base_dict(), &plan).expect("recompute album sort");

        assert_eq!(
            merged.get("sort_album"),
            Some("Existing Series 02 - Existing Title")
        );
    }

    #[test]
    fn metadata_to_ffmpeg_dict_writes_literal_audiobook_fields() {
        let metadata = AudiobookMetadata {
            title: Some("Test Audiobook".to_string()),
            artist: Some("Test Author".to_string()),
            album: Some("Test Series".to_string()),
            composer: Some("Test Narrator".to_string()),
            genre: Some("Audiobook".to_string()),
            date: Some("2025".to_string()),
            description: Some("A test audiobook for metadata integration".to_string()),
            series: Some("Primary".to_string()),
            series_part: Some("7".to_string()),
            subseries: Some("Sub".to_string()),
            subseries_part: Some("2".to_string()),
            track: Some((4, Some(32))),
            disk: Some((1, Some(3))),
            ..Default::default()
        };

        let dict = metadata_to_ffmpeg_dict(&metadata).expect("metadata conversion should succeed");

        assert_eq!(dict.get("title"), Some("Test Audiobook"));
        assert_eq!(dict.get("artist"), Some("Test Author"));
        assert_eq!(dict.get("album_artist"), Some("Test Author"));
        assert_eq!(dict.get("album"), Some("Test Series"));
        assert_eq!(dict.get("composer"), Some("Test Narrator"));
        assert_eq!(dict.get("genre"), Some("Audiobook"));
        assert_eq!(dict.get("date"), Some("2025"));
        assert_eq!(dict.get("year"), Some("2025"));
        assert_eq!(
            dict.get("description"),
            Some("A test audiobook for metadata integration")
        );
        assert_eq!(dict.get("series"), Some("Primary; Sub"));
        assert_eq!(
            dict.get("----:com.apple.iTunes:SERIES"),
            Some("Primary; Sub")
        );
        assert_eq!(dict.get("series-part"), Some("7; 2"));
        assert_eq!(dict.get("----:com.apple.iTunes:SERIES-PART"), Some("7; 2"));
        assert_eq!(dict.get("track"), Some("4/32"));
        assert_eq!(dict.get("disc"), Some("1/3"));
        assert_eq!(dict.get("media_type"), Some("2"));
    }

    #[test]
    fn validate_metadata_compatibility_warns_for_known_unsupported_cover_art_format() {
        let metadata = metadata_with_cover_art(b"RIFF\x00\x00\x00\x00WEBP".to_vec());

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(
            warnings
                .iter()
                .any(|warning| warning
                    .contains("Cover art format not supported for native embedding"))
        );
    }

    #[test]
    fn validate_metadata_compatibility_leaves_unrecognized_cover_art_bytes_quiet() {
        let metadata = metadata_with_cover_art(vec![0; 128]);

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn validate_metadata_compatibility_warns_for_large_cover_art_dimensions() {
        let metadata = metadata_with_cover_art(png_with_dimensions(2001, 600));

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(warnings
            .iter()
            .any(|warning| warning.contains("Cover art dimensions (2001x600) are very large")));
    }

    #[test]
    fn validate_metadata_compatibility_warns_for_small_cover_art_dimensions() {
        let metadata = metadata_with_cover_art(png_with_dimensions(99, 600));

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(warnings
            .iter()
            .any(|warning| warning.contains("Cover art dimensions (99x600) are very small")));
    }

    #[test]
    fn validate_metadata_compatibility_warns_for_supported_format_without_dimensions() {
        let mut cover_art = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        cover_art.resize(100, 0);
        let metadata = metadata_with_cover_art(cover_art);

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(warnings
            .iter()
            .any(|warning| warning == "Could not detect PNG dimensions - file may be corrupted"));
    }

    #[test]
    fn validate_metadata_compatibility_warns_when_cover_art_exceeds_10mb() {
        let metadata = metadata_with_cover_art(vec![0u8; 15 * 1024 * 1024]);

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(warnings
            .iter()
            .any(|warning| { warning == "Cover art exceeds recommended size limit (10MB)" }));
    }

    #[test]
    fn validate_metadata_compatibility_is_silent_at_10mb_boundary() {
        let metadata = metadata_with_cover_art(vec![0u8; 10 * 1024 * 1024]);

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(
            warnings.is_empty(),
            "10MB cover art should not generate size warnings: {warnings:?}"
        );
    }

    #[test]
    fn validate_metadata_compatibility_ignores_track_and_disk() {
        let metadata = AudiobookMetadata {
            track: Some((1, Some(12))),
            disk: Some((1, Some(3))),
            ..Default::default()
        };

        let warnings = validate_metadata_compatibility(&metadata);

        assert!(
            warnings.is_empty(),
            "track/disk preservation is proven in metadata_sinks, not here: {warnings:?}"
        );
    }
}
