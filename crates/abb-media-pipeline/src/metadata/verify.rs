use super::mp4::boxes::scan_top_level;
use super::{open_input, validate_source_fingerprint};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::file_replace::SiblingTemp;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::path::Path;

pub(crate) fn verify_surgical_mp4(path: &Path, expected_mdat_ranges: &[(u64, u64)]) -> Result<u64> {
    let mut file = File::open(path).map_err(AppError::Io)?;
    let file_len = file.seek(SeekFrom::End(0)).map_err(AppError::Io)?;
    let boxes = scan_top_level(&mut file, file_len)?;
    let actual_mdat: Vec<(u64, u64)> = boxes
        .iter()
        .filter(|item| &item.fourcc == b"mdat")
        .map(|item| (item.content_offset(), item.end()))
        .collect();
    if actual_mdat != expected_mdat_ranges {
        return Err(AppError::FileValidation(format!(
            "MP4 audio data ranges changed unexpectedly for '{}' ({actual_mdat:?} vs {expected_mdat_ranges:?})",
            sanitize_path_for_display(path)
        )));
    }
    let header_and_moov_bytes: u64 = boxes
        .iter()
        .map(|item| {
            if &item.fourcc == b"moov" {
                item.size
            } else {
                item.header_len
            }
        })
        .sum();
    drop(file);
    let input = open_input(path)?;
    if input.streams().is_empty() {
        return Err(AppError::FileValidation(format!(
            "Saved MP4 '{}' contains no playable streams",
            sanitize_path_for_display(path)
        )));
    }
    Ok(header_and_moov_bytes)
}

pub(crate) fn verify_streaming_mp4(
    path: &Path,
    source_mdat_sha256: [u8; 32],
    output_mdat_sha256: [u8; 32],
) -> Result<u64> {
    if source_mdat_sha256 != output_mdat_sha256 {
        return Err(AppError::FileValidation(format!(
            "Streamed MP4 '{}' audio payload digest mismatch",
            sanitize_path_for_display(path)
        )));
    }
    let mut file = File::open(path).map_err(AppError::Io)?;
    let file_len = file.seek(SeekFrom::End(0)).map_err(AppError::Io)?;
    let boxes = scan_top_level(&mut file, file_len)?;
    drop(file);
    let input = open_input(path)?;
    if input.streams().is_empty() {
        return Err(AppError::FileValidation(format!(
            "Streamed MP4 '{}' contains no playable streams",
            sanitize_path_for_display(path)
        )));
    }
    let bytes_read: u64 = boxes
        .iter()
        .map(|item| {
            if &item.fourcc == b"moov" {
                item.size
            } else {
                item.header_len
            }
        })
        .sum();
    Ok(bytes_read)
}

pub(super) fn commit_checked_save(
    original: &Path,
    saved: SiblingTemp,
    expected_fingerprint: &str,
) -> Result<()> {
    crate::diagnostics::stage_with("metadata_verify_audio", original, || {
        let bytes_read = check_saved_audio(original, saved.path())?;
        Ok(((), format!("mode=packets bytes_read={bytes_read}")))
    })?;
    let stat = std::fs::metadata(original).map_err(AppError::Io)?;
    validate_source_fingerprint(&stat, expected_fingerprint)?;
    crate::diagnostics::stage_with("metadata_replace", original, || {
        let timings = saved.commit_timed(original).map_err(AppError::Io)?;
        Ok(((), timings.log_fields()))
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
            Self::Measured(us) | Self::Estimated(us) => {
                format!("{:.1} s", us as f64 / 1_000_000.0)
            }
            Self::Unknown => "unknown".to_string(),
        }
    }
}

struct AudioShape {
    duration: AudioDuration,
    audio_streams: Vec<AudioStreamInventory>,
}

pub(super) fn check_saved_audio(original: &Path, saved: &Path) -> Result<u64> {
    const TOLERANCE_US: u64 = 500_000;
    let original_shape = audio_shape(original)?;
    let saved_shape = audio_shape(saved)?;
    let duration_moved = original_shape
        .duration
        .moved_beyond(saved_shape.duration, TOLERANCE_US);
    let totals = |streams: &[AudioStreamInventory]| {
        streams.iter().fold((0u64, 0u64), |(packets, bytes), s| {
            (packets + s.packets, bytes + s.payload_bytes)
        })
    };
    let (orig_packets, orig_bytes) = totals(&original_shape.audio_streams);
    let (saved_packets, saved_bytes) = totals(&saved_shape.audio_streams);
    if original_shape.audio_streams == saved_shape.audio_streams && !duration_moved {
        return Ok(orig_bytes.saturating_add(saved_bytes));
    }
    Err(AppError::FileValidation(format!(
        "The original '{}' was kept unchanged: the saved copy's audio did not match it (length {} vs {}, audio streams {} vs {}, packets {saved_packets} vs {orig_packets}, payload bytes {saved_bytes} vs {orig_bytes}).",
        sanitize_path_for_display(original),
        saved_shape.duration.display(),
        original_shape.duration.display(),
        saved_shape.audio_streams.len(),
        original_shape.audio_streams.len(),
    )))
}

fn audio_shape(path: &Path) -> Result<AudioShape> {
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

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
