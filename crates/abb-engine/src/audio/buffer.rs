//! Sample accumulation to build exact encoder-sized frames without truncation.
//! Isolates frame sizing logic from the main pipeline. Supports F32 planar
//! and packed F32/S16 formats.
//!
//! ## Supported Formats
//! - `F32(Planar)`: Native AAC encoder (ffmpeg's built-in aac)
//! - `I16(Packed)`: AAC-AT encoder (macOS AudioToolbox)
//! - `F32(Packed)`: Opus encoder
//!
//! Other formats and invalid frame sizes return an error at construction time to prevent
//! silent data corruption.

use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use log;

/// Storage abstraction for different sample formats.
///
/// Only supports formats actually used by our encoders:
/// - `F32Planar`: Separate buffers per channel with f32 samples (native AAC)
/// - `Packed`: Interleaved sample bytes (AAC-AT or Opus)
///
/// Attempting to use other formats or a zero frame size will return an error from
/// `SampleAccumulator::new()`.
enum SampleStorage {
    F32Planar(Vec<Vec<f32>>),
    Packed(Vec<u8>), // Interleaved sample bytes: [L0, R0, L1, R1, ...]
}

pub struct SampleAccumulator {
    channels: usize,
    frame_size: usize,
    sample_rate: u32,
    channel_layout: ff::channel_layout::ChannelLayout,
    format: ff::format::Sample,
    storage: SampleStorage,
    bytes_per_sample: usize,
    consumed_samples: usize,
}

#[derive(Clone, Copy)]
struct DrainConfig {
    frame_size: usize,
    format: ff::format::Sample,
    channel_layout: ff::channel_layout::ChannelLayout,
    sample_rate: u32,
    channels: usize,
}

impl SampleAccumulator {
    pub fn new(
        channels: usize,
        frame_size: usize,
        sample_rate: u32,
        channel_layout: ff::channel_layout::ChannelLayout,
        format: ff::format::Sample,
    ) -> Result<Self> {
        if frame_size == 0 {
            return Err(AppError::General(
                "Encoder reported frame_size=0; SampleAccumulator requires a fixed non-zero frame size"
                    .to_string(),
            ));
        }

        // Explicit format matching - only support formats actually used by our encoders.
        // This prevents silent data corruption from format/storage mismatches.
        // See: Gemini review on PR #28 regarding memory safety concern.
        let (bytes_per_sample, is_planar, storage) = match format {
            ff::format::Sample::F32(ff::format::sample::Type::Planar) => {
                // Native AAC encoder uses F32 planar
                let storage = SampleStorage::F32Planar(
                    (0..channels)
                        .map(|_| Vec::with_capacity(frame_size * 2))
                        .collect(),
                );
                (4, true, storage)
            }
            ff::format::Sample::I16(ff::format::sample::Type::Packed)
            | ff::format::Sample::F32(ff::format::sample::Type::Packed) => {
                let bytes = format.bytes();
                let storage =
                    SampleStorage::Packed(Vec::with_capacity(frame_size * channels * bytes * 2));
                (bytes, false, storage)
            }
            unsupported => {
                log::error!(
                    "SampleAccumulator received unsupported format {:?}; only F32(Planar/Packed) and I16(Packed) are supported",
                    unsupported
                );
                return Err(AppError::General(format!(
                    "Unsupported encoder sample format {:?}; SampleAccumulator supports only F32(Planar/Packed) and I16(Packed)",
                    unsupported
                )));
            }
        };

        log::debug!(
            "SampleAccumulator: format={:?} bytes_per_sample={} is_planar={} channels={} frame_size={}",
            format, bytes_per_sample, is_planar, channels, frame_size
        );

        Ok(Self {
            channels,
            frame_size,
            sample_rate,
            channel_layout,
            format,
            storage,
            bytes_per_sample,
            consumed_samples: 0,
        })
    }
    /// Push a frame; return any full frames now available.
    pub fn push_frame(&mut self, frame: &ff::frame::Audio) -> Vec<ff::frame::Audio> {
        let mut ready = Vec::new();
        let in_samples = frame.samples();
        if in_samples == 0 {
            return ready;
        }

        // First, copy samples into storage (scoped to release borrow)
        {
            match &mut self.storage {
                SampleStorage::F32Planar(buffers) => {
                    // F32 planar: each channel in separate plane
                    let available_planes = frame.planes();
                    for (ch, buffer) in buffers.iter_mut().enumerate() {
                        if ch >= available_planes {
                            log::warn!(
                                "Frame plane {} is missing while {} samples were reported - padding with silence",
                                ch,
                                in_samples
                            );
                            buffer.extend(std::iter::repeat_n(0.0f32, in_samples));
                            continue;
                        }

                        let plane = frame.plane::<f32>(ch);
                        let copy_len = plane.len().min(in_samples);
                        if copy_len < in_samples {
                            log::warn!(
                                "Frame plane {} has fewer samples than reported (have={}, expected={}) - padding remainder with silence",
                                ch, copy_len, in_samples
                            );
                        }
                        buffer.extend_from_slice(&plane[..copy_len]);
                        if copy_len < in_samples {
                            buffer.extend(std::iter::repeat_n(0.0f32, in_samples - copy_len));
                        }
                    }
                }
                SampleStorage::Packed(buffer) => {
                    let plane = frame.data(0);
                    let expected = in_samples * self.channels * self.bytes_per_sample;
                    let copied = plane.len().min(expected);
                    buffer.extend_from_slice(&plane[..copied]);
                    if copied < expected {
                        log::warn!("Packed frame has fewer bytes than reported (have={copied}, expected={expected}); truncating copy");
                    }
                }
            }
        }

        // Now drain full frames (borrow released above)
        while self.has_full_frame() {
            if let Some(f) = self.drain_one(false) {
                ready.push(f);
            } else {
                break;
            }
        }

        ready
    }

