//! Frame encoding and packet writing utilities.

use crate::errors::{AppError, Result};
use ffmpeg_next as ff;

#[cfg(debug_assertions)]
fn debug_validate_frame_contract(
    frame: &ff::frame::Audio,
    encoder: &ff::codec::encoder::audio::Encoder,
) {
    // Format/layout/rate must match encoder
    debug_assert_eq!(
        frame.format(),
        encoder.format(),
        "Frame format must match encoder format"
    );
    debug_assert_eq!(
        frame.channel_layout(),
        encoder.channel_layout(),
        "Channel layout mismatch"
    );
    debug_assert_eq!(frame.rate(), encoder.rate(), "Sample rate mismatch");

    // Samples must be > 0 and respect encoder frame size if non-zero
    let samples_i64 = frame.samples() as i64;
    debug_assert!(samples_i64 > 0, "Frame must contain at least one sample");
    let enc_frame_size_i64 = encoder.frame_size() as i64;
    if enc_frame_size_i64 > 0 {
        debug_assert!(
            samples_i64 <= enc_frame_size_i64,
            "Frame samples exceed encoder.frame_size()"
        );
    }

    // PTS should be set
    debug_assert!(
        frame.pts().is_some(),
        "Frame PTS must be set before encoding"
    );
}

/// Encodes frame and writes packets to output.
///
/// Note: F32 sample sanitization (NaN/Inf → 0, clamp to [-1,1]) is handled upstream
/// in `buffer.rs` `drain_one_f32_planar()`. AAC-AT uses I16 format which doesn't
/// need float sanitization.
pub(crate) fn encode_and_write_frame(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    frame: &ff::frame::Audio,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    #[cfg(debug_assertions)]
    debug_validate_frame_contract(frame, encoder);

    encoder
        .send_frame(frame)
        .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;

    let audio_end_pts = frame
        .pts()
        .ok_or_else(|| AppError::General("Encoder input frame has no sample timestamp.".into()))?
        + frame.samples() as i64;
    write_encoded_packets(
        encoder,
        output_context,
        output_stream_index,
        output_time_base,
        audio_end_pts,
    )
}

/// Flushes the encoder and writes the output trailer
pub(crate) fn finalize_encoding(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
    audio_end_pts: i64,
) -> Result<()> {
    encoder
        .send_eof()
        .map_err(|e| AppError::General(format!("Encoder flush failed: {e}")))?;
    write_encoded_packets(
        encoder,
        output_context,
        output_stream_index,
        output_time_base,
        audio_end_pts,
    )?;

    output_context
        .write_trailer()
        .map_err(|e| AppError::General(format!("Write trailer failed: {e}")))?;
    Ok(())
}

fn write_encoded_packets(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
    audio_end_pts: i64,
) -> Result<()> {
    let mut packet = ff::Packet::empty();
    loop {
        match encoder.receive_packet(&mut packet) {
            Ok(()) => {}
            Err(ff::Error::Eof)
            | Err(ff::Error::Other {
                errno: ff::error::EAGAIN,
            }) => return Ok(()),
            Err(error) => {
                return Err(AppError::General(format!(
                    "Encoder receive failed: {error}"
                )))
            }
        }
        let pts = packet.pts().ok_or_else(|| {
            AppError::General("Encoded audio packet has no sample timestamp.".into())
        })?;
        // AudioToolbox can report a full final packet even for a short input
        // tail. The container's playable duration ends at the submitted audio.
        if pts >= audio_end_pts {
            continue;
        }
        packet.set_duration(packet.duration().min(audio_end_pts - pts));
        packet.set_stream(output_stream_index);
        packet.rescale_ts(encoder.time_base(), output_time_base);
        packet
            .write_interleaved(output_context)
            .map_err(|error| AppError::General(format!("Write packet failed: {error}")))?;
    }
}
