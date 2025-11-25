//! Sample accumulation to build exact encoder-sized frames without truncation.
//! Phase 1 refactor: isolates frame sizing logic from main pipeline.
//! Phase 2: format-aware storage for both F32 planar and S16 packed formats.
//!
//! ## Supported Formats
//! - `F32(Planar)`: Native AAC encoder (ffmpeg's built-in aac)
//! - `I16(Packed)`: AAC-AT encoder (macOS AudioToolbox)
//!
//! Other formats will panic at construction time to prevent silent data corruption.

use ffmpeg_next as ff;
use log;

/// Storage abstraction for different sample formats.
///
/// Only supports formats actually used by our encoders:
/// - `F32Planar`: Separate buffers per channel with f32 samples (native AAC)
/// - `S16Packed`: Single interleaved buffer with i16 samples (AAC-AT)
///
/// Attempting to use other formats will panic at `SampleAccumulator::new()`.
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
    #[allow(dead_code)] // Kept for potential debug logging
    is_planar: bool,
}

impl SampleAccumulator {
    pub fn new(
        channels: usize,
        mut frame_size: usize,
        sample_rate: u32,
        channel_layout: ff::channel_layout::ChannelLayout,
        format: ff::format::Sample,
    ) -> Self {
        if frame_size == 0 {
            log::warn!("Encoder reported frame_size=0; using fallback 1024 for accumulation");
            frame_size = 1024; // AAC-LC typical frame size
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
                // Fail fast: unsupported formats would cause silent data corruption
                // if we tried to use F32Planar storage for I32 data, for example.
                panic!(
                    "SampleAccumulator: unsupported format {:?}. \
                     Only F32(Planar) and I16(Packed) are supported. \
                     Add explicit support if a new encoder requires a different format.",
                    unsupported
                );
            }
        };

        log::debug!(
            "SampleAccumulator: format={:?} bytes_per_sample={} is_planar={} channels={} frame_size={}",
            format, bytes_per_sample, is_planar, channels, frame_size
        );

