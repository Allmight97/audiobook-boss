//! Metadata handling for audiobook files
//!
//! This module provides functionality to read and write metadata from/to audio
//! files through container-aware metadata strategies. Pure metadata intent and
//! naming facts live in `abb-metadata-core` so focused tests can run without
//! compiling Tauri, FFmpeg, or container adapters.

use crate::errors::{AppError, Result};

pub mod reader;
pub(crate) mod tag_registry;

mod container;
#[cfg(test)]
mod contract_tests;
mod cover_art;
mod embedded_cover;
mod ffi;
mod ffmpeg_dict;
mod field_schema;
mod intent_plan;
mod metadata_ops;
mod metadata_sinks;
mod mp4_covr;
mod mp4ameta_bridge;
mod passthrough;
mod remux;
mod thumbnail;

pub use abb_metadata_core::{
    build_series_list, compute_album_sort, normalize_publication_date, publication_year_from_date,
    split_series_list, validate_metadata_intent_patch, AlbumSortPatchOp, AudiobookMetadata,
    MetadataCoreError, MetadataIntentPatch, MetadataIntentValidationResult, NamingMetadata,
    PatchOp,
};
pub(crate) use abb_metadata_core::{AlbumSortWriteAction, MetadataWritePlan};
pub use intent_plan::CoverArtPassthroughPolicy;
pub(crate) use intent_plan::{
    plan_metadata_outcome, plan_metadata_write_for_path, MetadataOutcomePlan,
    MetadataOutcomeRequest,
};

impl From<MetadataCoreError> for AppError {
    fn from(error: MetadataCoreError) -> Self {
        match error {
            MetadataCoreError::InvalidInput(message) => AppError::InvalidInput(message),
        }
    }
}

pub use reader::{display_tags_from_ffmpeg_dict, read_metadata};
pub use thumbnail::read_audio_cover_thumbnail;
pub(crate) use thumbnail::{optimize_cover_art, prepare_cover_art_for_write};

pub use cover_art::{
    add_cover_art_stream_pre_header, write_cover_art_packet_post_header, CoverFormat,
};
pub use ffmpeg_dict::{set_container_metadata, validate_metadata_compatibility};
pub(crate) use passthrough::prepare_output_cover_art;
pub use passthrough::{
    add_chapters_to_output, extract_passthrough_metadata, PassthroughMetadata, PassthroughSource,
};

/// Applies an explicit metadata intent patch to a real file: validate/plan,
/// then container-adapted write. The single save entry for command ingress
/// and integration round-trip proof (media-execution lane).
pub fn save_metadata_intent(path: &std::path::Path, patch: &MetadataIntentPatch) -> Result<()> {
    let plan = plan_metadata_write_for_path(path, patch)?;
    save_metadata_with_plan(path, &plan)
}

pub(crate) fn save_metadata_with_plan(
    path: &std::path::Path,
    plan: &MetadataWritePlan,
) -> Result<()> {
    match container::classify(path)? {
        container::ContainerRoute::Mp4Family => {
            mp4ameta_bridge::write_metadata_with_plan(path, plan)
        }
        route => remux::rewrite_metadata_with_ffmpeg_plan_as(
            path,
            Some(plan),
            None,
            route.remux_output_format(),
        ),
    }
}

/// Finalizes a freshly produced artifact's metadata in one container-aware
/// External-adapter artifact finalize: a remux pass carries chapters and cover
/// art, then MP4-family tag truth is rewritten through the mp4ameta adapter
/// chosen by actual container classification. The FFmpeg mov muxer silently
/// drops dictionary keys outside its known-atom table (series, series-part, the
/// iTunes freeform mirrors, sort_album), so MP4-family artifacts must not rely
/// on the remux for tag truth. Re-exported at the crate root for the
/// media-execution lane's artifact-finalize proof.
pub fn finalize_artifact_metadata(
    path: &std::path::Path,
    metadata: Option<&AudiobookMetadata>,
    passthrough: Option<&PassthroughMetadata>,
) -> Result<()> {
    if metadata.is_none() && passthrough.is_none() {
        return Ok(());
    }

    remux::rewrite_metadata_with_ffmpeg(path, metadata, passthrough)?;

    if let Some(metadata) = metadata {
        if should_write_finalized_metadata(path)? {
            write_finalized_metadata(path, metadata)?;
        }
    }
    Ok(())
}

pub(crate) fn write_cover_art_to_file(path: &std::path::Path, cover_data: Vec<u8>) -> Result<()> {
    let metadata = AudiobookMetadata {
        cover_art: Some(cover_data),
        ..Default::default()
    };
    let plan = MetadataWritePlan::from_metadata(metadata);
    save_metadata_with_plan(path, &plan)
}

pub(crate) fn should_write_finalized_metadata(path: &std::path::Path) -> Result<bool> {
    Ok(matches!(
        container::classify(path)?,
        container::ContainerRoute::Mp4Family
    ))
}

pub(crate) fn write_finalized_metadata(
    path: &std::path::Path,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    mp4ameta_bridge::write_metadata(path, metadata)
}
