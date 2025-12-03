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

/// Encodes frame and writes packets to output
pub(crate) fn encode_and_write_frame(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    frame: &ff::frame::Audio,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    use crate::errors::AppError;

    #[cfg(debug_assertions)]
    {
        // Validate structural contract
        debug_validate_frame_contract(frame, encoder);
        // Validate sample values in debug builds (only for F32 format)
        if matches!(frame.format(), ff::format::Sample::F32(_)) {
            for ch in 0..encoder.channel_layout().channels() as usize {
                let plane = frame.data(ch);
                let len_f32 = plane.len() / 4;
                let src: &[f32] =
                    unsafe { std::slice::from_raw_parts(plane.as_ptr() as *const f32, len_f32) };
                for &v in src.iter().take(frame.samples()) {
                    debug_assert!(v.is_finite(), "Non-finite sample encountered");
                    debug_assert!((-1.0..=1.0).contains(&v), "Sample out of range [-1,1]");
                }
            }
        }
    }

    // Optional encode-stage sanitation (default ON, can be disabled via ABB_DISABLE_ENCODE_SANITIZE)
    // Only applies to F32 format; S16 format (used by AAC-AT) doesn't need float sanitation
    let disable_encode_sanitize = std::env::var("ABB_DISABLE_ENCODE_SANITIZE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let is_f32_format = matches!(frame.format(), ff::format::Sample::F32(_));

    if !disable_encode_sanitize && is_f32_format {
        // Clamp to [-1,1] and replace non-finite values; preserve frame metadata
        let mut clean = ff::frame::Audio::empty();
        clean.set_format(frame.format());
        clean.set_channel_layout(frame.channel_layout());
        clean.set_rate(frame.rate());
        clean.set_samples(frame.samples());
        unsafe {
            clean.alloc(frame.format(), frame.samples(), frame.channel_layout());
        }
        clean.set_pts(frame.pts());

        if frame.samples() > 0 {
            let channels = frame.channel_layout().channels() as usize;
            for ch in 0..channels {
                let src_plane = frame.data(ch);
                let dst_plane = clean.data_mut(ch);
                let len_f32 = (dst_plane.len() / 4).min(src_plane.len() / 4);
                let src: &[f32] = unsafe {
                    std::slice::from_raw_parts(src_plane.as_ptr() as *const f32, len_f32)
                };
                let dst: &mut [f32] = unsafe {
                    std::slice::from_raw_parts_mut(dst_plane.as_mut_ptr() as *mut f32, len_f32)
                };
                let mut repaired = 0usize;
                for i in 0..len_f32 {
                    let mut v = src[i];
                    if !v.is_finite() {
                        v = 0.0;
                        repaired += 1;
                    }
                    if v > 1.0 {
                        v = 1.0;
                        repaired += 1;
                    }
                    if v < -1.0 {
                        v = -1.0;
                        repaired += 1;
                    }
                    dst[i] = v;
                }
                if repaired > 0 {
                    log::warn!(
                        "Sanitized {} samples on channel {} before encoding",
                        repaired,
                        ch
                    );
                }
            }
        }

        encoder
            .send_frame(&clean)
            .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
    } else {
        encoder
            .send_frame(frame)
            .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;
    }
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

/// Flushes the encoder and writes trailer, used when preview early-stop is engaged
pub(crate) fn finalize_encoding_after_preview(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    // Currently identical to finalize_encoding; separated for clarity and future hooks
    finalize_encoding(
        encoder,
        output_context,
        output_stream_index,
        output_time_base,
    )
}
