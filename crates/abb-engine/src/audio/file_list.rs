use super::{AacDecoder, AudioChapter, AudioFile, DecoderSelection};
use crate::errors::{AppError, Result};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileListInfo {
    pub files: Vec<AudioFile>,
    /// Total duration in seconds
    #[specta(type = specta_typescript::Number)]
    pub total_duration: f64,
    /// Total size in bytes
    #[specta(type = specta_typescript::Number)]
    pub total_size: f64,
    pub valid_count: usize,
    pub invalid_count: usize,
}

fn validate_single_file(path: &Path, aac_decoder: AacDecoder) -> Result<AudioFile> {
    let mut audio_file = AudioFile::new(path.to_path_buf());

    let canonical_path = match crate::audio::path_validation::validate_input_audio_path(path) {
        Ok(canonical) => {
            audio_file.set_path(canonical.clone());
            canonical
        }
        Err(e) => {
            audio_file.error = Some(e.to_string());
            return Ok(audio_file);
        }
    };

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

    match validate_audio_format(&canonical_path, file_size, aac_decoder) {
        Ok(properties) => {
            audio_file.format = Some(properties.format);
            audio_file.duration = Some(properties.duration);
            audio_file.bitrate = properties.bitrate;
            audio_file.sample_rate = properties.sample_rate;
            audio_file.channels = properties.channels;
            audio_file.codec_label = properties.codec_label;
            audio_file.preservation = Some(properties.preservation);
            audio_file.set_tag_title(properties.tag_title);
            audio_file.set_artist(properties.tag_artist);
            audio_file.chapters = properties.chapters;
            let (plan, cue) = match crate::metadata::inspect_chapter_source(
                &canonical_path,
                (properties.duration * 1000.0).round() as i64,
                properties.length_estimated,
                &mut audio_file.chapters,
            ) {
                Ok(inspected) => inspected,
                Err(error) => {
                    audio_file.error = Some(error.to_string());
                    return Ok(audio_file);
                }
            };
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

struct AudioProperties {
    format: String,
    duration: f64,
    length_estimated: bool,
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
    input: &abb_ffmpeg::Input,
    audio_stream: usize,
    file_size: u64,
) -> Result<()> {
    if !input.format_name().split(',').any(|name| name == "mp4") {
        return Ok(());
    }
    let incomplete = || {
        AppError::InvalidInput(
            "MP4 audio is incomplete or truncated: invalid indexed audio byte range.".into(),
        )
    };
    for (position, size) in input
        .indexed_byte_ranges(audio_stream)
        .ok_or_else(incomplete)?
    {
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
    let format = crate::audio::extensions::audio_format_for_path(path)?.label;

    let (duration, length_estimated, chapters, (tag_title, tag_artist), container_name) = {
        let ictx = crate::metadata::open_input(path)?;
        let audio_stream = ictx
            .best_audio_stream()
            .and_then(|index| ictx.streams().into_iter().nth(index))
            .ok_or_else(|| AppError::InvalidInput("No audio stream found".to_string()))?;
        validate_mp4_audio_extent(&ictx, audio_stream.index, file_size)?;
        let duration = match ictx.duration_micros() {
            Some(container) if container > 0 => container as f64 / 1_000_000.0,
            _ => match audio_stream.duration {
                Some(stream_dur) if stream_dur > 0 => {
                    let tb = audio_stream.time_base;
                    stream_dur as f64 * (tb.numerator as f64 / tb.denominator as f64)
                }
                _ => 0.0,
            },
        };
        let chapters = crate::metadata::read_chapters(&ictx);
        let display_tags = crate::metadata::display_tags(&ictx.tags());
        let container_name = ictx.format_name();
        let length_estimated = ictx.duration_is_estimated();
        (
            duration,
            length_estimated,
            chapters,
            display_tags,
            container_name,
        )
    };

    let measured = if length_estimated {
        crate::audio::processor::measured_length_seconds(path, aac_decoder)?
    } else {
        None
    };
    let (duration, length_estimated) =
        measured.map_or((duration, length_estimated), |seconds| (seconds, false));

    if duration <= 0.0 {
        return Err(AppError::InvalidInput(
            "Audio file has invalid duration (0 seconds)".to_string(),
        ));
    }

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
        length_estimated,
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

fn cue_review_error(cue: &crate::metadata::CueSource) -> AppError {
    AppError::InvalidInput(format!(
        "Review {}: confirm its timestamp interpretation or ignore the CUE before converting.",
        cue.file_name
    ))
}

impl FileListInfo {
    /// The rule for one title's sources: no CUE may wait for review, and CUE
    /// chapters cannot ride on a merged title.
    pub(crate) fn check_cue_chapters(files: &[AudioFile]) -> Result<()> {
        let unreviewed = files
            .iter()
            .filter(|file| file.is_valid)
            .filter_map(|file| file.cue_source.as_ref())
            .find(|cue| {
                matches!(
                    cue.status,
                    crate::metadata::CueStatus::NeedsConfirmation
                        | crate::metadata::CueStatus::Invalid
                )
            });
        if let Some(cue) = unreviewed {
            return Err(cue_review_error(cue));
        }
        if files.len() > 1
            && files
                .iter()
                .any(|file| file.chapter_plan.as_ref().is_some_and(|plan| plan.from_cue))
        {
            return Err(AppError::InvalidInput(
                "Merging CUE-bearing inputs is not supported. Separate the titles or ignore CUE chapters."
                    .into(),
            ));
        }
        Ok(())
    }

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
            file.cue_source = None;
        } else if let Some(cue) = &file.cue_source {
            return Err(cue_review_error(cue));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "file_list_tests.rs"]
mod tests;
