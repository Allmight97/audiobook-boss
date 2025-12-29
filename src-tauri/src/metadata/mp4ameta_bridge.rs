use crate::errors::{AppError, Result};
use crate::metadata::ffmpeg_bridge::detect_cover_art_format;
use crate::metadata::AudiobookMetadata;
use mp4ameta::{Data, FreeformIdent, Img, ImgFmt, MediaType, Tag, WriteConfig};
use std::path::Path;

const ITUNES_MEAN: &str = "com.apple.iTunes";

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
    metadata.date = tag.year().and_then(|y| y.parse::<u32>().ok());

    metadata.series = read_series(&tag);
    metadata.series_part = read_series_part(&tag);

    metadata.cover_art = tag.artwork().map(|img| img.data.to_vec());

    Ok(metadata)
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

    if let Some(date) = metadata.date {
        tag.set_year(date.to_string());
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

    if let Some(ref album_sort) = metadata.album_sort {
        tag.set_album_sort_order(album_sort);
    }

    tag.set_media_type(MediaType::AudioBook);

    let series_present = metadata
        .series
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let series_part_present = metadata
        .series_part
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);

    if let Some(ref series) = metadata.series {
        if series.trim().is_empty() {
            let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES");
            tag.remove_data_of(&ident);
            tag.remove_movement();
        } else {
            let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES");
            tag.set_data(ident, Data::Utf8(series.to_string()));
            tag.set_movement(series);
            tag.set_show_movement();
        }
    }

    if let Some(ref series_part) = metadata.series_part {
        if series_part.trim().is_empty() {
            let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES-PART");
            tag.remove_data_of(&ident);
            tag.remove_movement_index();
        } else {
            let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES-PART");
            tag.set_data(ident, Data::Utf8(series_part.to_string()));
            if let Some(index) = parse_series_part(series_part) {
                tag.set_movement_index(index);
                tag.set_show_movement();
            }
        }
    }

    if (metadata.series.is_some() || metadata.series_part.is_some())
        && !(series_present || series_part_present)
    {
        tag.remove_show_movement();
    }

    if let Some(ref cover_art) = metadata.cover_art {
        if cover_art.is_empty() {
            tag.remove_artworks();
        } else if let Some(image) = cover_art_to_img(cover_art)? {
            tag.set_artwork(image);
        }
    }

    Ok(())
}

fn read_series(tag: &Tag) -> Option<String> {
    let series = {
        let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES");
        let value = tag.strings_of(&ident).next().map(str::to_string);
        value
    };
    series.or_else(|| tag.movement().map(str::to_string))
}

fn read_series_part(tag: &Tag) -> Option<String> {
    let series_part = {
        let ident = FreeformIdent::new_static(ITUNES_MEAN, "SERIES-PART");
        let value = tag.strings_of(&ident).next().map(str::to_string);
        value
    };
    series_part.or_else(|| tag.movement_index().map(|idx| idx.to_string()))
}

fn parse_series_part(value: &str) -> Option<u16> {
    let raw = value.split('/').next()?.trim();
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
