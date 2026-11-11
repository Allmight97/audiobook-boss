use super::{
    faac::{self, FaacEncoder},
    write,
};
use crate::{
    audio::EncoderType,
    errors::{AppError, Result},
};
use abb_ffmpeg::{AudioFrame, ChannelLayout, CodecId, Muxer, Rational, SampleFormat};

pub(super) enum Backend {
    Ffmpeg(abb_ffmpeg::AudioEncoder),
    Faac(FaacEncoder),
}

impl Backend {
    pub fn rate(&self) -> u32 {
        match self {
            Self::Ffmpeg(e) => e.rate(),
            Self::Faac(e) => e.rate,
        }
    }
    pub fn format(&self) -> SampleFormat {
        match self {
            Self::Ffmpeg(e) => e.format(),
            Self::Faac(_) => SampleFormat::F32_PLANAR,
        }
    }
    /// The layout the encoder was opened with; every encoder ABB opens
    /// takes a speaker-mask layout.
    pub fn channel_layout(&self) -> Result<ChannelLayout> {
        match self {
            Self::Ffmpeg(e) => e.layout().ok_or_else(|| {
                AppError::General("The opened encoder reports no usable channel layout.".into())
            }),
            Self::Faac(e) => Ok(ChannelLayout::default_for(e.channels as u16)),
        }
    }
    pub fn frame_size(&self) -> u32 {
        match self {
            Self::Ffmpeg(e) => e.frame_size() as u32,
            Self::Faac(e) => e.info.frame_samples,
        }
    }
    pub fn time_base(&self) -> Rational {
        Rational::new(1, self.rate() as i32)
    }
    /// Opus headers and decoded skip counts use the 48 kHz clock, even when
    /// libopus consumes 24 kHz PCM. Mux timing stays on that same clock so
    /// MP4 edit lists retain the full interval.
    pub fn add_stream_to(&self, output: &mut abb_ffmpeg::Output) -> Result<usize> {
        let added = match self {
            Self::Ffmpeg(e) => output.add_stream_for_encoder(e).and_then(|index| {
                if e.codec() == CodecId::OPUS {
                    output.restate_stream_sample_clock(index, 48_000)?;
                }
                Ok(index)
            }),
            Self::Faac(e) => output.add_aac_stream(&e.stream()),
        };
        added.map_err(|e| AppError::General(format!("Add output stream failed: {e}")))
    }
}

#[derive(Default)]
pub(super) struct PacketStats {
    pub packets: u64,
    pub bytes: u64,
    pub postroll_packets: u64,
    pub trailing_padding: i64,
    pub first_pts: Option<i64>,
    pub last_pts: Option<i64>,
}

pub(crate) struct EncoderSession {
    backend: Backend,
    layout: ChannelLayout,
    /// `None` once the trailer is written.
    output: Option<Muxer>,
    stream_index: usize,
    time_base: Rational,
    frame_samples: usize,
    resolved: EncoderType,
    submitted_samples: i64,
    stats: PacketStats,
    finished: bool,
}

