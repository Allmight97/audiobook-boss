//! File list management and validation

use super::{AacDecoder, AudioChapter, AudioFile, DecoderSelection};
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::fs;
use std::path::Path;

/// Summary information for a file list
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileListInfo {
    /// List of validated audio files
    pub files: Vec<AudioFile>,
    /// Total duration in seconds
    #[specta(type = specta_typescript::Number)]
    pub total_duration: f64,
    /// Total size in bytes
    #[specta(type = specta_typescript::Number)]
    pub total_size: f64,
    /// Number of valid files
    pub valid_count: usize,
    /// Number of invalid files
    pub invalid_count: usize,
}

/// Validates a single audio file
fn validate_single_file(path: &Path, aac_decoder: AacDecoder) -> Result<AudioFile> {
    let mut audio_file = AudioFile::new(path.to_path_buf());

    // Use shared validation first
    let canonical_path = match crate::audio::path_validation::validate_input_audio_path(path) {
        Ok(canonical) => {
            // Update AudioFile to use canonical path
            audio_file.path = canonical.clone();
            canonical
        }
        Err(e) => {
            audio_file.error = Some(e.to_string());
            return Ok(audio_file);
        }
    };

    // Get file size (canonical path should exist)
    let file_size = match fs::metadata(&canonical_path) {
        Ok(metadata) => {
            audio_file.size = Some(metadata.len() as f64);
            metadata.len()
        }
        Err(e) => {
            audio_file.error = Some(format!("Cannot read file metadata: {e}"));
            return Ok(audio_file);
        }
    };

    // Validate audio format and get comprehensive metadata using canonical path
    match validate_audio_format(&canonical_path, file_size, aac_decoder) {
        Ok(properties) => {
            audio_file.format = Some(properties.format);
            audio_file.duration = Some(properties.duration);
            audio_file.bitrate = properties.bitrate;
            audio_file.sample_rate = properties.sample_rate;
            audio_file.channels = properties.channels;
            audio_file.codec_label = properties.codec_label;
            audio_file.preservation = Some(properties.preservation);
            audio_file.tag_title = properties.tag_title;
            audio_file.tag_artist = properties.tag_artist;
            audio_file.chapters = properties.chapters;
            let (plan, cue) = crate::metadata::inspect_chapter_source(
                &canonical_path,
                (properties.duration * 1000.0).round() as i64,
                &audio_file.chapters,
            )?;
            audio_file.chapter_plan = Some(plan);
            audio_file.cue_source = cue;
            audio_file.selected_decoder = properties
                .selected_decoder
                .as_ref()
                .map(|selection| selection.decoder_label.clone());
            audio_file.is_valid = true;

            return Ok(audio_file);
        }
        Err(e) => {
            audio_file.error = Some(e.to_string());
        }
    }

    Ok(audio_file)
}

/// Validates audio format using ffmpeg-next and returns comprehensive metadata
struct AudioProperties {
    format: String,
    duration: f64,
    bitrate: Option<u32>,
    sample_rate: Option<u32>,
    channels: Option<u32>,
    codec_label: Option<String>,
    tag_title: Option<String>,
    tag_artist: Option<String>,
    chapters: Vec<AudioChapter>,
    selected_decoder: Option<DecoderSelection>,
    preservation: super::AudioPreservation,
}

/// Known MP4 audio packets must fit inside the local file, even when the
/// demuxer would report ordinary EOF at a missing packet boundary.
fn validate_mp4_audio_extent(
    input: &ff::format::context::Input,
    audio_stream: &ff::Stream<'_>,
    file_size: u64,
) -> Result<()> {
    if !input.format().name().split(',').any(|name| name == "mp4") {
        return Ok(());
    }
    let incomplete = || {
        AppError::InvalidInput(
            "MP4 audio is incomplete or truncated: invalid indexed audio byte range.".into(),
        )
    };
    // SAFETY: The stream remains borrowed from the live input. These public
    // index APIs only inspect it; copy each entry's fields before another call.
    let stream = unsafe { audio_stream.as_ptr() };
    // SAFETY: `stream` is the valid pointer above, and the call only reads the stream's index.
    let count = unsafe { ff::ffi::avformat_index_get_entries_count(stream) };
    for index in 0..count {
        // SAFETY: `index` is below the entry count, and FFmpeg returns null for any entry it cannot give,
        // which `as_ref` turns into `None`. Position and size are copied out before the next FFmpeg call.
        let (position, size) = unsafe {
            ff::ffi::avformat_index_get_entry(stream.cast_mut(), index)
                .as_ref()
                .map(|entry| (entry.pos, entry.size()))
        }
        .ok_or_else(incomplete)?;
        let position = u64::try_from(position).map_err(|_| incomplete())?;
        let size = u64::try_from(size).map_err(|_| incomplete())?;
        let end = position.checked_add(size).ok_or_else(incomplete)?;
        if end > file_size {
            return Err(incomplete());
        }
    }
    Ok(())
}

