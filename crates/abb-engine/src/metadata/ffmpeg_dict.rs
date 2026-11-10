use super::container::Container;
use super::field_schema::{ALBUM_SORT_KEYS, MP4_ALBUM_SORT_KEY};
use super::{AudiobookMetadata, MetadataWritePlan};
use crate::metadata::metadata_sinks::{tag_pairs_for_ops, ExistingKeyRemoval};
use abb_metadata_core::PatchOp;

/// The container tags a save writes: the file's own tags without the ones
/// the plan replaces or clears, then the plan's values. Kept tags keep their
/// bytes.
pub(crate) fn merge_metadata_with_plan(
    existing: &abb_ffmpeg::Tags,
    plan: &MetadataWritePlan,
    container: &Container,
) -> crate::errors::Result<abb_ffmpeg::Tags> {
    let removal = ExistingKeyRemoval::for_plan(plan);
    let new_album_sort = match &plan.album_sort {
        Some(PatchOp::Set(value)) => Some(value.as_str()),
        _ => None,
    };
    let step = abb_ffmpeg::Step::SetTags;
    let mut merged = abb_ffmpeg::Tags::default();
    for (key, value) in existing.iter() {
        let name = key.display();
        let is_album_sort = ALBUM_SORT_KEYS
            .iter()
            .any(|sort_key| name.eq_ignore_ascii_case(sort_key));
        if (plan.album_sort.is_some() && is_album_sort) || removal.removes(&name) {
            continue;
        }
        merged.insert(key.clone(), value.clone());
    }

    for (key, value) in tag_pairs_for_ops(&plan.ops) {
        merged.push(key, &value, step)?;
    }
    if let Some(value) = new_album_sort {
        merged.push(container.album_sort_key(), value, step)?;
    }

    Ok(merged)
}

/// Sets the container tags an encode writes for `metadata`.
pub(crate) fn set_container_metadata(
    output: &mut abb_ffmpeg::Output,
    metadata: &AudiobookMetadata,
) -> crate::errors::Result<()> {
    let plan = MetadataWritePlan::from_metadata(metadata);
    let album_sort = if matches!(plan.album_sort, Some(PatchOp::Set(_))) {
        "set"
    } else {
        "preserve"
    };
    super::plan_log::log_field_decisions(&plan, album_sort, "ffmpeg_encode", "current_encoder");
    let mut pairs = tag_pairs_for_ops(&plan.ops);
    if let Some(PatchOp::Set(value)) = &plan.album_sort {
        pairs.push((MP4_ALBUM_SORT_KEY, value.clone()));
    }
    let step = abb_ffmpeg::Step::SetTags;
    let mut tags = abb_ffmpeg::Tags::default();
    for (key, value) in pairs {
        tags.push(key, &value, step)?;
    }
    output.set_tags(&tags)?;
    log::debug!("Container metadata set");
    Ok(())
}

#[cfg(test)]
#[path = "ffmpeg_dict_tests.rs"]
mod tests;