        Self {
            channels,
            frame_size,
            sample_rate,
            channel_layout,
            format,
            storage,
            bytes_per_sample,
            is_planar,
        }
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
        if self.frame_size == 0 {
            return false;
        }
        match &self.storage {
            SampleStorage::F32Planar(buffers) => buffers[0].len() >= self.frame_size,
            SampleStorage::S16Packed(buffer) => buffer.len() / self.channels >= self.frame_size,
        }
    }

    /// Flush tail (pad to full frame if pad=true) returning optional final frame.
    pub fn flush_tail(&mut self, pad: bool) -> Option<ff::frame::Audio> {
        let is_empty = match &self.storage {
            SampleStorage::F32Planar(buffers) => buffers[0].is_empty(),
            SampleStorage::S16Packed(buffer) => buffer.is_empty(),
        };

        if is_empty {
            return None;
        }

        if pad {
            match &mut self.storage {
                SampleStorage::F32Planar(buffers) => {
                    if buffers[0].len() < self.frame_size {
                        let missing = self.frame_size - buffers[0].len();
                        for buf in buffers.iter_mut() {
                            buf.extend(std::iter::repeat_n(0.0f32, missing));
                        }
                    }
                }
                SampleStorage::S16Packed(buffer) => {
                    let samples_available = buffer.len() / self.channels;
                    if samples_available < self.frame_size {
                        let missing_samples = self.frame_size - samples_available;
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
        let frame_size = self.frame_size;
        let format = self.format;
        let channel_layout = self.channel_layout;
        let sample_rate = self.sample_rate;
        let channels = self.channels;

        match &mut self.storage {
            SampleStorage::F32Planar(buffers) => Self::drain_one_f32_planar(
                buffers,
                allow_short,
                frame_size,
                format,
                channel_layout,
                sample_rate,
                channels,
            ),
            SampleStorage::S16Packed(buffer) => Self::drain_one_s16_packed(
                buffer,
                allow_short,
                frame_size,
                format,
                channel_layout,
                sample_rate,
                channels,
            ),
        }
    }

    fn drain_one_f32_planar(
        buffers: &mut [Vec<f32>],
        allow_short: bool,
        frame_size: usize,
        format: ff::format::Sample,
        channel_layout: ff::channel_layout::ChannelLayout,
        sample_rate: u32,
        channels: usize,
    ) -> Option<ff::frame::Audio> {
        let available = buffers[0].len();
        if available == 0 {
            return None;
        }
        if !allow_short && available < frame_size {
            return None;
        }
        let take = available.min(frame_size);

        let mut frame = ff::frame::Audio::empty();
        frame.set_format(format);
        frame.set_channel_layout(channel_layout);
        frame.set_rate(sample_rate);
        frame.set_samples(take);
        unsafe {
            frame.alloc(format, take, channel_layout);
        }

        let mut total_repairs = 0usize;
        for (ch, buffer) in buffers.iter_mut().enumerate() {
            let plane = frame.data_mut(ch);
            if plane.is_empty() {
                continue;
            }
            let dst: &mut [f32] =
                unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, take) };
            let src = &buffer[..take];

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
            buffer.drain(..take);
        }

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
        buffer: &mut Vec<i16>,
        allow_short: bool,
        frame_size: usize,
        format: ff::format::Sample,
        channel_layout: ff::channel_layout::ChannelLayout,
        sample_rate: u32,
        channels: usize,
    ) -> Option<ff::frame::Audio> {
        let samples_available = buffer.len() / channels;
        if samples_available == 0 {
            return None;
        }
        if !allow_short && samples_available < frame_size {
            return None;
        }
        let take = samples_available.min(frame_size);
        let take_total = take * channels;

        let mut frame = ff::frame::Audio::empty();
        frame.set_format(format);
        frame.set_channel_layout(channel_layout);
        frame.set_rate(sample_rate);
        frame.set_samples(take);
        unsafe {
            frame.alloc(format, take, channel_layout);
        }

        // S16 packed: all data goes in plane 0, interleaved
        let plane = frame.data_mut(0);
        if !plane.is_empty() {
            let dst: &mut [i16] = unsafe {
                std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut i16, take_total)
            };
            let src = &buffer[..take_total];

            // No sanitization needed for integer samples (can't be NaN/Inf)
            // Just copy directly
            dst.copy_from_slice(src);
        }

        buffer.drain(..take_total);
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulator_two_frames_f32_planar() {
        let mut acc = SampleAccumulator::new(
            1,
            1024,
            44100,
            ff::channel_layout::ChannelLayout::MONO,
            ff::format::Sample::F32(ff::format::sample::Type::Planar),
        );
        let mut f = ff::frame::Audio::empty();
        f.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
        f.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
        f.set_rate(44100);
        f.set_samples(2048);
        unsafe {
            f.alloc(f.format(), 2048, f.channel_layout());
        }
        let produced = acc.push_frame(&f);
        assert_eq!(produced.len(), 2);
        assert!(acc.flush_tail(true).is_none());
    }

    #[test]
    fn accumulator_s16_packed_mono() {
        // Test S16 packed format (used by AAC-AT encoder)
        let mut acc = SampleAccumulator::new(
            1, // mono
            1024,
            44100,
            ff::channel_layout::ChannelLayout::MONO,
            ff::format::Sample::I16(ff::format::sample::Type::Packed),
        );

        // Verify format detection
        assert_eq!(acc.bytes_per_sample, 2);

        // Create input frame with 2048 samples (should produce 2 frames of 1024)
        let mut f = ff::frame::Audio::empty();
        f.set_format(ff::format::Sample::I16(ff::format::sample::Type::Packed));
        f.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
        f.set_rate(44100);
        f.set_samples(2048);
        unsafe {
            f.alloc(f.format(), 2048, f.channel_layout());
        }

        let produced = acc.push_frame(&f);
        assert_eq!(
            produced.len(),
            2,
            "Should produce 2 full frames from 2048 samples"
        );
        assert!(acc.flush_tail(true).is_none(), "No tail should remain");
    }

    #[test]
    fn accumulator_s16_packed_stereo() {
        // Test S16 packed stereo format
        let mut acc = SampleAccumulator::new(
            2, // stereo
            1024,
            44100,
            ff::channel_layout::ChannelLayout::STEREO,
            ff::format::Sample::I16(ff::format::sample::Type::Packed),
        );

        assert_eq!(acc.bytes_per_sample, 2);

        // Create input frame with 2048 samples per channel
        // For packed stereo, plane 0 contains interleaved L/R samples
        let mut f = ff::frame::Audio::empty();
        f.set_format(ff::format::Sample::I16(ff::format::sample::Type::Packed));
        f.set_channel_layout(ff::channel_layout::ChannelLayout::STEREO);
        f.set_rate(44100);
        f.set_samples(2048); // samples per channel
        unsafe {
            f.alloc(f.format(), 2048, f.channel_layout());
        }

        let produced = acc.push_frame(&f);
        assert_eq!(
            produced.len(),
            2,
            "Should produce 2 full frames from 2048 samples"
        );

        // Verify output frame format
        if let Some(out_frame) = produced.first() {
            assert_eq!(out_frame.samples(), 1024);
            assert!(matches!(
                out_frame.format(),
                ff::format::Sample::I16(ff::format::sample::Type::Packed)
            ));
        }
    }

    #[test]
    fn accumulator_s16_partial_flush() {
        // Test partial frame flush with padding for S16
        let mut acc = SampleAccumulator::new(
            1,
            1024,
            44100,
            ff::channel_layout::ChannelLayout::MONO,
            ff::format::Sample::I16(ff::format::sample::Type::Packed),
        );

        // Push only 512 samples (less than frame_size)
        let mut f = ff::frame::Audio::empty();
        f.set_format(ff::format::Sample::I16(ff::format::sample::Type::Packed));
        f.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
        f.set_rate(44100);
        f.set_samples(512);
        unsafe {
            f.alloc(f.format(), 512, f.channel_layout());
        }

        let produced = acc.push_frame(&f);
        assert_eq!(
            produced.len(),
            0,
            "Should not produce frame with only 512 samples"
        );

        // Flush with padding
        let tail = acc.flush_tail(true);
        assert!(tail.is_some(), "Should flush padded tail frame");
        if let Some(tail_frame) = tail {
            assert_eq!(
                tail_frame.samples(),
                1024,
                "Tail should be padded to full frame size"
            );
        }
    }

    #[test]
    #[should_panic(expected = "unsupported format")]
    fn accumulator_rejects_unsupported_i32_planar() {
        // Verify that unsupported formats panic at construction time
        // rather than silently corrupting data at runtime.
        // This addresses the memory safety concern from Gemini's PR #28 review.
        let _acc = SampleAccumulator::new(
            1,
            1024,
            44100,
            ff::channel_layout::ChannelLayout::MONO,
            ff::format::Sample::I32(ff::format::sample::Type::Planar),
        );
    }

    #[test]
    #[should_panic(expected = "unsupported format")]
    fn accumulator_rejects_unsupported_f32_packed() {
        // F32 packed is not supported (we only support F32 planar)
        let _acc = SampleAccumulator::new(
            1,
            1024,
            44100,
            ff::channel_layout::ChannelLayout::MONO,
            ff::format::Sample::F32(ff::format::sample::Type::Packed),
        );
    }

    #[test]
    #[should_panic(expected = "unsupported format")]
    fn accumulator_rejects_unsupported_i16_planar() {
        // I16 planar is not supported (we only support I16 packed)
        let _acc = SampleAccumulator::new(
            1,
            1024,
            44100,
            ff::channel_layout::ChannelLayout::MONO,
            ff::format::Sample::I16(ff::format::sample::Type::Planar),
        );
    }
}
