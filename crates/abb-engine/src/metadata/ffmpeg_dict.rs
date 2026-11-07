use super::field_schema::{album_sort_key, ALBUM_SORT_KEYS, MP4_ALBUM_SORT_KEY};
use super::{AudiobookMetadata, MetadataWritePlan};
use crate::metadata::metadata_sinks::{ffmpeg_dict_for_ops, ExistingKeyRemoval};
use abb_metadata_core::PatchOp;

use ffmpeg_next as ff;

pub(crate) fn merge_metadata_with_plan<'a>(
    existing: ff::Dictionary<'a>,
    plan: &MetadataWritePlan,
    container: &str,
) -> ff::Dictionary<'a> {
    let removal = ExistingKeyRemoval::for_plan(plan);
    let new_album_sort = match &plan.album_sort {
        Some(PatchOp::Set(value)) => Some(value.as_str()),
        _ => None,
    };
    let mut merged = ff::Dictionary::new();
    for (key, value) in existing.iter() {
        let is_album_sort = ALBUM_SORT_KEYS
            .iter()
            .any(|sort_key| key.eq_ignore_ascii_case(sort_key));
        if (plan.album_sort.is_some() && is_album_sort) || removal.removes(key) {
            continue;
        }
        merged.set(key, value);
    }

    for (key, value) in ffmpeg_dict_for_ops(&plan.ops).iter() {
        merged.set(key, value);
    }
    if let Some(value) = new_album_sort {
        merged.set(album_sort_key(container), value);
    }

    merged
}

pub fn set_container_metadata(
    octx: &mut ff::format::context::Output,
    metadata: &AudiobookMetadata,
) {
    let plan = MetadataWritePlan::from_metadata(metadata);
    let album_sort = if matches!(plan.album_sort, Some(PatchOp::Set(_))) {
        "set"
    } else {
        "preserve"
    };
    super::plan_log::log_field_decisions(&plan, album_sort, "ffmpeg_encode", "current_encoder");
    let mut dict = ffmpeg_dict_for_ops(&plan.ops);
    if let Some(PatchOp::Set(value)) = &plan.album_sort {
        dict.set(MP4_ALBUM_SORT_KEY, value);
    }
    octx.set_metadata(dict);

    log::debug!("Container metadata set via ffmpeg-next");
}

#[cfg(test)]
#[path = "ffmpeg_dict_tests.rs"]
mod tests;
