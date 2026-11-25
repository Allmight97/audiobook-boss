use crate::errors::{AppError, Result};
use crate::metadata::cover_art::detect_cover_art_format;
use crate::metadata::field_schema::{text_keys, ITUNES_MEAN};
use crate::metadata::metadata_sinks::{apply_field_ops, Mp4ametaSink};
use crate::metadata::{
    normalize_publication_date, split_series_list, AudiobookMetadata, MetadataWritePlan,
};
use abb_metadata_core::{PatchOp, TextField};
use mp4ameta::{FreeformIdent, Img, Tag, WriteConfig};
use std::path::Path;

pub fn read_metadata(path: &Path) -> Result<AudiobookMetadata> {
    let tag = Tag::read_from_path(path)
        .map_err(|e| AppError::General(format!("mp4ameta read failed: {e}")))?;

    let mut metadata = AudiobookMetadata::new();

    metadata.title = tag.title().map(str::to_string);
    metadata.album = tag.album().map(str::to_string);
    metadata.artist =
        join_contributors(tag.artists()).or_else(|| join_contributors(tag.album_artists()));
    metadata.composer = join_contributors(tag.composers());
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

fn join_contributors<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let values: Vec<_> = values.collect();
    (!values.is_empty()).then(|| values.join(";"))
}

fn read_tuple_field((number, total): (Option<u16>, Option<u16>)) -> Option<(u32, Option<u32>)> {
    number.map(|number| (u32::from(number), total.map(u32::from)))
}

pub fn write_metadata(path: &Path, metadata: &AudiobookMetadata) -> Result<()> {
    let plan = MetadataWritePlan::from_metadata(metadata);
    write_metadata_with_plan(path, &plan)
}

pub(crate) fn write_metadata_with_plan(path: &Path, plan: &MetadataWritePlan) -> Result<()> {
    super::plan_log::log_write_plan(plan, "mp4ameta", &crate::diagnostics::artifact_id(path));
    crate::diagnostics::stage("metadata_mp4_tags", path, || {
        write_metadata_with_plan_inner(path, plan)
    })
}

fn write_metadata_with_plan_inner(path: &Path, plan: &MetadataWritePlan) -> Result<()> {
    let mut tag = Tag::read_from_path(path)
        .map_err(|e| AppError::General(format!("mp4ameta read failed: {e}")))?;

    apply_metadata(&mut tag, plan)?;

    let config = WriteConfig {
        write_meta_items: true,
        ..WriteConfig::NONE
    };

    tag.write_with_path(path, &config)
        .map_err(|e| AppError::General(format!("mp4ameta write failed: {e}")))?;

    Ok(())
}

fn apply_metadata(tag: &mut Tag, plan: &MetadataWritePlan) -> Result<()> {
    apply_field_ops(&mut Mp4ametaSink::new(tag), &plan.ops);

    if plan.series_touched {
        tag.remove_movement();
        tag.remove_movement_index();
        tag.remove_show_movement();
    }

    apply_album_sort(tag, plan.album_sort.as_ref());

    apply_cover_art(tag, plan.cover.as_ref())?;

    Ok(())
}

fn apply_cover_art(tag: &mut Tag, cover: Option<&PatchOp<Vec<u8>>>) -> Result<()> {
    match cover {
        Some(PatchOp::Set(cover_art)) => {
            if let Some(image) = cover_art_to_img(cover_art)? {
                tag.set_artwork(image);
            }
        }
        Some(PatchOp::Clear) => tag.remove_artworks(),
        None => {}
    }

    Ok(())
}

fn apply_album_sort(tag: &mut Tag, album_sort: Option<&PatchOp<String>>) {
    match album_sort {
        Some(PatchOp::Set(value)) => tag.set_album_sort_order(value),
        Some(PatchOp::Clear) => tag.remove_album_sort_order(),
        None => {}
    }
}

fn read_series_raw(tag: &Tag) -> Option<String> {
    read_freeform_string(tag, TextField::Series).or_else(|| tag.tv_show_name().map(str::to_string))
}

fn read_series_part_raw(tag: &Tag) -> Option<String> {
    read_freeform_string(tag, TextField::SeriesPart)
        .or_else(|| tag.tv_episode().map(|episode| episode.to_string()))
}

fn read_freeform_string(tag: &Tag, field: TextField) -> Option<String> {
    text_keys(field).mp4_freeform.iter().find_map(|name| {
        tag.strings_of(&FreeformIdent::new_static(ITUNES_MEAN, name))
            .next()
            .map(str::to_string)
    })
}

fn cover_art_to_img(bytes: &[u8]) -> Result<Option<Img<Vec<u8>>>> {
    let Some(format) = detect_cover_art_format(bytes) else {
        log::warn!("Unsupported cover art format for mp4ameta (only JPEG/PNG). Skipping art.");
        return Ok(None);
    };

    Ok(Some(Img::new(format.img_fmt(), bytes.to_vec())))
}
