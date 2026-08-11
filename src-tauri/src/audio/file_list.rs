//! File list management and validation

use super::{AudioChapter, AudioFile, DecoderSelection};
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
    /// Stable decoder identities aligned by index with `files`.
    pub selected_decoders: Vec<Option<DecoderSelection>>,
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

struct ValidatedAudioFile {
    audio_file: AudioFile,
    selected_decoder: Option<DecoderSelection>,
}

/// Validates a single audio file
fn validate_single_file(path: &Path) -> Result<ValidatedAudioFile> {
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
            return Ok(ValidatedAudioFile {
                audio_file,
                selected_decoder: None,
            });
        }
    };

    // Get file size (canonical path should exist)
    match fs::metadata(&canonical_path) {
        Ok(metadata) => audio_file.size = Some(metadata.len() as f64),
        Err(e) => {
            audio_file.error = Some(format!("Cannot read file metadata: {e}"));
            return Ok(ValidatedAudioFile {
                audio_file,
                selected_decoder: None,
            });
        }
    }

    // Validate audio format and get comprehensive metadata using canonical path
    match validate_audio_format(&canonical_path) {
        Ok(properties) => {
            audio_file.format = Some(properties.format);
            audio_file.duration = Some(properties.duration);
            audio_file.bitrate = properties.bitrate;
            audio_file.sample_rate = properties.sample_rate;
            audio_file.channels = properties.channels;
            audio_file.codec_label = properties.codec_label;
            audio_file.tag_title = properties.tag_title;
            audio_file.tag_artist = properties.tag_artist;
            audio_file.chapters = properties.chapters;
            audio_file.selected_decoder = properties
                .selected_decoder
                .as_ref()
                .map(|selection| selection.decoder_label.clone());
            audio_file.is_valid = true;

            return Ok(ValidatedAudioFile {
                audio_file,
                selected_decoder: properties.selected_decoder,
            });
        }
        Err(e) => {
            audio_file.error = Some(e.to_string());
        }
    }

    Ok(ValidatedAudioFile {
        audio_file,
        selected_decoder: None,
    })
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
}

fn validate_audio_format(path: &Path) -> Result<AudioProperties> {
    ff::init().map_err(AppError::Ffmpeg)?;

    // First check if we support the file extension
    let format = crate::audio::extensions::audio_format_for_path(path)?.label;

    let (duration, chapters, (tag_title, tag_artist)) = {
        let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
        let audio_stream = ictx
            .streams()
            .best(ff::media::Type::Audio)
            .ok_or_else(|| AppError::InvalidInput("No audio stream found".to_string()))?;
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
        (duration, chapters, display_tags)
    };

    // Validate that we got a reasonable duration
    if duration <= 0.0 {
        return Err(AppError::InvalidInput(
            "Audio file has invalid duration (0 seconds)".to_string(),
        ));
    }

    // Extract technical metadata
    let inspection = crate::audio::processor::inspect_audio_decoder(path)?;
    let selected_decoder = inspection.selected_decoder;
    log::info!(
        "validate_audio_format path={} selected_decoder_id={} selected_decoder={}",
        crate::errors::sanitize_path_for_display(path),
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
    })
}

/// Gets comprehensive information about a file list
pub fn get_file_list_info<P: AsRef<Path>>(file_paths: &[P]) -> Result<FileListInfo> {
    let mut files = Vec::new();
    let mut selected_decoders = Vec::new();

    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "No files provided for validation".to_string(),
        ));
    }

    for path in file_paths {
        let validated = validate_single_file(path.as_ref())?;
        selected_decoders.push(validated.selected_decoder);
        files.push(validated.audio_file);
    }

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

    Ok(FileListInfo {
        files,
        selected_decoders,
        total_duration,
        total_size,
        valid_count,
        invalid_count,
    })
}
