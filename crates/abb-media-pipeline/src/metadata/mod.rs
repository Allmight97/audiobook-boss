use crate::errors::{AppError, Result};

pub mod reader;

mod container;
mod cue;
pub use abb_metadata_core::{
    parse_cue, validate_chapters, ChapterSpec, CueInterpretation, CueSheet,
};
use container::Container;
pub use cue::{
    inspect_chapter_source, source_fingerprint, source_fingerprint_from_metadata,
    validate_chapter_plan, validate_source_fingerprint,
};
pub use cue::{ChapterPlan, CueChoice, CueSource, CueStatus};
mod chapter_count;
mod cover_art;
mod cover_choice;
mod embedded_cover;
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
mod tag_support;
mod thumbnail;

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
pub use thumbnail::read_audio_cover_thumbnail;
pub use thumbnail::render_display_thumbnail;
pub use thumbnail::{optimize_cover_art, prepare_cover_art_for_write};

pub use cover_art::{add_cover_stream, write_cover_packet, ResolvedCover};
pub use ffmpeg_dict::set_container_metadata;
pub use passthrough::{
    add_chapters, chapter_specs, prepare_output_cover_art, read_chapters, titled_chapter,
};
pub use passthrough::{
    extract_passthrough_metadata, verify_chapters, PassthroughMetadata, PassthroughSource,
};

pub(crate) fn open_input(path: &std::path::Path) -> Result<abb_ffmpeg::Input> {
    Ok(abb_ffmpeg::Input::open(path, &abb_ffmpeg::SecretOptions::default())?.input)
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

fn save_metadata_with_plan_before_commit(
    path: &std::path::Path,
    plan: &MetadataWritePlan,
    before_commit: impl FnOnce(),
) -> Result<()> {
    let route = crate::diagnostics::stage("metadata_classify", path, || container::classify(path))?;
    tag_support::refuse_unholdable_edits(&route, plan)?;
    let expected_fingerprint = source_fingerprint(path)?;
    let saved = remux::meta_temp(path)?;
    match route {
        Container::Mp4 => {
            crate::diagnostics::stage("metadata_copy_source", path, || {
                std::fs::copy(path, saved.path()).map_err(AppError::Io)
            })?;
            mp4ameta_bridge::write_metadata_with_plan(saved.path(), plan)?;
        }
        route => remux::remux_into(path, saved.path(), Some(plan), None, &route)?,
    }
    before_commit();
    commit_checked_save(path, saved, &expected_fingerprint)
}

fn commit_checked_save(
    original: &std::path::Path,
    saved: crate::file_replace::SiblingTemp,
    expected_fingerprint: &str,
) -> Result<()> {
    check_saved_audio(original, saved.path())?;
    let stat = std::fs::metadata(original).map_err(AppError::Io)?;
    validate_source_fingerprint(&stat, expected_fingerprint)?;
    crate::diagnostics::stage("metadata_replace", original, || {
        saved.commit(original).map_err(AppError::Io)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AudioStreamInventory {
    packets: u64,
    payload_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioDuration {
    Measured(i64),
    Estimated(i64),
    Unknown,
}

impl AudioDuration {
    fn from_input(input: &abb_ffmpeg::Input) -> Self {
        match (input.duration_micros(), input.duration_is_estimated()) {
            (Some(us), false) => Self::Measured(us),
            (Some(us), true) => Self::Estimated(us),
            (None, _) => Self::Unknown,
        }
    }

    fn moved_beyond(self, saved: Self, tolerance_us: u64) -> bool {
        matches!(
            (self, saved),
            (Self::Measured(original_us), Self::Measured(saved_us))
                if original_us.abs_diff(saved_us) > tolerance_us
        )
    }

    fn display(self) -> String {
        match self {
            Self::Measured(us) | Self::Estimated(us) => format!("{:.1} s", us as f64 / 1_000_000.0),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

struct AudioShape {
    duration: AudioDuration,
    audio_streams: Vec<AudioStreamInventory>,
}

fn check_saved_audio(original: &std::path::Path, saved: &std::path::Path) -> Result<()> {
    const TOLERANCE_US: u64 = 500_000;
    let original_shape = audio_shape(original)?;
    let saved_shape = audio_shape(saved)?;
    let duration_moved = original_shape
        .duration
        .moved_beyond(saved_shape.duration, TOLERANCE_US);
    if original_shape.audio_streams == saved_shape.audio_streams && !duration_moved {
        return Ok(());
    }
    let totals = |streams: &[AudioStreamInventory]| {
        streams.iter().fold((0u64, 0u64), |(packets, bytes), s| {
            (packets + s.packets, bytes + s.payload_bytes)
        })
    };
    let (orig_packets, orig_bytes) = totals(&original_shape.audio_streams);
    let (saved_packets, saved_bytes) = totals(&saved_shape.audio_streams);
    Err(AppError::FileValidation(format!(
        "The original '{}' was kept unchanged: the saved copy's audio did not match it (length {} vs {}, audio streams {} vs {}, packets {saved_packets} vs {orig_packets}, payload bytes {saved_bytes} vs {orig_bytes}).",
        crate::errors::sanitize_path_for_display(original),
        saved_shape.duration.display(),
        original_shape.duration.display(),
        saved_shape.audio_streams.len(),
        original_shape.audio_streams.len(),
    )))
}

fn audio_shape(path: &std::path::Path) -> Result<AudioShape> {
    let mut input = open_input(path)?;
    let streams = input.streams();
    let mut stream_to_audio = vec![None; streams.len()];
    let mut audio_streams = Vec::new();
    for stream in &streams {
        if stream.medium == abb_ffmpeg::Medium::Audio {
            let ordinal = audio_streams.len();
            stream_to_audio[stream.index] = Some(ordinal);
            audio_streams.push(AudioStreamInventory {
                packets: 0,
                payload_bytes: 0,
            });
        }
    }
    let duration = AudioDuration::from_input(&input);
    let mut packet = abb_ffmpeg::Packet::new()?;
    while input.read_packet(&mut packet)? {
        if let Some(ordinal) = stream_to_audio
            .get(packet.stream_index())
            .copied()
            .flatten()
        {
            audio_streams[ordinal].packets += 1;
            audio_streams[ordinal].payload_bytes += packet.data().len() as u64;
        }
    }
    Ok(AudioShape {
        duration,
        audio_streams,
    })
}

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
