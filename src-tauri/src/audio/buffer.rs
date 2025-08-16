//! Sample accumulation to build exact encoder-sized frames without truncation.
//! Phase 1 refactor: isolates frame sizing logic from main pipeline.

use ffmpeg_next as ff;
use log;

pub struct SampleAccumulator {
    channels: usize,
    frame_size: usize,
    sample_rate: u32,
    channel_layout: ff::channel_layout::ChannelLayout,
    format: ff::format::Sample,
    buffers: Vec<Vec<f32>>, // planar storage
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
        Self {
            channels,
            frame_size,
            sample_rate,
            channel_layout,
            format,
            buffers: (0..channels).map(|_| Vec::with_capacity(frame_size * 2)).collect(),
        }
    }

    /// Push a frame; return any full frames now available.
    pub fn push_frame(&mut self, frame: &ff::frame::Audio) -> Vec<ff::frame::Audio> {
        let mut ready = Vec::new();
        let in_samples = frame.samples() as usize;
        if in_samples == 0 { return ready; }
        unsafe {
            for ch in 0..self.channels {
                let plane = frame.data(ch);
                if plane.is_empty() { continue; }
                let slice = std::slice::from_raw_parts(
                    plane.as_ptr() as *const f32,
                    in_samples,
                );
                self.buffers[ch].extend_from_slice(slice);
            }
        }
    while self.frame_size > 0 && self.buffers[0].len() >= self.frame_size {
            if let Some(f) = self.drain_one(false) { ready.push(f); }
        }
        ready
    }

    /// Flush tail (pad to full frame if pad=true) returning optional final frame.
    pub fn flush_tail(&mut self, pad: bool) -> Option<ff::frame::Audio> {
        if self.buffers[0].is_empty() { return None; }
        if pad && self.buffers[0].len() < self.frame_size {
            let missing = self.frame_size - self.buffers[0].len();
            for ch in 0..self.channels {
                self.buffers[ch].extend(std::iter::repeat(0.0).take(missing));
            }
        }
        self.drain_one(true)
    }

    fn drain_one(&mut self, allow_short: bool) -> Option<ff::frame::Audio> {
        let available = self.buffers[0].len();
        if available == 0 { return None; }
        if !allow_short && available < self.frame_size { return None; }
        let take = available.min(self.frame_size);
        let mut frame = ff::frame::Audio::empty();
        frame.set_format(self.format);
        frame.set_channel_layout(self.channel_layout);
    frame.set_rate(self.sample_rate);
        frame.set_samples(take);
        unsafe { frame.alloc(self.format, take, self.channel_layout); }
        for ch in 0..self.channels {
            let plane = frame.data_mut(ch);
            if plane.is_empty() { continue; }
            let bytes_per_sample = 4; // f32
            let take_bytes = take * bytes_per_sample;
            let src = &self.buffers[ch][..take];
            let src_bytes = unsafe {
                std::slice::from_raw_parts(
                    src.as_ptr() as *const u8,
                    take_bytes,
                )
            };
            plane[..take_bytes].copy_from_slice(src_bytes);
            self.buffers[ch].drain(..take);
        }
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accumulator_two_frames() {
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
        unsafe { f.alloc(f.format(), 2048, f.channel_layout()); }
        let produced = acc.push_frame(&f);
        assert_eq!(produced.len(), 2);
        assert!(acc.flush_tail(true).is_none());
    }
}
