use super::mp4::boxes::{scan_top_level_strict, BoxHeader};
use super::{open_input, validate_source_fingerprint};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::file_replace::SiblingTemp;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const VERIFY_CHUNK_BYTES: usize = 1024 * 1024;

pub(crate) fn verify_surgical_mp4(path: &Path, expected_mdat_ranges: &[(u64, u64)]) -> Result<u64> {
    let mut file = File::open(path).map_err(AppError::Io)?;
    let file_len = file.seek(SeekFrom::End(0)).map_err(AppError::Io)?;
    let boxes = scan_top_level_strict(&mut file, file_len)?;
    let actual_mdat: Vec<(u64, u64)> = boxes
        .iter()
        .filter(|item| &item.fourcc == b"mdat")
        .map(|item| (item.content_offset(), item.end()))
        .collect();
    if expected_mdat_ranges.is_empty() || actual_mdat.is_empty() {
        return Err(AppError::FileValidation(format!(
            "MP4 '{}' contains no mdat audio data boxes",
            sanitize_path_for_display(path)
        )));
    }
    if actual_mdat != expected_mdat_ranges {
        return Err(AppError::FileValidation(format!(
            "MP4 audio data ranges changed unexpectedly for '{}' ({actual_mdat:?} vs {expected_mdat_ranges:?})",
            sanitize_path_for_display(path)
        )));
    }
    let bytes_read = header_and_moov_bytes(&boxes);
    drop(file);
    let input = open_input(path)?;
    if input.streams().is_empty() {
        return Err(AppError::FileValidation(format!(
            "Saved MP4 '{}' contains no playable streams",
            sanitize_path_for_display(path)
        )));
    }
    Ok(bytes_read)
}

pub(crate) fn verify_streaming_mp4(
    original: &Path,
    saved: &Path,
    expected_output_len: u64,
    source_mdat_sha256: [u8; 32],
) -> Result<u64> {
    let mut file = File::open(saved).map_err(AppError::Io)?;
    let file_len = file.seek(SeekFrom::End(0)).map_err(AppError::Io)?;
    if file_len != expected_output_len {
        return Err(AppError::FileValidation(format!(
            "Streamed MP4 '{}' length mismatch ({file_len} vs expected {expected_output_len})",
            sanitize_path_for_display(saved)
        )));
    }
    let boxes = scan_top_level_strict(&mut file, file_len)?;
    let mdat_ranges: Vec<(u64, u64)> = boxes
        .iter()
        .filter(|item| &item.fourcc == b"mdat")
        .map(|item| (item.content_offset(), item.end()))
        .collect();
    if mdat_ranges.is_empty() {
        return Err(AppError::FileValidation(format!(
            "Streamed MP4 '{}' contains no mdat box",
            sanitize_path_for_display(saved)
        )));
    }
    let (actual_output_sha256, mdat_bytes) = hash_file_ranges(&mut file, &mdat_ranges)?;
    if actual_output_sha256 != source_mdat_sha256 {
        return Err(AppError::FileValidation(format!(
            "Streamed MP4 '{}' audio payload digest mismatch",
            sanitize_path_for_display(saved)
        )));
    }
    drop(file);
    check_indexed_audio_within_mdat(saved, &mdat_ranges)?;
    let packet_bytes = check_saved_audio(original, saved)?;
    Ok(header_and_moov_bytes(&boxes)
        .saturating_add(mdat_bytes)
        .saturating_add(packet_bytes))
}

fn check_indexed_audio_within_mdat(path: &Path, mdat_ranges: &[(u64, u64)]) -> Result<()> {
    let input = open_input(path)?;
    let audio_streams: Vec<usize> = input
        .streams()
        .into_iter()
        .filter(|stream| stream.medium == abb_ffmpeg::Medium::Audio)
        .map(|stream| stream.index)
        .collect();
    if audio_streams.is_empty() {
        return Err(AppError::FileValidation(format!(
            "Streamed MP4 '{}' contains no playable audio streams",
            sanitize_path_for_display(path)
        )));
    }
    for stream_index in audio_streams {
        let Some(ranges) = input.indexed_byte_ranges(stream_index) else {
            return Err(AppError::FileValidation(format!(
                "Streamed MP4 '{}' is missing indexed audio byte ranges",
                sanitize_path_for_display(path)
            )));
        };
        for (pos, size) in ranges {
            let Ok(start) = u64::try_from(pos) else {
                return Err(AppError::FileValidation(format!(
                    "Streamed MP4 '{}' contains negative audio packet offset",
                    sanitize_path_for_display(path)
                )));
            };
            let Ok(len) = u64::try_from(size) else {
                return Err(AppError::FileValidation(format!(
                    "Streamed MP4 '{}' contains negative audio packet size",
                    sanitize_path_for_display(path)
                )));
            };
            let Some(end) = start.checked_add(len) else {
                return Err(AppError::FileValidation(format!(
                    "Streamed MP4 '{}' audio packet range overflowed u64",
                    sanitize_path_for_display(path)
                )));
            };
            if !mdat_ranges
                .iter()
                .any(|&(mdat_start, mdat_end)| start >= mdat_start && end <= mdat_end)
            {
                return Err(AppError::FileValidation(format!(
                    "Streamed MP4 '{}' audio packet range [{start}..{end}) falls outside mdat",
                    sanitize_path_for_display(path)
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn hash_file_ranges(file: &mut File, ranges: &[(u64, u64)]) -> Result<([u8; 32], u64)> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; VERIFY_CHUNK_BYTES];
    let mut total_bytes = 0u64;
    for &(start, end) in ranges {
        if start >= end {
            continue;
        }
        file.seek(SeekFrom::Start(start)).map_err(AppError::Io)?;
        let mut remaining = end - start;
        while remaining > 0 {
            let want = usize::try_from(remaining.min(buf.len() as u64)).unwrap_or(buf.len());
            let read = match file.read(&mut buf[..want]) {
                Ok(0) => {
                    return Err(AppError::FileValidation(
                        "Unexpected EOF while hashing MP4 audio data".to_string(),
                    ));
                }
                Ok(n) => n,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(AppError::Io(error)),
            };
            hasher.update(&buf[..read]);
            total_bytes = total_bytes.saturating_add(read as u64);
            remaining -= read as u64;
        }
    }
    Ok((hasher.finalize().into(), total_bytes))
}

fn header_and_moov_bytes(boxes: &[BoxHeader]) -> u64 {
    boxes
        .iter()
        .map(|item| {
            if &item.fourcc == b"moov" {
                item.size
            } else {
                item.header_len
            }
        })
        .sum()
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
