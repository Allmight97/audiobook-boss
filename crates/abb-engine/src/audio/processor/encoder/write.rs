use super::session::PacketStats;
use crate::errors::{AppError, Result};
use abb_ffmpeg::{
    AudioEncoder, AudioFrame, CodecId, Muxer, Packet, Progress, Rational, SkipSamples,
};

#[cfg(debug_assertions)]
fn debug_validate_frame_contract(frame: &AudioFrame, encoder: &AudioEncoder) {
    debug_assert_eq!(
        frame.format(),
        encoder.format(),
        "Frame format must match encoder format"
    );
    debug_assert_eq!(frame.layout(), encoder.layout(), "Channel layout mismatch");
    debug_assert_eq!(frame.rate(), encoder.rate(), "Sample rate mismatch");

    let samples = frame.samples();
    debug_assert!(samples > 0, "Frame must contain at least one sample");
    if encoder.frame_size() > 0 {
        debug_assert!(
            samples <= encoder.frame_size(),
            "Frame samples exceed encoder.frame_size()"
        );
    }

    debug_assert!(
        frame.pts().is_some(),
        "Frame PTS must be set before encoding"
    );
}

pub(super) fn encode_and_write_frame(
    encoder: &mut AudioEncoder,
    frame: &AudioFrame,
    output: &mut Muxer,
    output_stream_index: usize,
    output_time_base: Rational,
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
        output,
        output_stream_index,
        output_time_base,
        audio_end_pts,
        stats,
    )
}

pub(super) fn flush_encoder(
    encoder: &mut AudioEncoder,
    output: &mut Muxer,
    output_stream_index: usize,
    output_time_base: Rational,
    audio_end_pts: i64,
    stats: &mut PacketStats,
) -> Result<()> {
    encoder
        .send_end()
        .map_err(|e| AppError::General(format!("Encoder flush failed: {e}")))?;
    write_encoded_packets(
        encoder,
        output,
        output_stream_index,
        output_time_base,
        audio_end_pts,
        stats,
    )
}

fn write_encoded_packets(
    encoder: &mut AudioEncoder,
    output: &mut Muxer,
    output_stream_index: usize,
    output_time_base: Rational,
    audio_end_pts: i64,
    stats: &mut PacketStats,
) -> Result<()> {
    let receive_failure = |error| AppError::General(format!("Encoder receive failed: {error}"));
    let mut packet = Packet::new().map_err(receive_failure)?;
    loop {
        match encoder
            .receive_packet(&mut packet)
            .map_err(receive_failure)?
        {
            Progress::Ready => {}
            Progress::NeedsInput | Progress::Finished => return Ok(()),
        }
        if encoder.codec() == CodecId::OPUS && encoder.rate() != 48_000 {
            rescale_opus_skip_samples(&mut packet, encoder.rate())?;
        }
        write_packet(
            &mut packet,
            output,
            output_stream_index,
            output_time_base,
            encoder.time_base(),
            audio_end_pts,
            false,
            stats,
        )?;
    }
}

/// Writes one encoded packet. With `retain_postroll` (HE-AAC from FAAC) a
/// packet past the audio is kept and marked discard, and the packet that
/// crosses the end keeps its full duration with its padding as skip
/// metadata, so the HE decoder's postroll stays available.
#[expect(
    clippy::too_many_arguments,
    reason = "Private mux handoff; callers outside encoder use EncoderSession"
)]
pub(super) fn write_packet(
    packet: &mut Packet,
    output: &mut Muxer,
    output_stream_index: usize,
    output_time_base: Rational,
    input_time_base: Rational,
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
            packet.mark_discard();
            postroll = true;
        } else if pts + packet.duration() > audio_end_pts {
            let padding = pts + packet.duration() - audio_end_pts;
            packet
                .set_skip_samples(SkipSamples {
                    end: padding as u32,
                    ..SkipSamples::default()
                })
                .map_err(|_| AppError::General("Cannot allocate AAC padding metadata.".into()))?;
            trailing_padding = padding;
        }
    } else {
        // aac_at flushes one packet from an empty frame queue, stamped inside
        // audio already written and carrying no samples.
        if pts >= audio_end_pts || stats.written_end.is_some_and(|end| pts < end) {
            return Ok(());
        }
        trailing_padding = (pts + packet.duration() - audio_end_pts).max(0);
        packet.set_duration(packet.duration().min(audio_end_pts - pts));
        stats.written_end = Some(pts + packet.duration());
    }
    let bytes = packet.size() as u64;
    packet.set_stream_index(output_stream_index);
    packet.rescale_ts(input_time_base, output_time_base);
    output
        .write_interleaved(packet)
        .map_err(|e| AppError::General(format!("Write packet failed: {e}")))?;
    stats.postroll_packets += u64::from(postroll);
    stats.trailing_padding += trailing_padding;
    stats.packets += 1;
    stats.bytes += bytes;
    stats.first_pts.get_or_insert(pts);
    stats.last_pts = Some(pts);
    Ok(())
}

/// libopus counts trims at its input rate; Opus containers count decoded
/// samples at 48 kHz.
fn rescale_opus_skip_samples(packet: &mut Packet, input_rate: u32) -> Result<()> {
    let Some(skip) = packet
        .skip_samples()
        .map_err(|_| AppError::General("Invalid Opus trim metadata.".into()))?
    else {
        return Ok(());
    };
    let scaled = |count: u32| (u64::from(count) * 48_000 / u64::from(input_rate)) as u32;
    packet
        .set_skip_samples(SkipSamples {
            start: scaled(skip.start),
            end: scaled(skip.end),
            ..skip
        })
        .map_err(|_| AppError::General("Cannot allocate Opus trim metadata.".into()))
}
