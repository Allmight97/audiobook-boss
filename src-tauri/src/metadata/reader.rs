//! Metadata reading via ffmpeg-next
use super::{mp4ameta_bridge, split_series_list, AudiobookMetadata};
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::path::Path;

/// Reads container-level metadata and attached cover art using ffmpeg-next.
pub fn read_metadata<P: AsRef<Path>>(file_path: P) -> Result<AudiobookMetadata> {
    let path = file_path.as_ref();

    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            path.display()
        )));
    }

    if mp4ameta_bridge::is_mp4_container(path) {
        match mp4ameta_bridge::read_metadata(path) {
            Ok(mut metadata) => {
                metadata.cover_art = normalize_cover_art(metadata.cover_art);
                if metadata.series.is_none()
                    || metadata.series_part.is_none()
                    || metadata.subseries.is_none()
                    || metadata.subseries_part.is_none()
                    || metadata.cover_art.is_none()
                {
                    if let Ok(fallback) = read_metadata_with_ffmpeg(path) {
                        metadata.series = metadata.series.or(fallback.series);
                        metadata.series_part = metadata.series_part.or(fallback.series_part);
                        metadata.subseries = metadata.subseries.or(fallback.subseries);
                        metadata.subseries_part =
                            metadata.subseries_part.or(fallback.subseries_part);
                        if metadata.cover_art.is_none() {
                            metadata.cover_art = normalize_cover_art(fallback.cover_art);
                        }
                    }
                }
                return Ok(metadata);
            }
            Err(e) => {
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

    // Series metadata: prefer canonical tags, fall back to legacy/movement tags
    let series_raw = first_tag(
        &dict,
        &["series", "----:com.apple.iTunes:SERIES", "show", "MVNM"],
    );
    let series_part_raw = first_tag(
        &dict,
        &[
            "series-part",
            "----:com.apple.iTunes:SERIES-PART",
            "episode_sort",
            "MVIN",
        ],
    );
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
        .and_then(|v| v.parse::<u32>().ok());

    // Attached picture (cover art)
    metadata.cover_art = extract_attached_pic(&ictx);

    Ok(metadata)
}

fn normalize_cover_art(cover_art: Option<Vec<u8>>) -> Option<Vec<u8>> {
    cover_art.filter(|bytes| !bytes.is_empty())
}

fn first_tag(dict: &ff::DictionaryRef<'_>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| dict.get(key).map(str::to_string))
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

// tests are defined in `src-tauri/tests/metadata_reader.rs`
