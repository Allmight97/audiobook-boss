//! Frame accumulator for aligning decoded sample counts to encoder frame_size
//!
//! AAC (and many codecs) expect fixed-size frames (e.g. 1024 samples) while
//! decoded MP3 frames often produce 1152 samples. Feeding oversized frames
//! can produce warnings and suppress output packets. This accumulator collects
//! resampled, normalized frames and emits exact-size chunks to the encoder.
//!
//! Design goals:
//! - Keep implementation simple (single Vec<f32> buffer) – mono/stereo handled
//!   by planar conversion from ffmpeg-next Audio frames.
//! - Avoid allocations in hot path: reserve once using a small multiple of frame_size.
//! - Return ready-to-encode ffmpeg_next::frame::Audio instances sized exactly
//!   to encoder.frame_size() except for a final remainder on flush.
//!
//! NOTE: We currently assume planar F32 sample format (configured upstream). If
//! future formats are introduced, extend with a trait or enum dispatch.

use ffmpeg_next as ff;

pub struct FrameAccumulator {
    frame_size: usize,
    channels: usize,
    buffer: Vec<f32>,
}

impl FrameAccumulator {
    pub fn new(frame_size: usize, channels: usize) -> Self {
        // Reserve space for ~4 frames worth initially
        let mut buffer = Vec::with_capacity(frame_size * channels * 4);
        Self { frame_size, channels, buffer }
    }

    /// Pushes a decoded+resampled frame into the accumulator and returns any
    /// fully aligned frames now available for encoding.
    pub fn push(&mut self, frame: &ff::frame::Audio) -> Vec<ff::frame::Audio> {
        let mut ready = Vec::new();
        // Extract planar samples into buffer (assumes f32 planar)
        let samples = frame.samples();
        for ch in 0..self.channels { // safe assumption: encoder layout
            if let Some(data) = frame.data(ch) {
                // Each channel plane length in bytes -> f32 slice
                let plane: &[f32] = unsafe {
                    std::slice::from_raw_parts(
                        data.as_ptr() as *const f32,
                        samples
                    )
                };
                // Interleave manually: push channel samples in sequence after each other per frame? Simpler: store planar contiguous.
                // For AAC encoding we can keep planar form; we'll reconstruct planar frames.
                // We append channel planes sequentially; later we will slice per channel when constructing frames.
                self.buffer.extend_from_slice(plane);
            }
        }
        // While we have at least one full frame worth of samples per channel
        while self.buffer.len() >= self.frame_size * self.channels {
            let frame = self.build_frame(false);
            ready.push(frame);
        }
        ready
    }

    /// Flush remainder (final partial frame). If pad == true pads with zeros.
    pub fn flush(mut self, pad: bool) -> Option<ff::frame::Audio> {
        if self.buffer.is_empty() { return None; }
        if pad {
            let needed = self.frame_size * self.channels - self.buffer.len();
            if needed > 0 { self.buffer.extend(std::iter::repeat(0.0).take(needed)); }
        }
        Some(self.build_frame(true))
    }

    fn build_frame(&mut self, allow_partial: bool) -> ff::frame::Audio {
        let samples_available_per_channel = self.buffer.len() / self.channels;
        let take_samples = if allow_partial { samples_available_per_channel } else { self.frame_size };
        let mut out = ff::frame::Audio::empty();
        out.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
        // channel_layout() is set by caller afterward (safer than assuming here)
        out.set_rate(0); // caller sets
        out.set_samples(take_samples as i32);
        out.set_channel_layout(ff::channel_layout::ChannelLayout::default(self.channels as i32));

        // Allocate buffer for each plane then copy
        out.allocate();
        for ch in 0..self.channels { 
            let start = ch * samples_available_per_channel;
            let end = start + take_samples;
            let plane_slice = &self.buffer[start..end];
            if let Some(mut data) = out.data_mut(ch) {
                let dst: &mut [f32] = unsafe {
                    std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut f32, take_samples)
                };
                dst.copy_from_slice(plane_slice);
            }
        }
        // Remove consumed samples (per channel contiguous grouping)
        let consumed = take_samples * self.channels;
        self.buffer.drain(0..consumed);
        out
    }
}
