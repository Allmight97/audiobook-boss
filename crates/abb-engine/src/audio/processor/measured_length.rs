use std::path::Path;

use ffmpeg_next as ff;

use super::decoder::AudioDecoder;
use super::faad_decoder::packet_trim;
use crate::audio::AacDecoder;
use crate::errors::{AppError, Result};

/// The length decoding yields, for a file whose container only estimates it.
/// Packets count samples exactly when FFmpeg decodes and the stream's time
/// base divides the output rate; any other stream is decoded and counted.
/// `None` keeps the estimate: a zero output rate or a packet with no duration.
pub(in crate::audio) fn measured_length_seconds(
    path: &Path,
    aac_decoder: AacDecoder,
) -> Result<Option<f64>> {
    let mut opened = super::streams::open_best_audio_decoder(path, aac_decoder)?;
    let rate = opened.decoder.rate();
    if rate == 0 {
        return Ok(None);
    }
    let time_base = opened
        .input
        .stream(opened.stream_index)
        .map(|stream| stream.time_base())
        .ok_or_else(|| AppError::InvalidInput("No audio stream found".to_string()))?;
    let counts_samples = i64::from(time_base.1) % i64::from(rate) == 0;
    let seconds = if opened.decoder.is_ffmpeg() && counts_samples {
        packet_seconds(&mut opened.input, opened.stream_index, time_base, rate)?
    } else {
        decoded_seconds(&mut opened.input, opened.stream_index, &mut opened.decoder)?
    };
    Ok(seconds.filter(|seconds| *seconds > 0.0))
}

fn packet_seconds(
    input: &mut ff::format::context::Input,
    stream_index: usize,
    time_base: ff::Rational,
    rate: u32,
) -> Result<Option<f64>> {
    let (mut ticks, mut trimmed) = (0_i64, 0_u64);
    let mut packet = ff::Packet::empty();
    while read_packet(input, &mut packet)? {
        if packet.stream() != stream_index {
            continue;
        }
        if packet.duration() <= 0 {
            return Ok(None);
        }
        let (skip, padding, discard) = packet_trim(&packet);
        if discard {
            continue;
        }
        ticks += packet.duration();
        trimmed += u64::from(skip) + u64::from(padding);
    }
    Ok(Some(
        ticks as f64 * f64::from(time_base) - trimmed as f64 / f64::from(rate),
    ))
}

fn decoded_seconds(
    input: &mut ff::format::context::Input,
    stream_index: usize,
    decoder: &mut AudioDecoder,
) -> Result<Option<f64>> {
    let mut samples = 0_u64;
    let mut packet = ff::Packet::empty();
    let mut frame = ff::frame::Audio::empty();
    while read_packet(input, &mut packet)? {
        if packet.stream() == stream_index {
            decoder.send_packet(&packet)?;
            samples += drain(decoder, &mut frame)?;
        }
    }
    decoder.send_eof()?;
    samples += drain(decoder, &mut frame)?;
    Ok(Some(samples as f64 / f64::from(decoder.rate())))
}

fn drain(decoder: &mut AudioDecoder, frame: &mut ff::frame::Audio) -> Result<u64> {
    let mut samples = 0_u64;
    loop {
        match decoder.receive_frame(frame) {
            Ok(()) => samples += frame.samples() as u64,
            Err(ff::Error::Other {
                errno: ff::error::EAGAIN,
            })
            | Err(ff::Error::Eof) => return Ok(samples),
            Err(error) => {
                return Err(AppError::InvalidInput(format!(
                    "Cannot decode the audio to measure its length: {error}"
                )))
            }
        }
    }
}

fn read_packet(input: &mut ff::format::context::Input, packet: &mut ff::Packet) -> Result<bool> {
    match packet.read(input) {
        Ok(()) => Ok(true),
        Err(ff::Error::Eof) => Ok(false),
        Err(error) => Err(AppError::InvalidInput(format!(
            "Cannot read the audio to measure its length: {error}"
        ))),
    }
}
