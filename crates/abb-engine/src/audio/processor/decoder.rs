//! The per-input audio decoder: FFmpeg's selected decoder, or bundled FAAD3
//! for AAC when the `AacDecoder` setting chooses it. Both yield FFmpeg audio
//! frames, so the resampler, accumulator, and encoder stay decoder-neutral.

use super::faad_decoder::FaadDecoder;
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::time::{Duration, Instant};

/// FAAC and FAAD library versions, for a host's build-identity log line.
pub(crate) fn codec_library_versions() -> String {
    format!(
        "faac={} faad={}",
        super::encoder::faac_library_version(),
        super::faad_decoder::library_version()
    )
}

enum Backend {
    Ffmpeg(ff::codec::decoder::Audio),
    Faad(Box<FaadDecoder>),
}

#[derive(Default)]
struct DecodeStats {
    packets: u64,
    emitted_samples: u64,
    decode_time: Duration,
}

pub(crate) struct AudioDecoder {
    backend: Backend,
    time_base: ff::Rational,
    bit_rate: usize,
    stats: DecodeStats,
    summary_label: Option<String>,
}

impl AudioDecoder {
    pub(super) fn ffmpeg(decoder: ff::codec::decoder::Audio) -> Self {
        Self {
            time_base: decoder.packet_time_base(),
            bit_rate: decoder.bit_rate(),
            backend: Backend::Ffmpeg(decoder),
            stats: DecodeStats::default(),
            summary_label: None,
        }
    }

    pub(super) fn faad(decoder: FaadDecoder, time_base: ff::Rational, bit_rate: usize) -> Self {
        Self {
            backend: Backend::Faad(Box::new(decoder)),
            time_base,
            bit_rate,
            stats: DecodeStats::default(),
            summary_label: None,
        }
    }

    /// Logs one `decode_summary` line for this input when the decoder drops.
    pub(super) fn log_summary_as(&mut self, label: String) {
        let faad = match &self.backend {
            Backend::Ffmpeg(_) => String::new(),
            Backend::Faad(decoder) => {
                let format = decoder.format();
                format!(
                    " object_type={} frame_samples={} decoder_delay={}",
                    format.object_type, format.frame_samples, format.decoder_delay
                )
            }
        };
        log::info!(
            "decode_open input={label} decoder={} rate={} channels={} format={:?} bit_rate={}{faad}",
            self.name(),
            self.rate(),
            self.channels(),
            self.format(),
            self.bit_rate
        );
        self.summary_label = Some(label);
    }

    /// The decoder that produces PCM: FFmpeg's codec name (`aac`, `aac_at`, ...) or `faad3`.
    pub(crate) fn name(&self) -> String {
        match &self.backend {
            Backend::Ffmpeg(decoder) => decoder
                .codec()
                .map_or_else(|| "unknown".into(), |codec| codec.name().to_string()),
            Backend::Faad(_) => "faad3".into(),
        }
    }

    pub(crate) fn id(&self) -> ff::codec::Id {
        match &self.backend {
            Backend::Ffmpeg(decoder) => decoder.id(),
            Backend::Faad(_) => ff::codec::Id::AAC,
        }
    }

    pub(crate) fn rate(&self) -> u32 {
        match &self.backend {
            Backend::Ffmpeg(decoder) => decoder.rate(),
            Backend::Faad(decoder) => decoder.format().rate,
        }
    }

    pub(crate) fn channels(&self) -> u16 {
        match &self.backend {
            Backend::Ffmpeg(decoder) => decoder.channels(),
            Backend::Faad(decoder) => decoder.format().channels as u16,
        }
    }

    pub(crate) fn format(&self) -> ff::format::Sample {
        match &self.backend {
            Backend::Ffmpeg(decoder) => decoder.format(),
            Backend::Faad(_) => ff::format::Sample::F32(ff::format::sample::Type::Packed),
        }
    }

