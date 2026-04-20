use crate::errors::{AppError, Result};
use crate::metadata::ffmpeg_bridge::detect_cover_art_format;
use crate::metadata::tag_registry::{ITUNES_MEAN, SERIES_FREEFORM_NAME, SERIES_PART_FREEFORM_NAME};
use crate::metadata::{
    build_series_list, normalize_publication_date, split_series_list, AudiobookMetadata,
};
use mp4ameta::{Data, FreeformIdent, Img, ImgFmt, MediaType, Tag, WriteConfig};
use std::path::Path;

pub fn is_mp4_container(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    matches!(ext.to_ascii_lowercase().as_str(), "m4a" | "m4b" | "mp4")
}

pub fn read_metadata(path: &Path) -> Result<AudiobookMetadata> {
    let tag = Tag::read_from_path(path)
        .map_err(|e| AppError::General(format!("mp4ameta read failed: {e}")))?;

    let mut metadata = AudiobookMetadata::new();

    metadata.title = tag.title().map(str::to_string);
    metadata.album = tag.album().map(str::to_string);
    metadata.artist = tag
        .artist()
        .or_else(|| tag.album_artist())
        .map(str::to_string);
    metadata.composer = tag.composer().map(str::to_string);
    metadata.genre = tag.genre().map(str::to_string);
    metadata.comment = tag.comment().map(str::to_string);
    metadata.description = tag.description().map(str::to_string);
    metadata.album_sort = tag.album_sort_order().map(str::to_string);
    metadata.date = tag.year().and_then(normalize_publication_date);
    metadata.track = read_tuple_field(tag.track());
    metadata.disk = read_tuple_field(tag.disc());

    let series_raw = read_series_raw(&tag);
    let series_part_raw = read_series_part_raw(&tag);
    let (series, subseries) = split_series_list(series_raw.as_deref());
    let (series_part, subseries_part) = split_series_list(series_part_raw.as_deref());
    metadata.series = series;
    metadata.series_part = series_part;
    metadata.subseries = subseries;
    metadata.subseries_part = subseries_part;

    metadata.cover_art = tag.artwork().map(|img| img.data.to_vec());

    Ok(metadata)
}

fn read_tuple_field((number, total): (Option<u16>, Option<u16>)) -> Option<(u32, Option<u32>)> {
    number.map(|number| (u32::from(number), total.map(u32::from)))
}

pub fn write_metadata(path: &Path, metadata: &AudiobookMetadata) -> Result<()> {
    let mut tag = Tag::read_from_path(path)
        .map_err(|e| AppError::General(format!("mp4ameta read failed: {e}")))?;

    apply_metadata(&mut tag, metadata)?;

    let config = WriteConfig {
        write_meta_items: true,
        ..WriteConfig::NONE
    };

    tag.write_with_path(path, &config)
        .map_err(|e| AppError::General(format!("mp4ameta write failed: {e}")))?;

    Ok(())
}

fn apply_metadata(tag: &mut Tag, metadata: &AudiobookMetadata) -> Result<()> {
    let (effective_title, effective_series, effective_series_part) =
        resolve_effective_metadata(tag, metadata);

    if let Some(ref title) = metadata.title {
        tag.set_title(title);
    }

    if let Some(ref artist) = metadata.artist {
        tag.set_artist(artist);
        tag.set_album_artist(artist);
    }

    if let Some(ref album) = metadata.album {
        tag.set_album(album);
    }

    if let Some(ref composer) = metadata.composer {
        tag.set_composer(composer);
    }

    if let Some(ref genre) = metadata.genre {
        tag.set_genre(genre);
    }

    if let Some(ref date) = metadata.date {
        let trimmed = date.trim();
        if trimmed.is_empty() {
            tag.remove_year();
        } else {
            tag.set_year(trimmed.to_string());
        }
    }

    if let Some(ref comment) = metadata.comment {
        tag.set_comment(comment);
    }

    if let Some(ref description) = metadata.description {
        if description.trim().is_empty() {
            tag.remove_descriptions();
        } else {
            tag.set_description(description);
        }
    }

    apply_album_sort(
        tag,
        metadata,
        effective_title.as_deref(),
        effective_series.as_deref(),
        effective_series_part.as_deref(),
    );

    tag.set_media_type(MediaType::AudioBook);

    apply_series_metadata(tag, metadata);
    apply_cover_art(tag, metadata)?;

    Ok(())
}

