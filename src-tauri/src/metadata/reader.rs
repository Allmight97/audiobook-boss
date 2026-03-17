//! Metadata reading via ffmpeg-next
use super::{mp4ameta_bridge, normalize_publication_date, split_series_list, AudiobookMetadata};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::tag_registry::{SERIES_PART_READ_KEYS, SERIES_READ_KEYS};
use ffmpeg_next as ff;
use std::path::Path;

/// Reads container-level metadata and attached cover art using ffmpeg-next.
pub fn read_metadata<P: AsRef<Path>>(file_path: P) -> Result<AudiobookMetadata> {
    let path = file_path.as_ref();

    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            sanitize_path_for_display(path)
        )));
    }

    if mp4ameta_bridge::is_mp4_container(path) {
        match mp4ameta_bridge::read_metadata(path) {
            Ok(mut metadata) => {
                metadata.cover_art = normalize_cover_art(metadata.cover_art);
                if needs_ffmpeg_partial_hydration(&metadata) {
                    // FALLBACK[FB-001]: trigger=mp4ameta read succeeds but leaves cover art or primary series fields unset
                    // observe=warn log with backfilled fields
                    // sunset=2026-05-31 issue=#196
                    match read_metadata_with_ffmpeg(path) {
                        Ok(fallback) => {
                            let mut backfilled_fields: Vec<&str> = Vec::new();

                            if metadata.series.is_none() && fallback.series.is_some() {
                                backfilled_fields.push("series");
                            }
                            if metadata.series_part.is_none() && fallback.series_part.is_some() {
                                backfilled_fields.push("series_part");
                            }
                            if metadata.subseries.is_none() && fallback.subseries.is_some() {
                                backfilled_fields.push("subseries");
                            }
                            if metadata.subseries_part.is_none()
                                && fallback.subseries_part.is_some()
                            {
                                backfilled_fields.push("subseries_part");
                            }
                            let fallback_cover_art = normalize_cover_art(fallback.cover_art);
                            if metadata.cover_art.is_none() && fallback_cover_art.is_some() {
                                backfilled_fields.push("cover_art");
                            }

                            metadata.series = metadata.series.or(fallback.series);
                            metadata.series_part = metadata.series_part.or(fallback.series_part);
                            metadata.subseries = metadata.subseries.or(fallback.subseries);
                            metadata.subseries_part =
                                metadata.subseries_part.or(fallback.subseries_part);
                            if metadata.cover_art.is_none() {
                                metadata.cover_art = fallback_cover_art;
                            }

                            if !backfilled_fields.is_empty() {
                                log::warn!(
                                    "FALLBACK[FB-001] applied ffmpeg partial metadata hydration for {} (fields: {})",
                                    path.display(),
                                    backfilled_fields.join(", ")
                                );
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "FALLBACK[FB-001] ffmpeg partial metadata hydration unavailable for {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
                return Ok(metadata);
            }
            Err(e) => {
                // FALLBACK[FB-001]: trigger=mp4ameta read fails and ffmpeg fallback path is required
                // observe=warn log with primary read failure reason
                // sunset=2026-03-31 issue=#196
                log::warn!("mp4ameta read failed ({}); falling back to ffmpeg", e);
            }
        }
    }

    read_metadata_with_ffmpeg(path)
}

fn read_metadata_with_ffmpeg(path: &Path) -> Result<AudiobookMetadata> {
    ff::init().map_err(AppError::Ffmpeg)?;

    let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
    let dict = ictx.metadata();

    let mut metadata = AudiobookMetadata::new();

    metadata.title = dict.get("title").map(str::to_string);
    metadata.artist = dict.get("artist").map(str::to_string);
    metadata.album = dict.get("album").map(str::to_string);
    metadata.composer = dict.get("composer").map(str::to_string);
    metadata.genre = dict.get("genre").map(str::to_string);
    metadata.comment = dict.get("comment").map(str::to_string);
    metadata.description = dict.get("description").map(str::to_string);
    metadata.album_sort = dict.get("sort_album").map(str::to_string);

    // FALLBACK[FB-007]: trigger=legacy files with movement-tag-only series metadata
    // observe=covered via metadata fallback tests and migration fallback register
    // sunset=2026-05-31 issue=#202
    // Series metadata: prefer canonical tags, fall back to legacy/movement tags
    let series_raw = first_tag(&dict, &SERIES_READ_KEYS);
    let series_part_raw = first_tag(&dict, &SERIES_PART_READ_KEYS);
    let (series, subseries) = split_series_list(series_raw.as_deref());
    let (series_part, subseries_part) = split_series_list(series_part_raw.as_deref());
    metadata.series = series;
    metadata.series_part = series_part;
    metadata.subseries = subseries;
    metadata.subseries_part = subseries_part;

    // Year/date can be stored under `date` or `year`
    metadata.date = dict
        .get("date")
        .or_else(|| dict.get("year"))
        .and_then(normalize_publication_date);

    // Attached picture (cover art)
    metadata.cover_art = extract_attached_pic(&ictx);

    Ok(metadata)
}

fn normalize_cover_art(cover_art: Option<Vec<u8>>) -> Option<Vec<u8>> {
    cover_art.filter(|bytes| !bytes.is_empty())
}

fn needs_ffmpeg_partial_hydration(metadata: &AudiobookMetadata) -> bool {
    metadata.cover_art.is_none() || metadata.series.is_none() || metadata.series_part.is_none()
}

fn first_tag_with_lookup<F>(keys: &[&str], mut lookup: F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    keys.iter().find_map(|key| lookup(key))
}

fn first_tag(dict: &ff::DictionaryRef<'_>, keys: &[&str]) -> Option<String> {
    first_tag_with_lookup(keys, |key| dict.get(key).map(str::to_string))
}

/// Extracts the first attached picture (cover art) from the container streams.
fn extract_attached_pic(ictx: &ff::format::context::Input) -> Option<Vec<u8>> {
    use ff::format::stream::Disposition;

    for stream in ictx.streams() {
        if stream.disposition().contains(Disposition::ATTACHED_PIC) {
            unsafe {
                let av_stream = stream.as_ptr();
                let pic = (*av_stream).attached_pic;
                if !pic.data.is_null() && pic.size > 0 {
                    let bytes = std::slice::from_raw_parts(pic.data, pic.size as usize);
                    return Some(bytes.to_vec());
                }
            }
        }
    }
    None
}

// EXCEPTION: tiny helper inline tests — first_tag_with_lookup is private, no I/O
#[cfg(test)]
mod tests {
    use super::{first_tag_with_lookup, needs_ffmpeg_partial_hydration, AudiobookMetadata};
    use crate::metadata::tag_registry::{SERIES_PART_READ_KEYS, SERIES_READ_KEYS};
    use std::collections::BTreeMap;

    #[test]
    fn prefers_canonical_series_part_key() {
        let tags = BTreeMap::from([
            ("series-part", "1".to_string()),
            ("----:com.apple.iTunes:SERIES-PART", "2".to_string()),
            ("episode_sort", "3".to_string()),
            ("MVIN", "4".to_string()),
        ]);
        let selected = first_tag_with_lookup(&SERIES_PART_READ_KEYS, |key| tags.get(key).cloned());
        assert_eq!(selected.as_deref(), Some("1"));
    }

    #[test]
    fn falls_back_to_itunes_series_part_key() {
        let tags = BTreeMap::from([
            ("----:com.apple.iTunes:SERIES-PART", "2".to_string()),
            ("episode_sort", "3".to_string()),
            ("MVIN", "4".to_string()),
        ]);
        let selected = first_tag_with_lookup(&SERIES_PART_READ_KEYS, |key| tags.get(key).cloned());
        assert_eq!(selected.as_deref(), Some("2"));
    }

    #[test]
    fn falls_back_to_episode_sort_before_mvin() {
        let tags = BTreeMap::from([("episode_sort", "3".to_string()), ("MVIN", "4".to_string())]);
        let selected = first_tag_with_lookup(&SERIES_PART_READ_KEYS, |key| tags.get(key).cloned());
        assert_eq!(selected.as_deref(), Some("3"));
    }

    #[test]
    fn falls_back_to_mvin_when_other_series_part_keys_are_missing() {
        let tags = BTreeMap::from([("MVIN", "4".to_string())]);
        let selected = first_tag_with_lookup(&SERIES_PART_READ_KEYS, |key| tags.get(key).cloned());
        assert_eq!(selected.as_deref(), Some("4"));
    }

    #[test]
    fn does_not_treat_part_as_series_part_key() {
        let tags = BTreeMap::from([("part", "9".to_string())]);
        let selected = first_tag_with_lookup(&SERIES_PART_READ_KEYS, |key| tags.get(key).cloned());
        assert!(selected.is_none());
    }

    #[test]
    fn falls_back_to_show_before_movement_name_for_series() {
        let tags = BTreeMap::from([
            ("show", "Show Series".to_string()),
            ("MVNM", "Movement".to_string()),
        ]);
        let selected = first_tag_with_lookup(&SERIES_READ_KEYS, |key| tags.get(key).cloned());
        assert_eq!(selected.as_deref(), Some("Show Series"));
    }

    #[test]
    fn falls_back_to_movement_name_for_series_when_other_keys_are_missing() {
        let tags = BTreeMap::from([("MVNM", "Movement".to_string())]);
        let selected = first_tag_with_lookup(&SERIES_READ_KEYS, |key| tags.get(key).cloned());
        assert_eq!(selected.as_deref(), Some("Movement"));
    }

    #[test]
    fn partial_hydration_trigger_ignores_missing_subseries_only() {
        let metadata = AudiobookMetadata {
            series: Some("Series".to_string()),
            series_part: Some("1".to_string()),
            cover_art: Some(vec![1, 2, 3]),
            ..Default::default()
        };

        assert!(!needs_ffmpeg_partial_hydration(&metadata));
    }

    #[test]
    fn partial_hydration_trigger_keeps_cover_art_and_primary_series_gaps() {
        let missing_cover_art = AudiobookMetadata {
            series: Some("Series".to_string()),
            series_part: Some("1".to_string()),
            ..Default::default()
        };
        assert!(needs_ffmpeg_partial_hydration(&missing_cover_art));

        let missing_series = AudiobookMetadata {
            series_part: Some("1".to_string()),
            cover_art: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        assert!(needs_ffmpeg_partial_hydration(&missing_series));

        let missing_series_part = AudiobookMetadata {
            series: Some("Series".to_string()),
            cover_art: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        assert!(needs_ffmpeg_partial_hydration(&missing_series_part));
    }
}
