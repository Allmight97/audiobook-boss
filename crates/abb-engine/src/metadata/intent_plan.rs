use super::{AudiobookMetadata, MetadataIntentPatch, MetadataWritePlan, NamingMetadata};
use crate::errors::Result;
use crate::metadata::PassthroughMetadata;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverArtPassthroughPolicy {
    Preserve,
    /// The user chose this title's cover, so the output must carry it.
    Chosen,
    SuppressAfterExplicitClear,
}

impl CoverArtPassthroughPolicy {
    pub(crate) fn from_intent_patch(patch: Option<&MetadataIntentPatch>) -> Self {
        match patch.and_then(|patch| patch.cover_art.as_ref()) {
            Some(super::PatchOp::Clear) => Self::SuppressAfterExplicitClear,
            Some(super::PatchOp::Set(_)) => Self::Chosen,
            None => Self::Preserve,
        }
    }

    pub(crate) fn apply_to_passthrough(
        self,
        passthrough: Option<PassthroughMetadata>,
    ) -> Option<PassthroughMetadata> {
        match self {
            Self::Preserve | Self::Chosen => passthrough,
            Self::SuppressAfterExplicitClear => {
                log::info!("cover_art_plan decision=skip_passthrough reason=explicit_cover_clear");
                passthrough.and_then(PassthroughMetadata::without_cover_art)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MetadataOutcomePlan {
    /// The anchor source's own tags as read for this plan, so a later edit
    /// can be planned again without reading the source.
    pub(crate) source_metadata: AudiobookMetadata,
    pub(crate) effective_metadata: AudiobookMetadata,
    pub(crate) naming_metadata: NamingMetadata,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
    /// The patch a route that writes onto the source's own tags applies: the
    /// user's intent plus the processing album sort when the user stated none.
    pub(crate) write_intent: Option<MetadataIntentPatch>,
}

pub(crate) fn plan_metadata_outcome(
    input_path: &Path,
    intent_patch: Option<&MetadataIntentPatch>,
) -> Result<MetadataOutcomePlan> {
    let source = crate::metadata::read_metadata(input_path)?;
    plan_metadata_outcome_from(source, intent_patch)
}

pub(crate) fn plan_metadata_outcome_from(
    source: AudiobookMetadata,
    intent_patch: Option<&MetadataIntentPatch>,
) -> Result<MetadataOutcomePlan> {
    let effective_metadata = resolve_effective_processing_metadata(source.clone(), intent_patch)?;
    let naming_metadata = resolve_naming_metadata(&effective_metadata, intent_patch);
    let write_intent = processing_write_intent(intent_patch, &effective_metadata);
    Ok(MetadataOutcomePlan {
        source_metadata: source,
        effective_metadata,
        naming_metadata,
        cover_art_passthrough: CoverArtPassthroughPolicy::from_intent_patch(intent_patch),
        write_intent,
    })
}

pub(crate) fn plan_metadata_write_for_path(
    input_path: &Path,
    patch: &MetadataIntentPatch,
) -> Result<MetadataWritePlan> {
    let source_metadata = crate::metadata::read_metadata(input_path)?;
    Ok(MetadataWritePlan::from_patch(patch, source_metadata)?)
}

fn processing_write_intent(
    patch: Option<&MetadataIntentPatch>,
    effective: &AudiobookMetadata,
) -> Option<MetadataIntentPatch> {
    let mut write_intent = patch.cloned().unwrap_or_default();
    if write_intent.album_sort.is_none() {
        write_intent.album_sort = effective
            .series
            .as_deref()
            .zip(effective.title.as_deref())
            .and_then(|(series, title)| {
                crate::metadata::compute_album_sort(series, effective.series_part.as_deref(), title)
            })
            .map(crate::metadata::AlbumSortPatchOp::Set);
    }
    (write_intent != MetadataIntentPatch::default()).then_some(write_intent)
}

fn resolve_effective_processing_metadata(
    source: AudiobookMetadata,
    patch: Option<&MetadataIntentPatch>,
) -> Result<AudiobookMetadata> {
    let mut effective = match patch {
        Some(patch) => patch.apply_to_metadata(source)?,
        None => source,
    };
    if patch.is_none_or(|patch| patch.album_sort.is_none()) {
        effective.album_sort = crate::metadata::processing_album_sort(&effective);
    }
    Ok(effective)
}

fn resolve_naming_metadata(
    resolved_metadata: &AudiobookMetadata,
    patch: Option<&MetadataIntentPatch>,
) -> NamingMetadata {
    let mut naming_metadata = NamingMetadata::from_metadata(resolved_metadata);
    if !patch.is_some_and(MetadataIntentPatch::touches_series_family) {
        naming_metadata.scrub_legacy_source_series_parts_for_naming();
    }
    naming_metadata
}

#[cfg(test)]
#[path = "intent_plan_tests.rs"]
mod tests;