    /// Check if storage has enough samples for a full frame
    fn has_full_frame(&self) -> bool {
        self.available_samples() >= self.frame_size
    }

    fn available_samples(&self) -> usize {
        match &self.storage {
            SampleStorage::F32Planar(buffers) => buffers
                .iter()
                .map(|buffer| buffer.len().saturating_sub(self.consumed_samples))
                .min()
                .unwrap_or(0),
            SampleStorage::Packed(buffer) => buffer
                .len()
                .saturating_div(self.channels * self.bytes_per_sample)
                .saturating_sub(self.consumed_samples),
        }
    }

    /// Return the real final samples; the encoder owns codec padding.
    pub fn flush_tail(&mut self) -> Option<ff::frame::Audio> {
        self.drain_one(true)
    }

    fn drain_one(&mut self, allow_short: bool) -> Option<ff::frame::Audio> {
        // Extract immutable fields to avoid borrow conflicts
        let config = DrainConfig {
            frame_size: self.frame_size,
            format: self.format,
            channel_layout: self.channel_layout,
            sample_rate: self.sample_rate,
            channels: self.channels,
        };
        let consumed_samples = &mut self.consumed_samples;

        let frame = match &self.storage {
            SampleStorage::F32Planar(buffers) => {
                Self::drain_one_f32_planar(buffers, consumed_samples, allow_short, config)
            }
            SampleStorage::Packed(buffer) => {
                Self::drain_one_packed(buffer, consumed_samples, allow_short, config)
            }
        };

        if frame.is_some() {
            self.compact_if_needed();
        }
        frame
    }

    fn drain_one_f32_planar(
        buffers: &[Vec<f32>],
        consumed_samples: &mut usize,
        allow_short: bool,
        config: DrainConfig,
    ) -> Option<ff::frame::Audio> {
        let available = buffers[0].len().saturating_sub(*consumed_samples);
        if available == 0 {
            return None;
        }
        if !allow_short && available < config.frame_size {
            return None;
        }
        let take = available.min(config.frame_size);

        let mut frame = ff::frame::Audio::empty();
        frame.set_format(config.format);
        frame.set_channel_layout(config.channel_layout);
        frame.set_rate(config.sample_rate);
        frame.set_samples(take);
        unsafe {
            frame.alloc(config.format, take, config.channel_layout);
        }

        for (ch, buffer) in buffers.iter().enumerate() {
            if ch >= frame.planes() {
                log::warn!(
                    "Allocated F32 planar frame is missing plane {} while {} channels were requested",
                    ch,
                    config.channels
                );
                continue;
            }

            let dst = frame.plane_mut::<f32>(ch);
            let start = *consumed_samples;
            let src = &buffer[start..start + take];

            let mut report = SanitizeReport::default();
            for (dst, &src) in dst[..take].iter_mut().zip(src) {
                *dst = report.sanitize(src);
            }
            report.log(&format!("channel_index={ch}"), take);
        }
        *consumed_samples += take;
        Some(frame)
    }

