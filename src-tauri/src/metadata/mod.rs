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
mod ffi;
mod ffmpeg_dict;
mod intent_plan;
mod mp4ameta_bridge;
mod passthrough;
mod remux;

pub use abb_metadata_core::{
    build_series_list, compute_album_sort, normalize_publication_date, publication_year_from_date,
    split_series_list, validate_metadata_intent_patch, AlbumSortPatchOp, AudiobookMetadata,
    MetadataCoreError, MetadataIntentPatch, MetadataIntentValidationResult, NamingMetadata,
    PatchOp,
};
pub(crate) use abb_metadata_core::{AlbumSortWriteAction, MetadataWritePlan};
pub use intent_plan::CoverArtPassthroughPolicy;
pub(crate) use intent_plan::{
    plan_metadata_outcome, plan_metadata_write, MetadataOutcomePlan, MetadataOutcomeRequest,
};

impl From<MetadataCoreError> for AppError {
    fn from(error: MetadataCoreError) -> Self {
        match error {
            MetadataCoreError::InvalidInput(message) => AppError::InvalidInput(message),
        }
    }
}

pub use reader::read_metadata;

pub use cover_art::{
    add_cover_art_stream_pre_header, detect_cover_art_format, write_cover_art_packet_post_header,
    CoverFormat,
};
pub use ffmpeg_dict::{set_container_metadata, validate_metadata_compatibility};
pub(crate) use passthrough::merge_passthrough_cover_art;
#[allow(unused_imports)]
pub use passthrough::ChapterSpec;
pub use passthrough::{
    add_chapters_to_output, extract_passthrough_metadata, PassthroughMetadata, PassthroughSource,
};
pub use remux::rewrite_metadata_with_ffmpeg;

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
