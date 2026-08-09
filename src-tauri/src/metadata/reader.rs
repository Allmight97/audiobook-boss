//! Metadata reading for audiobook files.
use super::{mp4ameta_bridge, normalize_publication_date, split_series_list, AudiobookMetadata};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::field_schema::TagField;
use ffmpeg_next as ff;
use std::path::Path;

/// Reads audiobook metadata, preferring mp4ameta for probed MP4-family containers.
pub fn read_metadata<P: AsRef<Path>>(file_path: P) -> Result<AudiobookMetadata> {
    let path = file_path.as_ref();

    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            sanitize_path_for_display(path)
        )));
    }

    ff::init().map_err(AppError::Ffmpeg)?;
    let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
    let route = super::container::classify_format_name(ictx.format().name());

    if matches!(route, super::container::ContainerRoute::Mp4Family) {
        drop(ictx);
        let mut metadata = mp4ameta_bridge::read_metadata(path)?;
        metadata.cover_art = normalize_cover_art(metadata.cover_art);
        return Ok(metadata);
    }

    read_metadata_with_ffmpeg_input(&ictx)
}

pub(crate) fn read_cover_art_for_thumbnail(path: &Path) -> Result<Option<Vec<u8>>> {
    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            sanitize_path_for_display(path)
        )));
    }

    // MP4-family: bound artwork via atom walk without opening FFmpeg (which can
    // materialize a full attached-pic / covr packet during demuxer open).
    if super::mp4_covr::looks_like_mp4_family_file(path)? {
        return mp4ameta_bridge::read_cover_art_for_thumbnail(path);
    }

    super::embedded_cover::read_bounded_non_mp4_cover_art(path)
}

fn read_metadata_with_ffmpeg_input(ictx: &ff::format::context::Input) -> Result<AudiobookMetadata> {
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
    metadata.track = parse_position_field(
        first_tag(&dict, TagField::Track).as_deref(),
        first_tag_keys(
            &dict,
            TagField::Track.read_total_keys().expect("track totals"),
        )
        .as_deref(),
    );
    metadata.disk = parse_position_field(
        first_tag(&dict, TagField::Disk).as_deref(),
        first_tag_keys(
            &dict,
            TagField::Disk.read_total_keys().expect("disk totals"),
        )
        .as_deref(),
    );

    // Series metadata: prefer canonical tags, then supported legacy aliases.
    let series_raw = first_tag(&dict, TagField::Series);
    let series_part_raw = first_tag(&dict, TagField::SeriesPart);
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
    metadata.cover_art = extract_attached_pic(ictx);

    Ok(metadata)
}

fn normalize_cover_art(cover_art: Option<Vec<u8>>) -> Option<Vec<u8>> {
    cover_art.filter(|bytes| !bytes.is_empty())
}

fn first_tag_with_lookup<F>(keys: &[&str], mut lookup: F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    keys.iter().find_map(|key| lookup(key))
}

fn first_tag(dict: &ff::DictionaryRef<'_>, field: TagField) -> Option<String> {
    first_tag_keys(dict, field.read_keys())
}

/// Returns the display-safe title and artist tags from an already-open FFmpeg input.
pub fn display_tags_from_ffmpeg_dict(
    dict: &ff::DictionaryRef<'_>,
) -> (Option<String>, Option<String>) {
    (
        first_tag(dict, TagField::Title),
        first_tag(dict, TagField::Artist),
    )
}

fn first_tag_keys(dict: &ff::DictionaryRef<'_>, keys: &[&str]) -> Option<String> {
    first_tag_with_lookup(keys, |key| dict.get(key).map(str::to_string))
}

fn parse_position_field(primary: Option<&str>, total: Option<&str>) -> Option<(u32, Option<u32>)> {
    let primary = primary?.trim();
    if primary.is_empty() {
        return None;
    }

    if let Some((number, total)) = parse_number_pair(primary) {
        return Some((number, total));
    }

    let number = primary.parse::<u32>().ok()?;
    let total = total.and_then(parse_total_value);
    Some((number, total))
}

fn parse_number_pair(value: &str) -> Option<(u32, Option<u32>)> {
    let (number, total) = value.split_once('/')?;
    let number = number.trim().parse::<u32>().ok()?;
    let total = parse_total_value(total);
    Some((number, total))
}

fn parse_total_value(value: &str) -> Option<u32> {
    value.trim().parse::<u32>().ok()
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
    use super::{first_tag_with_lookup, parse_position_field};
    use crate::metadata::field_schema::TagField;
    use std::collections::BTreeMap;

    #[test]
    fn prefers_canonical_series_part_key() {
        let tags = BTreeMap::from([
            ("series-part", "1".to_string()),
            ("----:com.apple.iTunes:SERIES-PART", "2".to_string()),
            ("episode_sort", "3".to_string()),
        ]);
        let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
            tags.get(key).cloned()
        });
        assert_eq!(selected.as_deref(), Some("1"));
    }

    #[test]
    fn falls_back_to_itunes_series_part_key() {
        let tags = BTreeMap::from([
            ("----:com.apple.iTunes:SERIES-PART", "2".to_string()),
            ("episode_sort", "3".to_string()),
        ]);
        let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
            tags.get(key).cloned()
        });
        assert_eq!(selected.as_deref(), Some("2"));
    }

    #[test]
    fn falls_back_to_episode_sort_for_series_part() {
        let tags = BTreeMap::from([("episode_sort", "3".to_string())]);
        let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
            tags.get(key).cloned()
        });
        assert_eq!(selected.as_deref(), Some("3"));
    }

    #[test]
    fn does_not_treat_part_as_series_part_key() {
        let tags = BTreeMap::from([("part", "9".to_string())]);
        let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
            tags.get(key).cloned()
        });
        assert!(selected.is_none());
    }

    #[test]
    fn falls_back_to_show_for_series() {
        let tags = BTreeMap::from([("show", "Show Series".to_string())]);
        let selected =
            first_tag_with_lookup(TagField::Series.read_keys(), |key| tags.get(key).cloned());
        assert_eq!(selected.as_deref(), Some("Show Series"));
    }

    #[test]
    fn parses_track_number_with_inline_total() {
        assert_eq!(
            parse_position_field(Some("3/12"), None),
            Some((3, Some(12)))
        );
    }

    #[test]
    fn parses_track_number_with_separate_total() {
        assert_eq!(
            parse_position_field(Some("3"), Some("12")),
            Some((3, Some(12)))
        );
    }

    #[test]
    fn ignores_invalid_position_values() {
        assert_eq!(parse_position_field(Some("abc"), Some("12")), None);
        assert_eq!(parse_position_field(Some(""), Some("12")), None);
    }
}
