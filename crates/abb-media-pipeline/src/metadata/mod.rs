use crate::errors::{AppError, Result};

pub mod reader;

mod container;
mod cue;
pub use abb_metadata_core::{
    parse_cue, validate_chapters, ChapterSpec, CueInterpretation, CueSheet,
};
use container::Container;
pub(crate) use cue::{inspect_chapter_source, validate_chapter_plan, validate_source_fingerprint};
pub use cue::{source_fingerprint, source_fingerprint_from_metadata};
pub use cue::{ChapterPlan, CueChoice, CueSource, CueStatus};
mod chapter_count;
mod cover_art;
mod cover_choice;
mod embedded_cover;
mod ffmpeg_dict;
mod field_schema;
mod intent_plan;
mod journal;
mod matroska_cover;
mod metadata_sinks;
pub(crate) mod mp4;
mod passthrough;
mod plan_log;
mod remux;
mod tag_support;
mod thumbnail;
mod verify;
mod write_strategy;

pub(crate) use abb_metadata_core::MetadataWritePlan;
pub use abb_metadata_core::{
    compute_album_sort, normalize_publication_date, processing_album_sort,
    publication_year_from_date, split_series_list, validate_metadata_intent_patch,
    AlbumSortPatchOp, AudiobookMetadata, MetadataCoreError, MetadataIntentPatch,
    MetadataIntentValidationField, MetadataIntentValidationResult, NamingMetadata, PatchOp,
};
pub use chapter_count::{join_chapters, JoinedSource};
pub use intent_plan::CoverArtPassthroughPolicy;
pub(crate) use intent_plan::{
    plan_metadata_outcome, plan_metadata_outcome_from, plan_metadata_write_for_path,
    MetadataOutcomePlan,
};

impl From<MetadataCoreError> for AppError {
    fn from(error: MetadataCoreError) -> Self {
        match error {
            MetadataCoreError::InvalidInput(message) => AppError::InvalidInput(message),
        }
    }
}

pub(crate) use reader::display_tags;
pub use reader::read_metadata;
pub use thumbnail::optimize_cover_art;
pub(crate) use thumbnail::prepare_cover_art_for_write;
pub use thumbnail::read_audio_cover_thumbnail;
pub use thumbnail::render_display_thumbnail;

pub(crate) use cover_art::{add_cover_stream, write_cover_packet, ResolvedCover};
pub(crate) use ffmpeg_dict::set_container_metadata;
pub(crate) use passthrough::{add_chapters, prepare_output_cover_art, read_chapters};
pub use passthrough::{chapter_specs, titled_chapter};
pub use passthrough::{
    extract_passthrough_metadata, verify_chapters, PassthroughMetadata, PassthroughSource,
};

pub(crate) fn open_input(path: &std::path::Path) -> Result<abb_ffmpeg::Input> {
    Ok(abb_ffmpeg::Input::open(path, &abb_ffmpeg::SecretOptions::default())?.input)
}

pub(crate) fn recover_journal_if_present(path: &std::path::Path) -> Result<()> {
    journal::recover_if_present(path).map(|_| ())
}

pub fn save_metadata_intent(path: &std::path::Path, patch: &MetadataIntentPatch) -> Result<()> {
    let plan = plan_metadata_write_for_path(path, patch)?;
    save_metadata_with_plan(path, &plan)
}

pub(crate) fn save_metadata_with_plan(
    path: &std::path::Path,
    plan: &MetadataWritePlan,
) -> Result<()> {
    save_metadata_with_plan_before_commit(path, plan, || {})
}

#[cfg(test)]
use verify::commit_checked_save;

fn save_metadata_with_plan_before_commit(
    path: &std::path::Path,
    plan: &MetadataWritePlan,
    before_commit: impl FnOnce(),
) -> Result<()> {
    write_strategy::save_metadata_with_strategy(path, plan, before_commit)
}

pub(crate) fn finalize_artifact_metadata(
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
        if route.is_mp4_family() {
            before_tag_write();
            let started = std::time::Instant::now();
            mp4::write_metadata(path, metadata)?;
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

pub(crate) fn remux_preserved_audio_container(
    path: &std::path::Path,
    extension: &str,
    chapters: &[ChapterSpec],
) -> Result<()> {
    let target = Container::from_extension(extension).ok_or_else(|| {
        crate::errors::AppError::InvalidInput("Unsupported audiobook container.".into())
    })?;
    let source = extract_passthrough_metadata(&[PassthroughSource {
        path: path.into(),
        duration: None,
        is_valid: true,
        chapters: None,
    }]);
    let same_container = container::classify(path)? == target;
    if same_container
        && passthrough::first_chapter_difference(
            &source.chapters,
            chapters,
            target.stores_chapter_ends(),
        )
        .is_none()
    {
        return Ok(());
    }
    let mut metadata = read_metadata(path)?;
    let cover_art = if same_container {
        metadata.cover_art = None;
        None
    } else {
        source.cover_art
    };
    let passthrough = PassthroughMetadata {
        chapters: chapters.to_vec(),
        cover_art,
        ..Default::default()
    };
    let plan = MetadataWritePlan::from_metadata(&metadata);
    remux::rewrite_metadata_with_ffmpeg_plan_as(path, Some(&plan), Some(&passthrough), &target)?;
    finish_artifact_tags(path, Some(&metadata), None, || {})?;
    Ok(())
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