impl EncoderSession {
    pub(super) fn new(
        backend: Backend,
        layout: ChannelLayout,
        output: Muxer,
        stream_index: usize,
        time_base: Rational,
        frame_samples: usize,
        resolved: EncoderType,
    ) -> Self {
        Self {
            backend,
            layout,
            output: Some(output),
            stream_index,
            time_base,
            frame_samples,
            resolved,
            submitted_samples: 0,
            stats: PacketStats::default(),
            finished: false,
        }
    }
    pub(crate) fn rate(&self) -> u32 {
        self.backend.rate()
    }
    pub(crate) fn format(&self) -> SampleFormat {
        self.backend.format()
    }
    pub(crate) fn channel_layout(&self) -> ChannelLayout {
        self.layout
    }
    pub(crate) fn samples_per_frame(&self) -> usize {
        self.frame_samples
    }
    pub(crate) fn name(&self) -> &'static str {
        crate::audio::settings_encoder::resolve_encoder_name(self.resolved)
    }

    pub(crate) fn submit(&mut self, frame: &AudioFrame) -> Result<()> {
        let Some(output) = self
            .output
            .as_mut()
            .filter(|_| frame.pts() == Some(self.submitted_samples))
        else {
            return Err(AppError::General(
                "Encoder input must follow the contiguous submitted sample timeline.".into(),
            ));
        };
        self.submitted_samples += frame.samples() as i64;
        match &mut self.backend {
            Backend::Ffmpeg(encoder) => write::encode_and_write_frame(
                encoder,
                frame,
                output,
                self.stream_index,
                self.time_base,
                &mut self.stats,
            ),
            Backend::Faac(encoder) => {
                if let Some(mut packet) = encoder.encode(Some(frame))? {
                    write::write_packet(
                        &mut packet,
                        output,
                        self.stream_index,
                        self.time_base,
                        Rational::new(1, encoder.rate as i32),
                        self.submitted_samples,
                        encoder.is_he(),
                        &mut self.stats,
                    )?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn finish(&mut self) -> Result<()> {
        let Some(mut output) = self.output.take() else {
            return Err(AppError::General("The encoder already finished.".into()));
        };
        match &mut self.backend {
            Backend::Ffmpeg(encoder) => write::flush_encoder(
                encoder,
                &mut output,
                self.stream_index,
                self.time_base,
                self.submitted_samples,
                &mut self.stats,
            )?,
            Backend::Faac(encoder) => {
                let rate = encoder.rate;
                while let Some(mut packet) = encoder.encode(None)? {
                    write::write_packet(
                        &mut packet,
                        &mut output,
                        self.stream_index,
                        self.time_base,
                        Rational::new(1, rate as i32),
                        self.submitted_samples,
                        encoder.is_he(),
                        &mut self.stats,
                    )?;
                }
            }
        }
        output
            .finish()
            .map_err(|e| AppError::General(format!("Write trailer failed: {e}")))?;
        self.finished = true;
        Ok(())
    }

    pub(crate) fn diagnostics(&self) -> String {
        let ffmpeg_version = abb_ffmpeg::ffmpeg_version();
        let codec = match &self.backend {
            Backend::Faac(e) => format!("faac_version={} profile={} rate_control={} target_bitrate_bps={} quality={} bandwidth={} core_priming_samples={} reported_encoder_delay_samples={}",
                faac::library_version(), e.profile_name(), if e.is_vbr() { "VBR" } else { "ABR" }, e.info.bit_rate, e.info.quality, e.info.bandwidth, e.priming(), e.info.encoder_delay),
            Backend::Ffmpeg(_) if self.resolved == EncoderType::NativeAac => "profile=AAC-LC aac_coder=nmr aac_rc=cbr options_verified=true".into(),
            Backend::Ffmpeg(_) if self.resolved == EncoderType::Opus => "codec=Opus encoder=libopus rate_control=VBR".into(),
            Backend::Ffmpeg(_) => "profile=AAC-LC encoder=AudioToolbox".into(),
        };
        let seconds = self.submitted_samples as f64 / f64::from(self.rate());
        let bitrate = if seconds > 0.0 {
            self.stats.bytes as f64 * 8.0 / seconds / 1000.0
        } else {
            0.0
        };
        format!("ffmpeg_version={ffmpeg_version}\n{codec}\noutput_rate={} output_channels={} pcm_format={:?} frame_samples={}\nsubmitted_samples={} encoded_packets={} encoded_payload_bytes={} achieved_payload_kbps={bitrate:.3}\nfirst_packet_pts={:?} last_packet_pts={:?} postroll_packets={} trailing_padding_samples={} mux_finished={}\n",
            self.rate(), self.channel_layout().channels(), self.format(), self.frame_samples,
            self.submitted_samples, self.stats.packets, self.stats.bytes, self.stats.first_pts,
            self.stats.last_pts, self.stats.postroll_packets, self.stats.trailing_padding, self.finished)
    }
}