    fn drain_one_packed(
        buffer: &[u8],
        consumed_samples: &mut usize,
        allow_short: bool,
        config: DrainConfig,
    ) -> Option<ff::frame::Audio> {
        let bytes_per_frame = config.channels * config.format.bytes();
        let samples_available = buffer.len() / bytes_per_frame;
        let available = samples_available.saturating_sub(*consumed_samples);
        if available == 0 {
            return None;
        }
        if !allow_short && available < config.frame_size {
            return None;
        }
        let take = available.min(config.frame_size);
        let take_total = take * bytes_per_frame;
        let start = *consumed_samples * bytes_per_frame;

        let mut frame = ff::frame::Audio::empty();
        frame.set_format(config.format);
        frame.set_channel_layout(config.channel_layout);
        frame.set_rate(config.sample_rate);
        frame.set_samples(take);
        unsafe {
            frame.alloc(config.format, take, config.channel_layout);
        }

        // Packed samples share plane zero; byte copies preserve alignment and precision.
        let dst = &mut frame.data_mut(0)[..take_total];
        dst.copy_from_slice(&buffer[start..start + take_total]);
        if config.format == ff::format::Sample::F32(ff::format::sample::Type::Packed) {
            let mut report = SanitizeReport::default();
            for bytes in dst.chunks_exact_mut(4) {
                let value = f32::from_ne_bytes(bytes.try_into().expect("four-byte float"));
                bytes.copy_from_slice(&report.sanitize(value).to_ne_bytes());
            }
            report.log("channel_index=packed", take);
        }

        *consumed_samples += take;
        Some(frame)
    }

    fn compact_if_needed(&mut self) {
        if self.consumed_samples == 0 {
            return;
        }

        let total_samples = match &self.storage {
            SampleStorage::F32Planar(buffers) => buffers[0].len(),
            SampleStorage::Packed(buffer) => buffer.len() / (self.channels * self.bytes_per_sample),
        };
        let compact_threshold = self.frame_size.saturating_mul(2);
        let should_compact = self.consumed_samples >= compact_threshold
            && self.consumed_samples.saturating_mul(2) >= total_samples;

        if !should_compact {
            return;
        }

        match &mut self.storage {
            SampleStorage::F32Planar(buffers) => {
                for buffer in buffers.iter_mut() {
                    buffer.drain(..self.consumed_samples);
                }
            }
            SampleStorage::Packed(buffer) => {
                let drop_total = self.consumed_samples * self.channels * self.bytes_per_sample;
                buffer.drain(..drop_total);
            }
        }
        self.consumed_samples = 0;
    }
}

/// Clamped peaks up to +1 dBFS are routine for lossy decodes and resampling
/// of masters normalized near 0 dBFS, and the clamp is inaudible. Larger
/// excursions suggest a gain, format, or layout mismatch worth investigating.
const CLIP_WARN_PEAK: f32 = 1.122;

/// Float sample repair before encoding: clamp to [-1.0, 1.0], zero NaN/Inf.
#[derive(Default)]
struct SanitizeReport {
    clipped: usize,
    non_finite: usize,
    clipped_peak: f32,
}

impl SanitizeReport {
    fn sanitize(&mut self, value: f32) -> f32 {
        if !value.is_finite() {
            self.non_finite += 1;
            return 0.0;
        }
        if value.abs() > 1.0 {
            self.clipped += 1;
            self.clipped_peak = self.clipped_peak.max(value.abs());
        }
        value.clamp(-1.0, 1.0)
    }

    /// NaN/Inf always warn; clipping warns only above `CLIP_WARN_PEAK`.
    fn level(&self) -> Option<log::Level> {
        if self.non_finite > 0 || self.clipped_peak > CLIP_WARN_PEAK {
            Some(log::Level::Warn)
        } else if self.clipped > 0 {
            Some(log::Level::Debug)
        } else {
            None
        }
    }

    fn log(&self, scope: &str, frame_size: usize) {
        if let Some(level) = self.level() {
            log::log!(
                level,
                "Accumulator sanitized float samples before encoding: {scope} clipped={} non_finite={} clipped_peak={:.6} frame_size={frame_size}",
                self.clipped,
                self.non_finite,
                self.clipped_peak
            );
        }
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
