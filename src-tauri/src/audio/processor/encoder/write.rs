//! Frame encoding and packet writing utilities.

use super::session::PacketStats;
use crate::errors::{AppError, Result};
use ff::packet::Mut;
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
pub(super) fn encode_and_write_frame(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    frame: &ff::frame::Audio,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
    stats: &mut PacketStats,
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
        stats,
    )
}

/// Flushes pending FFmpeg packets; the session owns the output trailer.
pub(super) fn flush_encoder(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
    audio_end_pts: i64,
    stats: &mut PacketStats,
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
        stats,
    )?;

    Ok(())
}

fn write_encoded_packets(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
    audio_end_pts: i64,
    stats: &mut PacketStats,
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
        write_packet(
            packet,
            output_context,
            output_stream_index,
            output_time_base,
            encoder.time_base(),
            audio_end_pts,
            false,
            stats,
        )?;
        packet = ff::Packet::empty();
    }
}

#[allow(clippy::too_many_arguments)] // Private mux handoff; callers outside encoder use EncoderSession.
pub(super) fn write_packet(
    mut packet: ff::Packet,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
    input_time_base: ff::Rational,
    audio_end_pts: i64,
    retain_postroll: bool,
    stats: &mut PacketStats,
) -> Result<()> {
    let pts = packet
        .pts()
        .ok_or_else(|| AppError::General("Encoded audio packet has no sample timestamp.".into()))?;
    let mut postroll = false;
    let mut trailing_padding = 0;
    if retain_postroll {
        if pts >= audio_end_pts {
            packet.set_flags(
                packet.flags() | ff::packet::Flags::from_bits_retain(ff::sys::AV_PKT_FLAG_DISCARD),
            );
            postroll = true;
        } else if pts + packet.duration() > audio_end_pts {
            let padding = pts + packet.duration() - audio_end_pts;
            // SAFETY: packet owns the new 10-byte FFmpeg skip-samples payload.
            // Preserve full access-unit duration so HE decoder postroll remains available.
            unsafe {
                let data = ff::sys::av_packet_new_side_data(
                    packet.as_mut_ptr(),
                    ff::sys::AVPacketSideDataType::AV_PKT_DATA_SKIP_SAMPLES,
                    10,
                );
                if data.is_null() {
                    return Err(AppError::General(
                        "Cannot allocate AAC padding metadata.".into(),
                    ));
                }
                std::ptr::write_bytes(data, 0, 10);
                std::ptr::copy_nonoverlapping(
                    (padding as u32).to_le_bytes().as_ptr(),
                    data.add(4),
                    4,
                );
            }
            trailing_padding = padding;
        }
    } else {
        if pts >= audio_end_pts {
            return Ok(());
        }
        trailing_padding = (pts + packet.duration() - audio_end_pts).max(0);
        packet.set_duration(packet.duration().min(audio_end_pts - pts));
    }
    let bytes = packet.size() as u64;
    packet.set_stream(output_stream_index);
    packet.rescale_ts(input_time_base, output_time_base);
    packet
        .write_interleaved(output_context)
        .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
    stats.postroll_packets += u64::from(postroll);
    stats.trailing_padding += trailing_padding;
    stats.packets += 1;
    stats.bytes += bytes;
    stats.first_pts.get_or_insert(pts);
    stats.last_pts = Some(pts);
    Ok(())
}
