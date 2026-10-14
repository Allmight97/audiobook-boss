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
pub(crate) use cue::{
    inspect_chapter_source, source_fingerprint, validate_chapter_plan, validate_source_fingerprint,
};
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
        container::ContainerRoute::Mp4Family => {
            crate::diagnostics::stage("metadata_copy_source", path, || {
                std::fs::copy(path, saved.path()).map_err(AppError::Io)
            })?;
            mp4ameta_bridge::write_metadata_with_plan(saved.path(), plan)?;
        }
        route => remux::remux_into(
            path,
            saved.path(),
            Some(plan),
            None,
            route.remux_output_format(),
        )?,
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
/// or whose duration is more than 0.5 s away. A loss under 0.5 s passes. An
/// original without a known duration (raw ADTS) is checked by stream count only.
fn check_saved_audio(original: &std::path::Path, saved: &std::path::Path) -> Result<()> {
    const TOLERANCE_US: u64 = 500_000;
    let (original_us, original_streams) = audio_shape(original)?;
    let (saved_us, saved_streams) = audio_shape(saved)?;
    let unknown = ffmpeg_next::ffi::AV_NOPTS_VALUE;
    let duration_moved = original_us != unknown
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

fn audio_shape(path: &std::path::Path) -> Result<(i64, usize)> {
    let input = ffmpeg_next::format::input(path).map_err(AppError::Ffmpeg)?;
    let audio_streams = input
        .streams()
        .filter(|stream| stream.parameters().medium() == ffmpeg_next::media::Type::Audio)
        .count();
    Ok((input.duration(), audio_streams))
}

/// Finalizes a freshly produced artifact's metadata in one container-aware
/// pass, as a preserved merge needs: a remux pass carries chapters and cover
/// art, then MP4-family tag truth is rewritten through the mp4ameta adapter
/// chosen by actual container classification. The FFmpeg mov muxer silently
/// drops dictionary keys outside its known-atom table (series, series-part,
/// sort_album), so MP4-family artifacts must not rely
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

/// Puts an owned staging artifact into the target container with the accepted
/// chapters, keeping its audio, cover, and tags. A file already in that
/// container with exactly those chapters is left alone. Callers never pass an
/// original source here.
pub(crate) fn remux_preserved_audio_container(
    path: &std::path::Path,
    extension: &str,
    chapters: &[ChapterSpec],
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
    let source = extract_passthrough_metadata(&[PassthroughSource {
        path: path.into(),
        duration: None,
        is_valid: true,
        chapters: None,
    }]);
    if container::classify(path)?.remux_output_format() == Some(target)
        && source.chapters == chapters
    {
        return Ok(());
    }
    let metadata = read_metadata(path)?;
    let passthrough = PassthroughMetadata {
        chapters: chapters.to_vec(),
        cover_art: source.cover_art,
    };
    let plan = MetadataWritePlan::from_metadata(metadata.clone());
    remux::rewrite_metadata_with_ffmpeg_plan_as(
        path,
        Some(&plan),
        Some(&passthrough),
        Some(target),
    )?;
    finish_artifact_tags(path, Some(&metadata), None, || {})?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_cases::integration_media_execution_tests::write_sine_wav;

    #[test]
    fn a_saved_copy_shorter_than_the_original_is_refused_and_the_original_kept() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let original = dir.path().join("book.wav");
        write_sine_wav(&original, 2.0, 440.0);
        let before = std::fs::read(&original).expect("read original");
        let saved = remux::meta_temp(&original).expect("temp");
        write_sine_wav(saved.path(), 1.0, 440.0);
        let saved_path = saved.path().to_path_buf();

        let error = commit_checked_save(&original, saved).expect_err("shorter copy refused");

        assert!(error.to_string().contains("unchanged"), "{error}");
        assert_eq!(std::fs::read(&original).expect("reread original"), before);
        assert!(!saved_path.exists(), "the refused copy is removed");
    }
}
