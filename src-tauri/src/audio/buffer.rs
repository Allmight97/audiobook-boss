//! Sample accumulation to build exact encoder-sized frames without truncation.
//! Isolates frame sizing logic from the main pipeline. Supports F32 planar
//! and S16 packed formats.
//!
//! ## Supported Formats
//! - `F32(Planar)`: Native AAC encoder (ffmpeg's built-in aac)
//! - `I16(Packed)`: AAC-AT encoder (macOS AudioToolbox)
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
/// - `S16Packed`: Single interleaved buffer with i16 samples (AAC-AT)
///
/// Attempting to use other formats or a zero frame size will return an error from
/// `SampleAccumulator::new()`.
enum SampleStorage {
    F32Planar(Vec<Vec<f32>>),
    S16Packed(Vec<i16>), // Interleaved: [L0, R0, L1, R1, ...]
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
            ff::format::Sample::I16(ff::format::sample::Type::Packed) => {
                // AAC-AT (macOS AudioToolbox) uses S16 packed/interleaved
                let storage =
                    SampleStorage::S16Packed(Vec::with_capacity(frame_size * channels * 2));
                (2, false, storage)
            }
            unsupported => {
                log::error!(
                    "SampleAccumulator received unsupported format {:?}; only F32(Planar) and I16(Packed) are supported",
                    unsupported
                );
                return Err(AppError::General(format!(
                    "Unsupported encoder sample format {:?}; SampleAccumulator supports only F32(Planar) and I16(Packed)",
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
                    unsafe {
                        for (ch, buffer) in buffers.iter_mut().enumerate() {
                            let plane = frame.data(ch);
                            if plane.is_empty() {
                                log::warn!(
                                    "Frame plane {} is empty while {} samples were reported – padding with silence",
                                    ch,
                                    in_samples
                                );
                                buffer.extend(std::iter::repeat_n(0.0f32, in_samples));
                                continue;
                            }
                            let available_samples = plane.len() / self.bytes_per_sample;
                            let copy_len = available_samples.min(in_samples);
                            if copy_len < in_samples {
                                log::warn!(
                                    "Frame plane {} has fewer samples than reported (have={}, expected={}) – truncating copy",
                                    ch, copy_len, in_samples
                                );
                            }
                            let slice =
                                std::slice::from_raw_parts(plane.as_ptr() as *const f32, copy_len);
                            buffer.extend_from_slice(slice);
                            if copy_len < in_samples {
                                buffer.extend(std::iter::repeat_n(0.0f32, in_samples - copy_len));
                            }
                        }
                    }
                }
                SampleStorage::S16Packed(buffer) => {
                    // S16 packed: all channels interleaved in plane 0
                    unsafe {
                        let plane = frame.data(0);
                        if !plane.is_empty() {
                            // Total samples = bytes / 2 (S16)
                            // Samples per channel = total / channels
                            let total_samples = plane.len() / self.bytes_per_sample;
                            let samples_per_channel = total_samples / self.channels;
                            let expected_total = in_samples * self.channels;

                            if total_samples < expected_total {
                                log::warn!(
                                    "S16 packed frame has fewer samples than reported (have={}, expected={}) – truncating copy",
                                    samples_per_channel, in_samples
                                );
                            }

                            let copy_samples = total_samples.min(expected_total);
                            let slice = std::slice::from_raw_parts(
                                plane.as_ptr() as *const i16,
                                copy_samples,
                            );
                            buffer.extend_from_slice(slice);
                        }
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
            SampleStorage::S16Packed(buffer) => buffer
                .len()
                .saturating_div(self.channels)
                .saturating_sub(self.consumed_samples),
        }
    }

    /// Flush tail (pad to full frame if pad=true) returning optional final frame.
    pub fn flush_tail(&mut self, pad: bool) -> Option<ff::frame::Audio> {
        let available = self.available_samples();
        if available == 0 {
            return None;
        }

        if pad {
            match &mut self.storage {
                SampleStorage::F32Planar(buffers) => {
                    if available < self.frame_size {
                        let missing = self.frame_size - available;
                        for buf in buffers.iter_mut() {
                            buf.extend(std::iter::repeat_n(0.0f32, missing));
                        }
                    }
                }
                SampleStorage::S16Packed(buffer) => {
                    if available < self.frame_size {
                        let missing_samples = self.frame_size - available;
                        // Pad with silence (0) for all channels
                        buffer.extend(std::iter::repeat_n(0i16, missing_samples * self.channels));
                    }
                }
            }
        }
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
            SampleStorage::S16Packed(buffer) => {
                Self::drain_one_s16_packed(buffer, consumed_samples, allow_short, config)
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

        let mut total_repairs = 0usize;
        for (ch, buffer) in buffers.iter().enumerate() {
            let plane = frame.data_mut(ch);
            if plane.is_empty() {
                continue;
            }
            let dst: &mut [f32] =
                unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, take) };
            let start = *consumed_samples;
            let src = &buffer[start..start + take];

            // Sanitize float samples: clamp to [-1.0, 1.0], fix NaN/Inf
            let mut repaired = 0usize;
            for i in 0..take {
                let mut v = src[i];
                if !v.is_finite() {
                    v = 0.0;
                    repaired += 1;
                } else if v > 1.0 {
                    v = 1.0;
                    repaired += 1;
                } else if v < -1.0 {
                    v = -1.0;
                    repaired += 1;
                }
                dst[i] = v;
            }
            total_repairs += repaired;
        }
        *consumed_samples += take;

        if total_repairs > 0 {
            log::warn!(
                "Accumulator sanitized {} float samples before encoding (frame_size={})",
                total_repairs,
                take
            );
        }
        Some(frame)
    }

    fn drain_one_s16_packed(
        buffer: &[i16],
        consumed_samples: &mut usize,
        allow_short: bool,
        config: DrainConfig,
    ) -> Option<ff::frame::Audio> {
        let samples_available = buffer.len() / config.channels;
        let available = samples_available.saturating_sub(*consumed_samples);
        if available == 0 {
            return None;
        }
        if !allow_short && available < config.frame_size {
            return None;
        }
        let take = available.min(config.frame_size);
        let take_total = take * config.channels;
        let start = *consumed_samples * config.channels;

        let mut frame = ff::frame::Audio::empty();
        frame.set_format(config.format);
        frame.set_channel_layout(config.channel_layout);
        frame.set_rate(config.sample_rate);
        frame.set_samples(take);
        unsafe {
            frame.alloc(config.format, take, config.channel_layout);
        }

        // S16 packed: all data goes in plane 0, interleaved
        let plane = frame.data_mut(0);
        if !plane.is_empty() {
            let dst: &mut [i16] = unsafe {
                std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut i16, take_total)
            };
            let src = &buffer[start..start + take_total];

            // No sanitization needed for integer samples (can't be NaN/Inf)
            // Just copy directly
            dst.copy_from_slice(src);
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
            SampleStorage::S16Packed(buffer) => buffer.len() / self.channels,
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
            SampleStorage::S16Packed(buffer) => {
                let drop_total = self.consumed_samples * self.channels;
                buffer.drain(..drop_total);
            }
        }
        self.consumed_samples = 0;
    }
}