fn validate_audio_format(
    path: &Path,
    file_size: u64,
    aac_decoder: AacDecoder,
) -> Result<AudioProperties> {
    ff::init().map_err(AppError::Ffmpeg)?;

    // First check if we support the file extension
    let format = crate::audio::extensions::audio_format_for_path(path)?.label;

    let (duration, chapters, (tag_title, tag_artist), container_name) = {
        let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
        let audio_stream = ictx
            .streams()
            .best(ff::media::Type::Audio)
            .ok_or_else(|| AppError::InvalidInput("No audio stream found".to_string()))?;
        validate_mp4_audio_extent(&ictx, &audio_stream, file_size)?;
        let container = ictx.duration();
        let duration = if container > 0 {
            container as f64 / ffmpeg_next::ffi::AV_TIME_BASE as f64
        } else {
            let stream_dur = audio_stream.duration();
            if stream_dur > 0 {
                let tb = audio_stream.time_base();
                stream_dur as f64 * (tb.0 as f64 / tb.1 as f64)
            } else {
                0.0
            }
        };
        let chapters = ictx
            .chapters()
            .map(|chapter| AudioChapter {
                title: chapter.metadata().get("title").map(str::to_string),
                start_ms: ff::Rescale::rescale(
                    &chapter.start(),
                    chapter.time_base(),
                    ff::Rational(1, 1_000),
                ),
                end_ms: ff::Rescale::rescale(
                    &chapter.end(),
                    chapter.time_base(),
                    ff::Rational(1, 1_000),
                ),
            })
            .collect();
        let display_tags = crate::metadata::display_tags_from_ffmpeg_dict(&ictx.metadata());
        (
            duration,
            chapters,
            display_tags,
            ictx.format().name().to_string(),
        )
    };

    // Validate that we got a reasonable duration
    if duration <= 0.0 {
        return Err(AppError::InvalidInput(
            "Audio file has invalid duration (0 seconds)".to_string(),
        ));
    }

    // Extract technical metadata
    let inspection = crate::audio::processor::inspect_audio_decoder(path, aac_decoder)?;
    let selected_decoder = inspection.selected_decoder;
    log::debug!(
        "validate_audio_format source_path={:?} selected_decoder_id={} selected_decoder={}",
        path,
        selected_decoder.decoder_id.as_str(),
        selected_decoder.decoder_label.as_str()
    );

    let sample_rate = Some(inspection.sample_rate);
    let channels = Some(inspection.channels);

    Ok(AudioProperties {
        format: format.to_string(),
        duration,
        bitrate: inspection.bitrate,
        sample_rate,
        channels,
        codec_label: inspection.codec_label,
        tag_title,
        tag_artist,
        chapters,
        selected_decoder: Some(selected_decoder),
        preservation: crate::audio::processor::assess_preservation(
            &path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
            &container_name,
            inspection.codec_id,
        ),
    })
}

/// Gets comprehensive information about a file list
pub fn get_file_list_info<P: AsRef<Path>>(
    file_paths: &[P],
    aac_decoder: AacDecoder,
) -> Result<FileListInfo> {
    let mut files = Vec::new();

    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "No files provided for validation".to_string(),
        ));
    }

    for path in file_paths {
        files.push(validate_single_file(path.as_ref(), aac_decoder)?);
    }
    Ok(FileListInfo::from_files(files))
}

impl FileListInfo {
    /// Totals for files that were already inspected.
    pub fn from_files(files: Vec<AudioFile>) -> Self {
        let mut total_duration = 0.0;
        let mut total_size = 0.0;
        let mut valid_count = 0;
        let mut invalid_count = 0;
        for file in &files {
            if file.is_valid {
                total_duration += file
                    .duration
                    .filter(|value| value.is_finite())
                    .unwrap_or(0.0);
                total_size += file.size.filter(|value| value.is_finite()).unwrap_or(0.0);
                valid_count += 1;
            } else {
                invalid_count += 1;
            }
        }
        Self {
            files,
            total_duration,
            total_size,
            valid_count,
            invalid_count,
        }
    }
}

/// Bind accepted chapter facts to freshly inspected audio, never to a reread CUE.
pub fn apply_chapter_plans(
    files: &mut FileListInfo,
    plans: Option<&std::collections::HashMap<String, crate::metadata::ChapterPlan>>,
) -> Result<()> {
    for file in files.files.iter_mut().filter(|file| file.is_valid) {
        let accepted = plans.and_then(|plans| file.path.to_str().and_then(|path| plans.get(path)));
        if let Some(plan) = accepted {
            crate::metadata::validate_chapter_plan(
                &file.path,
                (file.duration.unwrap_or(0.0) * 1000.0).round() as i64,
                plan,
            )?;
            file.chapter_plan = Some(plan.clone());
            file.cue_source = None; // accepted intent supersedes inspection/confirmation status
        } else if file.cue_source.is_some() {
            return Err(AppError::InvalidInput(
                "Review the sibling CUE in Input before processing, or explicitly ignore it."
                    .into(),
            ));
        }
    }
    Ok(())
}
