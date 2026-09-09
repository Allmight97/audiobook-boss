//! One in-process session owns codec, muxing, and the submitted audio interval.

use super::{
    faac::{self, FaacEncoder},
    write,
};
use crate::{
    audio::EncoderType,
    errors::{AppError, Result},
};
use ffmpeg_next as ff;
use std::ffi::CStr;

pub(super) enum Backend {
    Ffmpeg(ff::codec::encoder::audio::Encoder),
    Faac(FaacEncoder),
}

impl Backend {
    pub fn rate(&self) -> u32 {
        match self {
            Self::Ffmpeg(e) => e.rate(),
            Self::Faac(e) => e.info.sample_rate,
        }
    }
    pub fn format(&self) -> ff::format::Sample {
        match self {
            Self::Ffmpeg(e) => e.format(),
            Self::Faac(_) => ff::format::Sample::F32(ff::format::sample::Type::Planar),
        }
    }
    pub fn channel_layout(&self) -> ff::ChannelLayout {
        match self {
            Self::Ffmpeg(e) => e.channel_layout(),
            Self::Faac(e) => ff::ChannelLayout::default(e.channels as i32),
        }
    }
    pub fn frame_size(&self) -> u32 {
        match self {
            Self::Ffmpeg(e) => e.frame_size(),
            Self::Faac(e) => e.info.frame_samples,
        }
    }
    pub fn time_base(&self) -> ff::Rational {
        ff::Rational(1, self.rate() as i32)
    }
    pub fn parameters(&self) -> Result<ff::codec::Parameters> {
        match self {
            Self::Ffmpeg(e) => Ok(ff::codec::Parameters::from(e)),
            Self::Faac(e) => e.parameters(),
        }
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
    output: ff::format::context::Output,
    stream_index: usize,
    time_base: ff::Rational,
    frame_samples: usize,
    resolved: EncoderType,
    submitted_samples: i64,
    stats: PacketStats,
    finished: bool,
}

impl EncoderSession {
    pub(super) fn new(
        backend: Backend,
        output: ff::format::context::Output,
        stream_index: usize,
        time_base: ff::Rational,
        frame_samples: usize,
        resolved: EncoderType,
    ) -> Self {
        Self {
            backend,
            output,
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
    pub(crate) fn format(&self) -> ff::format::Sample {
        self.backend.format()
    }
    pub(crate) fn channel_layout(&self) -> ff::ChannelLayout {
        self.backend.channel_layout()
    }
    pub(crate) fn samples_per_frame(&self) -> usize {
        self.frame_samples
    }
    pub(crate) fn name(&self) -> &'static str {
        match self.resolved {
            EncoderType::FaacHeAac => "faac",
            EncoderType::AacAt => "aac_at",
            _ => "aac",
        }
    }

    pub(crate) fn submit(&mut self, frame: &ff::frame::Audio) -> Result<()> {
        if self.finished || frame.pts() != Some(self.submitted_samples) {
            return Err(AppError::General(
                "Encoder input must follow the contiguous submitted sample timeline.".into(),
            ));
        }
        self.submitted_samples += frame.samples() as i64;
        match &mut self.backend {
            Backend::Ffmpeg(encoder) => write::encode_and_write_frame(
                encoder,
                frame,
                &mut self.output,
                self.stream_index,
                self.time_base,
                &mut self.stats,
            ),
            Backend::Faac(encoder) => {
                if let Some(packet) = encoder.encode(Some(frame))? {
                    write::write_packet(
                        packet,
                        &mut self.output,
                        self.stream_index,
                        self.time_base,
                        ff::Rational(1, self.backend.rate() as i32),
                        self.submitted_samples,
                        true,
                        &mut self.stats,
                    )?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn finish(&mut self) -> Result<()> {
        match &mut self.backend {
            Backend::Ffmpeg(encoder) => write::flush_encoder(
                encoder,
                &mut self.output,
                self.stream_index,
                self.time_base,
                self.submitted_samples,
                &mut self.stats,
            )?,
            Backend::Faac(encoder) => {
                let rate = encoder.info.sample_rate;
                while let Some(packet) = encoder.encode(None)? {
                    write::write_packet(
                        packet,
                        &mut self.output,
                        self.stream_index,
                        self.time_base,
                        ff::Rational(1, rate as i32),
                        self.submitted_samples,
                        true,
                        &mut self.stats,
                    )?;
                }
            }
        }
        self.output
            .write_trailer()
            .map_err(|e| AppError::General(format!("Write trailer failed: {e}")))?;
        self.finished = true;
        Ok(())
    }

    pub(crate) fn diagnostics(&self) -> String {
        // SAFETY: FFmpeg exposes a static, NUL-terminated version string.
        let ffmpeg_version =
            unsafe { CStr::from_ptr(ff::sys::av_version_info()) }.to_string_lossy();
        let codec = match &self.backend {
            Backend::Faac(e) => format!("faac_version={} profile=HE-AAC-v1 target_bitrate_bps={} core_priming_samples={} decoder_delay_samples={} reported_encoder_delay_samples={} timing=core_edit_list_with_postroll",
                faac::library_version(), e.info.bit_rate * e.channels, faac::HE_PRIMING_SAMPLES, faac::HE_DECODER_DELAY_SAMPLES, e.info.encoder_delay),
            Backend::Ffmpeg(_) if self.resolved == EncoderType::NativeAac => "profile=AAC-LC aac_coder=nmr aac_nmr_speed=0 options_verified=true".into(),
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
