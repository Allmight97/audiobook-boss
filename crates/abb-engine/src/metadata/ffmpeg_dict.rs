use super::{AlbumSortWriteAction, AudiobookMetadata, MetadataWritePlan};
use crate::errors::Result;
use crate::metadata::metadata_sinks::{
    apply_metadata_field_ops_to_ffmpeg_dict, ExistingKeyRemoval,
};

use ffmpeg_next as ff;

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
    let removal = ExistingKeyRemoval::for_metadata(metadata);
    let clears_album_sort = plan.album_sort == AlbumSortWriteAction::Clear;
    let mut merged = ff::Dictionary::new();
    for (key, value) in existing.iter() {
        if (clears_album_sort && key == "sort_album") || removal.removes(key) {
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
    }

    Ok(merged)
}

pub fn set_container_metadata(
    octx: &mut ff::format::context::Output,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    super::metadata_ops::log_field_decisions(
        metadata,
        if metadata
            .album_sort
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
        {
            "set"
        } else {
            "preserve"
        },
        "ffmpeg_encode",
        "current_encoder",
    );
    let dict = metadata_to_ffmpeg_dict(metadata)?;
    octx.set_metadata(dict);

    log::debug!("Container metadata set via ffmpeg-next");
    Ok(())
}

#[cfg(test)]
#[path = "ffmpeg_dict_tests.rs"]
mod tests;
