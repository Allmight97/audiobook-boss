use crate::errors::{AppError, Result};

pub mod reader;

mod container;
mod cue;
pub use abb_metadata_core::{
    parse_cue, validate_chapters, ChapterSpec, CueInterpretation, CueSheet,
};
use container::Container;
pub(crate) use cue::{
    inspect_chapter_source, source_fingerprint, source_fingerprint_from_metadata,
    validate_chapter_plan, validate_source_fingerprint,
};
pub use cue::{ChapterPlan, CueChoice, CueSource, CueStatus};
mod chapter_count;
mod cover_art;
mod embedded_cover;
mod ffi;
mod ffmpeg_dict;
mod field_schema;
mod intent_plan;
mod matroska_cover;
mod metadata_sinks;
mod mp4_covr;
mod mp4ameta_bridge;
mod passthrough;
mod plan_log;
mod remux;
mod thumbnail;

pub(crate) use abb_metadata_core::MetadataWritePlan;
pub use abb_metadata_core::{
    compute_album_sort, normalize_publication_date, processing_album_sort,
    publication_year_from_date, split_series_list, validate_metadata_intent_patch,
    AlbumSortPatchOp, AudiobookMetadata, MetadataCoreError, MetadataIntentPatch,
    MetadataIntentValidationField, MetadataIntentValidationResult, NamingMetadata, PatchOp,
};
pub(crate) use chapter_count::{join_chapters, JoinedSource};
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

pub use reader::{display_tags_from_ffmpeg_dict, read_metadata};
pub use thumbnail::read_audio_cover_thumbnail;
pub(crate) use thumbnail::render_display_thumbnail;
pub(crate) use thumbnail::{optimize_cover_art, prepare_cover_art_for_write};

pub(crate) use cover_art::ResolvedCover;
pub use cover_art::{add_cover_art_stream_pre_header, write_cover_art_packet_post_header};
pub(crate) use ffi::length_is_estimated;
pub use ffmpeg_dict::set_container_metadata;
pub(crate) use passthrough::prepare_output_cover_art;
pub use passthrough::{
    add_chapters_to_output, extract_passthrough_metadata, read_chapters, verify_chapters,
    PassthroughMetadata, PassthroughSource,
};

pub fn save_metadata_intent(path: &std::path::Path, patch: &MetadataIntentPatch) -> Result<()> {
    let plan = plan_metadata_write_for_path(path, patch)?;
    save_metadata_with_plan(path, &plan)
}

/// Saves into a sibling copy (a clone of the source for MP4, a remux for
/// others), checks the copy's audio against the original, then swaps it in.
/// The original is never opened for writing.
pub(crate) fn save_metadata_with_plan(
    path: &std::path::Path,
    plan: &MetadataWritePlan,
) -> Result<()> {
    let route = crate::diagnostics::stage("metadata_classify", path, || container::classify(path))?;
    let saved = remux::meta_temp(path)?;
    match route {
        Container::Mp4 => {
            crate::diagnostics::stage("metadata_copy_source", path, || {
                std::fs::copy(path, saved.path()).map_err(AppError::Io)
            })?;
            mp4ameta_bridge::write_metadata_with_plan(saved.path(), plan)?;
        }
        route => remux::remux_into(path, saved.path(), Some(plan), None, Some(&route))?,
    }
    commit_checked_save(path, saved)
}

fn commit_checked_save(
    original: &std::path::Path,
    saved: crate::file_replace::SiblingTemp,
) -> Result<()> {
    check_saved_audio(original, saved.path())?;
    crate::diagnostics::stage("metadata_replace", original, || {
        saved.commit(original).map_err(AppError::Io)
    })
}

/// Refuses a saved copy whose audio stream count differs from the original's
/// or whose duration is more than 0.5 s away. A loss under 0.5 s passes.
/// Durations count only when FFmpeg measured both or estimated both from a
/// bitrate: a joined MP3 reads as an estimate while its remuxed copy is
/// measured. Without comparable durations (that case, raw ADTS) only the
/// stream count is checked.
fn check_saved_audio(original: &std::path::Path, saved: &std::path::Path) -> Result<()> {
    const TOLERANCE_US: u64 = 500_000;
    let (original_us, original_estimated, original_streams) = audio_shape(original)?;
    let (saved_us, saved_estimated, saved_streams) = audio_shape(saved)?;
    let unknown = ffmpeg_next::ffi::AV_NOPTS_VALUE;
    let duration_moved = original_us != unknown
        && original_estimated == saved_estimated
        && (saved_us == unknown || original_us.abs_diff(saved_us) > TOLERANCE_US);
    if original_streams == saved_streams && !duration_moved {
        return Ok(());
    }
    let seconds = |us: i64| {
        if us == unknown {
            "unknown".to_string()
        } else {
            format!("{:.1} s", us as f64 / 1_000_000.0)
        }
    };
    Err(AppError::FileValidation(format!(
        "The original '{}' was kept unchanged: the saved copy's audio did not match it (length {} vs {}, audio streams {} vs {}).",
        crate::errors::sanitize_path_for_display(original),
        seconds(saved_us),
        seconds(original_us),
        saved_streams,
        original_streams
    )))
}

fn audio_shape(path: &std::path::Path) -> Result<(i64, bool, usize)> {
    let input = ffmpeg_next::format::input(path).map_err(AppError::Ffmpeg)?;
    let audio_streams = input
        .streams()
        .filter(|stream| stream.parameters().medium() == ffmpeg_next::media::Type::Audio)
        .count();
    Ok((input.duration(), length_is_estimated(&input), audio_streams))
}

/// Remuxes a freshly produced artifact into its container with the accepted
/// chapters and cover art, then finishes its tags (see `finish_artifact_tags`).
/// Used by preserved merges.
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
        if route.is_mp4_family() {
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

/// Puts an owned staging artifact into the target container with the accepted
/// chapters, keeping its audio, pictures, and tags. A file already in that
/// container with those chapters (for MP4, those titles and starts) is left
/// alone. Callers never pass an original source here.
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
    // A same-container remux copies every picture the file holds; a cover
    // would replace them. A container change carries the first one over.
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
    remux::rewrite_metadata_with_ffmpeg_plan_as(
        path,
        Some(&plan),
        Some(&passthrough),
        Some(&target),
    )?;
    finish_artifact_tags(path, Some(&metadata), None, || {})?;
    Ok(())
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