fn apply_series_metadata(tag: &mut Tag, metadata: &AudiobookMetadata) {
    let (series_value, series_part_value) = build_series_list(
        metadata.series.as_deref(),
        metadata.series_part.as_deref(),
        metadata.subseries.as_deref(),
        metadata.subseries_part.as_deref(),
    );
    let series_present = series_value
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let series_part_present = series_part_value
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);

    let primary_series = metadata
        .series
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let primary_series_part = metadata
        .series_part
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if metadata.series.is_some() {
        let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_FREEFORM_NAME);
        // Always remove existing atoms first to prevent duplication
        tag.remove_data_of(&ident);
        tag.remove_movement();
        tag.remove_tv_show_name();
        tag.remove_tv_show_name_sort_order();

        if let Some(series_value) = series_value.as_deref() {
            tag.set_data(ident, Data::Utf8(series_value.to_string()));
        }
        if let Some(primary_series) = primary_series {
            tag.set_movement(primary_series);
            tag.set_show_movement();
        }
    }

    if metadata.series_part.is_some() {
        let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART_FREEFORM_NAME);
        // Always remove existing atoms first to prevent duplication
        tag.remove_data_of(&ident);
        tag.remove_movement_index();
        tag.remove_tv_episode();
        tag.remove_tv_episode_name();

        if let Some(series_part_value) = series_part_value.as_deref() {
            tag.set_data(ident, Data::Utf8(series_part_value.to_string()));
        }
        if let Some(primary_series_part) = primary_series_part {
            if let Some(index) = parse_series_part(primary_series_part) {
                tag.set_movement_index(index);
                tag.set_show_movement();
            }
        }
    }

    let should_clear_series = metadata.series.is_some()
        || metadata.series_part.is_some()
        || metadata.subseries.is_some()
        || metadata.subseries_part.is_some();
    if should_clear_series && !(series_present || series_part_present) {
        tag.remove_show_movement();
    }
}

fn apply_cover_art(tag: &mut Tag, metadata: &AudiobookMetadata) -> Result<()> {
    if let Some(ref cover_art) = metadata.cover_art {
        if cover_art.is_empty() {
            tag.remove_artworks();
        } else if let Some(image) = cover_art_to_img(cover_art)? {
            tag.set_artwork(image);
        }
    }

    Ok(())
}

fn resolve_effective_metadata(
    tag: &Tag,
    metadata: &AudiobookMetadata,
) -> (Option<String>, Option<String>, Option<String>) {
    let existing_title = tag.title().map(str::to_string);
    let (existing_series, _) = split_series_list(read_series_raw(tag).as_deref());
    let (existing_series_part, _) = split_series_list(read_series_part_raw(tag).as_deref());

    let effective_title = metadata
        .title
        .as_deref()
        .map(str::to_string)
        .or(existing_title);
    let effective_series = metadata
        .series
        .as_deref()
        .map(str::to_string)
        .or(existing_series);
    let effective_series_part = metadata
        .series_part
        .as_deref()
        .map(str::to_string)
        .or(existing_series_part);

    (effective_title, effective_series, effective_series_part)
}

fn apply_album_sort(
    tag: &mut Tag,
    metadata: &AudiobookMetadata,
    effective_title: Option<&str>,
    effective_series: Option<&str>,
    effective_series_part: Option<&str>,
) {
    let should_recompute =
        metadata.series.is_some() || metadata.title.is_some() || metadata.series_part.is_some();

    if !should_recompute {
        if let Some(ref album_sort) = metadata.album_sort {
            tag.set_album_sort_order(album_sort);
            return;
        }
    }

    if let (Some(series), Some(title)) = (effective_series, effective_title) {
        if let Some(computed) =
            crate::metadata::compute_album_sort(series, effective_series_part, title)
        {
            tag.set_album_sort_order(&computed);
        }
    }
}

fn read_series_raw(tag: &Tag) -> Option<String> {
    let series = {
        let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_FREEFORM_NAME);
        let value = tag.strings_of(&ident).next().map(str::to_string);
        value
    };
    let series = series.or_else(|| tag.tv_show_name().map(str::to_string));
    // FALLBACK[FB-007]: trigger=legacy files store series in movement tag only
    // observe=series metadata compatibility tests + fallback register tracking
    // sunset=2026-05-31 issue=#202
    series.or_else(|| tag.movement().map(str::to_string))
}

fn read_series_part_raw(tag: &Tag) -> Option<String> {
    let series_part = {
        let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART_FREEFORM_NAME);
        let value = tag.strings_of(&ident).next().map(str::to_string);
        value
    };
    let series_part = series_part.or_else(|| tag.tv_episode().map(|episode| episode.to_string()));
    // FALLBACK[FB-007]: trigger=legacy files store series-part in movement_index only
    // observe=series metadata compatibility tests + fallback register tracking
    // sunset=2026-05-31 issue=#202
    series_part.or_else(|| tag.movement_index().map(|idx| idx.to_string()))
}

fn parse_series_part(value: &str) -> Option<u16> {
    let raw = value.trim();
    if raw.is_empty() || raw.contains('/') {
        return None;
    }
    let parsed = raw.parse::<u16>().ok()?;
    if parsed == 0 {
        None
    } else {
        Some(parsed)
    }
}

fn cover_art_to_img(bytes: &[u8]) -> Result<Option<Img<Vec<u8>>>> {
    let Some(format) = detect_cover_art_format(bytes) else {
        log::warn!("Unsupported cover art format for mp4ameta (only JPEG/PNG). Skipping art.");
        return Ok(None);
    };

    let img = match format {
        crate::metadata::CoverFormat::Jpeg => Img::new(ImgFmt::Jpeg, bytes.to_vec()),
        crate::metadata::CoverFormat::Png => Img::new(ImgFmt::Png, bytes.to_vec()),
    };
    Ok(Some(img))
}
