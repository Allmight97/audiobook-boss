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

pub(crate) struct MetadataOutcomeRequest<'a> {
    pub(crate) input_path: Option<&'a Path>,
    pub(crate) intent_patch: Option<&'a MetadataIntentPatch>,
}

#[derive(Debug, Clone)]
pub(crate) struct MetadataOutcomePlan {
    /// The anchor source's own tags as read for this plan, so a later edit
    /// can be planned again without reading the source.
    pub(crate) source_metadata: Option<AudiobookMetadata>,
    pub(crate) effective_metadata: Option<AudiobookMetadata>,
    pub(crate) naming_metadata: Option<NamingMetadata>,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
    /// The patch a route that writes onto the source's own tags applies: the
    /// user's intent plus the processing album sort when the user stated none.
    pub(crate) write_intent: Option<MetadataIntentPatch>,
}

pub(crate) fn plan_metadata_outcome(
    request: MetadataOutcomeRequest<'_>,
) -> Result<MetadataOutcomePlan> {
    let source = request
        .input_path
        .map(crate::metadata::read_metadata)
        .transpose()?;
    plan_metadata_outcome_from(source, request)
}

pub(crate) fn plan_metadata_outcome_from(
    source: Option<AudiobookMetadata>,
    request: MetadataOutcomeRequest<'_>,
) -> Result<MetadataOutcomePlan> {
    let effective_metadata =
        resolve_effective_processing_metadata(source.clone(), request.intent_patch)?;
    let naming_metadata = resolve_naming_metadata(
        effective_metadata.as_ref(),
        request.input_path,
        request.intent_patch,
    );
    let write_intent = processing_write_intent(request.intent_patch, effective_metadata.as_ref());
    Ok(MetadataOutcomePlan {
        source_metadata: source,
        effective_metadata,
        naming_metadata,
        cover_art_passthrough: CoverArtPassthroughPolicy::from_intent_patch(request.intent_patch),
        write_intent,
    })
}

pub(crate) fn plan_metadata_write_for_path(
    input_path: &Path,
    patch: &MetadataIntentPatch,
) -> Result<MetadataWritePlan> {
    let source_metadata = crate::metadata::read_metadata(input_path)?;
    Ok(patch.to_write_plan_with_source(source_metadata)?)
}

fn processing_write_intent(
    patch: Option<&MetadataIntentPatch>,
    effective: Option<&AudiobookMetadata>,
) -> Option<MetadataIntentPatch> {
    let mut write_intent = patch.cloned().unwrap_or_default();
    // Only a derived key is written; an underivable one already equals the
    // source's own value, so writing it would rewrite tags for no change.
    if write_intent.album_sort.is_none() {
        write_intent.album_sort = effective
            .and_then(|metadata| {
                crate::metadata::compute_album_sort(
                    metadata.series.as_deref()?,
                    metadata.series_part.as_deref(),
                    metadata.title.as_deref()?,
                )
            })
            .map(crate::metadata::AlbumSortPatchOp::Set);
    }
    (write_intent != MetadataIntentPatch::default()).then_some(write_intent)
}

fn resolve_effective_processing_metadata(
    source: Option<AudiobookMetadata>,
    patch: Option<&MetadataIntentPatch>,
) -> Result<Option<AudiobookMetadata>> {
    let mut effective = match (source, patch) {
        (Some(source), Some(patch)) => patch.apply_to_metadata(source)?,
        (Some(source), None) => source,
        (None, Some(patch)) => patch.to_processing_overlay()?,
        (None, None) => return Ok(None),
    };
    if patch.is_none_or(|patch| patch.album_sort.is_none()) {
        effective.album_sort = crate::metadata::processing_album_sort(&effective);
    }
    Ok(Some(effective))
}

fn resolve_naming_metadata(
    resolved_metadata: Option<&AudiobookMetadata>,
    input_path: Option<&Path>,
    patch: Option<&MetadataIntentPatch>,
) -> Option<NamingMetadata> {
    let mut naming_metadata = resolved_metadata.map(NamingMetadata::from_metadata)?;

    let series_family_touched = patch.is_some_and(MetadataIntentPatch::touches_series_family);
    if input_path.is_some() && !series_family_touched {
        naming_metadata.scrub_legacy_source_series_parts_for_naming();
    }

    Some(naming_metadata)
}

#[cfg(test)]
#[path = "intent_plan_tests.rs"]
mod tests;
