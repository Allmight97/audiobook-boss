use super::container::Container;
use super::field_schema::{comment_key_rank, position_keys, text_keys, KeySet, ALBUM_SORT_KEYS};
use super::{mp4ameta_bridge, normalize_publication_date, split_series_list, AudiobookMetadata};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use abb_metadata_core::{PositionField, TextField};
use ffmpeg_next as ff;
use std::path::Path;

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

    if Container::of_input(&ictx).is_mp4_family() {
        drop(ictx);
        let mut metadata = mp4ameta_bridge::read_metadata(path)?;
        metadata.cover_art = normalize_cover_art(metadata.cover_art);
        return Ok(metadata);
    }

    read_metadata_with_ffmpeg_input(&ictx)
}

fn read_metadata_with_ffmpeg_input(ictx: &ff::format::context::Input) -> Result<AudiobookMetadata> {
    let dict = ictx.metadata();

    let mut metadata = AudiobookMetadata::new();

    for field in TextField::ALL {
        let value = read_text(&dict, field);
        match field {
            TextField::Date => {
                metadata.date = value.as_deref().and_then(normalize_publication_date);
            }
            TextField::Series => {
                (metadata.series, metadata.subseries) = split_series_list(value.as_deref());
            }
            TextField::SeriesPart => {
                (metadata.series_part, metadata.subseries_part) =
                    split_series_list(value.as_deref());
            }
            _ => *metadata.text_mut(field) = value,
        }
    }
    metadata.album_sort = first_tag_keys(&dict, &ALBUM_SORT_KEYS);
    for field in PositionField::ALL {
        let keys = position_keys(field);
        *metadata.position_mut(field) = parse_position_field(
            first_tag_keys(&dict, keys.read_number).as_deref(),
            first_tag_keys(&dict, keys.read_total).as_deref(),
        );
    }

    metadata.cover_art = extract_attached_pic(ictx);

    Ok(metadata)
}

fn read_text(dict: &ff::DictionaryRef<'_>, field: TextField) -> Option<String> {
    match text_keys(field).read {
        KeySet::Keys(keys) => first_tag_keys(dict, keys),
        KeySet::UserComments => read_comment(dict),
    }
}

fn read_comment(dict: &ff::DictionaryRef<'_>) -> Option<String> {
    dict.iter()
        .filter_map(|(key, value)| {
            comment_key_rank(key).map(|rank| (rank, key.to_ascii_lowercase(), value))
        })
        .min_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)))
        .map(|(_, _, value)| value.to_owned())
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

pub fn display_tags_from_ffmpeg_dict(
    dict: &ff::DictionaryRef<'_>,
) -> (Option<String>, Option<String>) {
    (
        read_text(dict, TextField::Title),
        read_text(dict, TextField::Artist),
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

pub(super) fn extract_attached_pic(ictx: &ff::format::context::Input) -> Option<Vec<u8>> {
    ictx.streams()
        .find_map(|stream| attached_pic_bytes(&stream))
}

pub(super) fn attached_pic_bytes(stream: &ff::format::stream::Stream) -> Option<Vec<u8>> {
    if !stream
        .disposition()
        .contains(ff::format::stream::Disposition::ATTACHED_PIC)
    {
        return None;
    }
    // SAFETY: `stream` borrows the live input, so its `AVStream` pointer is valid. The code reads
    // `attached_pic` only after the `ATTACHED_PIC` check, requires non-null data and positive size, and
    // copies the bytes into an owned `Vec` before the borrow ends.
    unsafe {
        let pic = (*stream.as_ptr()).attached_pic;
        if pic.data.is_null() || pic.size <= 0 {
            return None;
        }
        Some(std::slice::from_raw_parts(pic.data, pic.size as usize).to_vec())
    }
}

#[cfg(test)]
#[path = "reader_tests.rs"]
mod tests;
