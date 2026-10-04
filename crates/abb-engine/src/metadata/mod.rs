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
mod cue;
pub use abb_metadata_core::{
    parse_cue, validate_chapters, ChapterSpec, CueInterpretation, CueSheet,
};
pub(crate) use cue::{inspect_chapter_source, validate_chapter_plan, validate_source_fingerprint};
pub use cue::{ChapterPlan, CueSource, CueStatus};
mod cover_art;
mod embedded_cover;
mod ffi;
mod ffmpeg_dict;
mod field_schema;
mod intent_plan;
mod matroska_cover;
mod metadata_ops;
mod metadata_sinks;
mod mp4_covr;
mod mp4ameta_bridge;
mod passthrough;
mod remux;
mod thumbnail;

pub use abb_metadata_core::{
    build_series_list, compute_album_sort, normalize_publication_date, processing_album_sort,
    publication_year_from_date, split_series_list, validate_metadata_intent_patch,
    AlbumSortPatchOp, AudiobookMetadata, MetadataCoreError, MetadataIntentPatch,
    MetadataIntentValidationField, MetadataIntentValidationResult, NamingMetadata, PatchOp,
};
pub(crate) use abb_metadata_core::{AlbumSortWriteAction, MetadataWritePlan};
pub use intent_plan::CoverArtPassthroughPolicy;
pub(crate) use intent_plan::{
    plan_metadata_outcome, plan_metadata_outcome_from, plan_metadata_write_for_path,
    MetadataOutcomePlan, MetadataOutcomeRequest,
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
pub(crate) use thumbnail::render_display_thumbnail;
pub(crate) use thumbnail::{optimize_cover_art, prepare_cover_art_for_write};

pub use cover_art::{add_cover_art_stream_pre_header, write_cover_art_packet_post_header};
pub use ffmpeg_dict::set_container_metadata;
pub(crate) use passthrough::prepare_output_cover_art;
pub use passthrough::{
    add_chapters_to_output, extract_passthrough_metadata, verify_chapters, PassthroughMetadata,
    PassthroughSource,
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
    match crate::diagnostics::stage("metadata_classify", path, || container::classify(path))? {
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
/// pass, as a preserved merge needs: a remux pass carries chapters and cover
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

    crate::diagnostics::stage("metadata_remux", path, || {
        remux::rewrite_metadata_with_ffmpeg(path, metadata, passthrough)
    })?;
    finish_artifact_tags(
        path,
        metadata,
        passthrough.map(|value| value.chapters.as_slice()),
        || {},
    )?;
    Ok(())
}

/// Last metadata step on a produced artifact, after muxing or remuxing:
/// MP4-family tag truth is rewritten through mp4ameta (the mov muxer drops
/// series, freeform, and sort keys), then accepted chapters are verified.
/// `before_tag_write` runs only when that rewrite happens, so callers can
/// report progress without knowing the container strategy. Returns whether
/// tags were rewritten.
pub(crate) fn finish_artifact_tags(
    path: &std::path::Path,
    metadata: Option<&AudiobookMetadata>,
    chapters: Option<&[ChapterSpec]>,
    before_tag_write: impl FnOnce(),
) -> Result<bool> {
    let mut rewrote = false;
    if let Some(metadata) = metadata {
        let route =
            crate::diagnostics::stage("metadata_classify", path, || container::classify(path))?;
        if route == container::ContainerRoute::Mp4Family {
            before_tag_write();
            let started = std::time::Instant::now();
            mp4ameta_bridge::write_metadata(path, metadata)?;
            log::info!(
                "artifact_tag_write status=ok elapsed_ms={} artifact={}",
                started.elapsed().as_millis(),
                crate::diagnostics::artifact_id(path)
            );
            rewrote = true;
        }
    }
    if let Some(chapters) = chapters {
        verify_chapters(path, chapters)?;
    }
    Ok(rewrote)
}

/// Changes only the container on an owned staging artifact, retaining its audio,
/// accepted chapters, and metadata. Callers never pass an original source here.
pub(crate) fn remux_preserved_audio_container(
    path: &std::path::Path,
    extension: &str,
) -> Result<()> {
    let target = match extension {
        "m4a" | "m4b" => "mp4",
        "mka" => "matroska",
        "mp3" => "mp3",
        _ => {
            return Err(crate::errors::AppError::InvalidInput(
                "Unsupported audiobook container.".into(),
            ))
        }
    };
    if container::classify(path)?.remux_output_format() == Some(target) {
        return Ok(());
    }
    let metadata = read_metadata(path)?;
    let passthrough = extract_passthrough_metadata(&[PassthroughSource {
        path: path.into(),
        duration: None,
        is_valid: true,
        chapters: None,
    }])
    .into_option();
    let plan = MetadataWritePlan::from_metadata(metadata.clone());
    remux::rewrite_metadata_with_ffmpeg_plan_as(
        path,
        Some(&plan),
        passthrough.as_ref(),
        Some(target),
    )?;
    finish_artifact_tags(
        path,
        Some(&metadata),
        passthrough.as_ref().map(|value| value.chapters.as_slice()),
        || {},
    )?;
    Ok(())
}
