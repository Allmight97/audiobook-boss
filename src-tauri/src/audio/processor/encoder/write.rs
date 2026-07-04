//! Frame encoding and packet writing utilities.

use crate::errors::Result;
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
/// in `buffer.rs` `drain_one_f32_planar()`. FDK/AAC-AT use I16 format which doesn't
/// need float sanitization.
pub(crate) fn encode_and_write_frame(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    frame: &ff::frame::Audio,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    use crate::errors::AppError;

    #[cfg(debug_assertions)]
    debug_validate_frame_contract(frame, encoder);

    encoder
        .send_frame(frame)
        .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;

    let mut pkt = ff::Packet::empty();
    while encoder.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(output_stream_index);
        pkt.rescale_ts(encoder.time_base(), output_time_base);
        pkt.write_interleaved(output_context)
            .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
    }
    Ok(())
}

/// Flushes the encoder and writes the output trailer
pub(crate) fn finalize_encoding(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    use crate::errors::AppError;

    encoder.send_eof().ok();
    let mut pkt = ff::Packet::empty();
    while encoder.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(output_stream_index);
        pkt.rescale_ts(encoder.time_base(), output_time_base);
        pkt.write_interleaved(output_context)
            .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
    }

    output_context
        .write_trailer()
        .map_err(|e| AppError::General(format!("Write trailer failed: {e}")))?;
    Ok(())
}