    pub(crate) fn channel_layout(&self) -> ff::ChannelLayout {
        match &self.backend {
            Backend::Ffmpeg(decoder) => decoder.channel_layout(),
            Backend::Faad(decoder) => decoder.channel_layout(),
        }
    }

    /// Containers without channel-layout semantics (e.g. WAV/PCM) open with an
    /// unspecified layout while their decoded frames carry the default layout
    /// for the channel count; swresample then rejects every frame with "Input
    /// changed". Normalize so the decoder, its frames, and the resampler agree.
    pub(crate) fn normalized_channel_layout(&mut self) -> ff::ChannelLayout {
        let Backend::Ffmpeg(decoder) = &mut self.backend else {
            return self.channel_layout();
        };
        let mut layout = decoder.channel_layout();
        if layout.is_empty() && decoder.channels() > 0 {
            layout = ff::ChannelLayout::default(i32::from(decoder.channels()));
            decoder.set_channel_layout(layout);
            log::info!(
                "Input declared no channel layout; defaulting for {} channel(s)",
                decoder.channels()
            );
        }
        layout
    }

    pub(crate) fn bit_rate(&self) -> usize {
        self.bit_rate
    }

    pub(crate) fn profile(&self) -> ff::codec::Profile {
        match &self.backend {
            Backend::Ffmpeg(decoder) => decoder.profile(),
            Backend::Faad(_) => ff::codec::Profile::Unknown,
        }
    }

    pub(crate) fn packet_time_base(&self) -> ff::Rational {
        self.time_base
    }

    pub(crate) fn send_packet(&mut self, packet: &ff::Packet) -> Result<()> {
        let started = Instant::now();
        self.stats.packets += 1;
        let result = match &mut self.backend {
            Backend::Ffmpeg(decoder) => decoder
                .send_packet(packet)
                .map_err(|error| AppError::General(error.to_string())),
            Backend::Faad(decoder) => decoder.send_packet(packet),
        };
        self.stats.decode_time += started.elapsed();
        result
    }

    pub(crate) fn send_eof(&mut self) -> Result<()> {
        match &mut self.backend {
            Backend::Ffmpeg(decoder) => decoder
                .send_eof()
                .map_err(|error| AppError::General(error.to_string())),
            Backend::Faad(decoder) => {
                decoder.send_eof();
                Ok(())
            }
        }
    }

    pub(crate) fn receive_frame(
        &mut self,
        frame: &mut ff::frame::Audio,
    ) -> std::result::Result<(), ff::Error> {
        let started = Instant::now();
        let result = match &mut self.backend {
            Backend::Ffmpeg(decoder) => decoder.receive_frame(frame),
            Backend::Faad(decoder) => decoder.receive_frame(frame),
        };
        self.stats.decode_time += started.elapsed();
        if result.is_ok() {
            self.stats.emitted_samples += frame.samples() as u64;
        }
        result
    }
}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        let Some(label) = self.summary_label.take() else {
            return;
        };
        let stats = &self.stats;
        let rate = f64::from(self.rate().max(1));
        let audio_seconds = stats.emitted_samples as f64 / rate;
        let decode_seconds = stats.decode_time.as_secs_f64();
        let speed = if decode_seconds > 0.0 {
            audio_seconds / decode_seconds
        } else {
            0.0
        };
        let trims = match &self.backend {
            Backend::Ffmpeg(_) => String::new(),
            Backend::Faad(decoder) => {
                let c = decoder.counts();
                format!(
                    " delay_trimmed={} skip_trimmed={} padding_trimmed={} discarded_packets={} concealed_frames={} degraded_frames={}",
                    c.delay_samples, c.skipped_samples, c.padding_samples, c.discarded_packets, c.concealed_frames, c.degraded_frames
                )
            }
        };
        log::info!(
            "decode_summary input={label} decoder={} packets={} emitted_samples={} audio_s={audio_seconds:.3} decode_ms={:.1} speed_x={speed:.1}{trims}",
            self.name(),
            stats.packets,
            stats.emitted_samples,
            decode_seconds * 1000.0
        );
    }
}
