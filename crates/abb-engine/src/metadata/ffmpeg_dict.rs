use super::field_schema::{album_sort_key, ALBUM_SORT_KEYS, MP4_ALBUM_SORT_KEY};
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
    container: &str,
) -> Result<ff::Dictionary<'a>> {
    let metadata = &plan.metadata;
    let removal = ExistingKeyRemoval::for_metadata(metadata);
    let overrides = metadata_to_ffmpeg_dict(metadata)?;
    let new_album_sort = match &plan.album_sort {
        AlbumSortWriteAction::Set(value) if !value.trim().is_empty() => Some(value.as_str()),
        _ => overrides.get(MP4_ALBUM_SORT_KEY),
    };
    let replaces_album_sort =
        plan.album_sort == AlbumSortWriteAction::Clear || new_album_sort.is_some();
    let mut merged = ff::Dictionary::new();
    for (key, value) in existing.iter() {
        let is_album_sort = ALBUM_SORT_KEYS
            .iter()
            .any(|sort_key| key.eq_ignore_ascii_case(sort_key));
        if (replaces_album_sort && is_album_sort) || removal.removes(key) {
            continue;
        }
        merged.set(key, value);
    }

    for (key, value) in overrides
        .iter()
        .filter(|(key, _)| *key != MP4_ALBUM_SORT_KEY)
    {
        merged.set(key, value);
    }
    if let Some(value) = new_album_sort {
        merged.set(album_sort_key(container), value);
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
