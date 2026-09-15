//! The encoded and decoded sample intervals of ABB-produced FAAC HE files.
//!
//! Apple MP4 playback uses core priming. FFmpeg's native HE decoder also emits
//! SBR reconstruction delay; re-import reads all access units and applies the
//! corresponding PCM interval. The tool tag limits this policy to our files.

use crate::errors::{AppError, Result};
use ff::{packet::Mut, Rescale};
use ffmpeg_next as ff;
use std::path::Path;

pub(super) const ENCODING_TOOL: &str = "AudioBook Boss FAAC HE-AAC";
pub(super) const CORE_PRIMING: i64 = 2079;
pub(super) const SBR_DELAY: i64 = 962;

#[derive(Clone, Copy, Debug)]
pub(super) struct FaacDecodeWindow {
    pub start_sample: i64,
    pub end_sample: i64,
    pub rate: u32,
}

impl FaacDecodeWindow {
    pub fn from_input(input: &ff::format::context::Input) -> Result<Option<Self>> {
        if input.metadata().get("encoder") != Some(ENCODING_TOOL) {
            return Ok(None);
        }
        let stream = input
            .streams()
            .best(ff::media::Type::Audio)
            .ok_or_else(|| AppError::InvalidInput("FAAC file has no audio stream.".into()))?;
        let parameters = stream.parameters();
        // SAFETY: copy the sample rate while the stream's parameters are borrowed.
        let rate = unsafe { (*parameters.as_ptr()).sample_rate };
        if parameters.id() != ff::codec::Id::AAC || rate <= 0 || stream.duration() <= 0 {
            return Err(AppError::InvalidInput(
                "FAAC file has no valid playable sample interval.".into(),
            ));
        }
        let samples = stream
            .duration()
            .rescale(stream.time_base(), ff::Rational(1, rate));
        let start_sample = CORE_PRIMING + SBR_DELAY;
        let end_sample = start_sample
            .checked_add(samples)
            .filter(|end| *end > start_sample)
            .ok_or_else(|| {
                AppError::InvalidInput("FAAC playable interval is out of range.".into())
            })?;
        Ok(Some(Self {
            start_sample,
            end_sample,
            rate: rate as u32,
        }))
    }

    /// Packet skip metadata lets the native decoder own frame slicing before resampling.
    pub fn apply(self, packet: &mut ff::Packet, time_base: ff::Rational) -> Result<()> {
        let samples = ff::Rational(1, self.rate as i32);
        let start = packet
            .pts()
            .ok_or_else(|| AppError::InvalidInput("FAAC packet has no timestamp.".into()))?
            .rescale(time_base, samples);
        let length = packet.duration().rescale(time_base, samples);
        if start < 0 || length != 2048 || start > i64::MAX - length {
            return Err(AppError::InvalidInput(
                "FAAC packet has an invalid access-unit interval.".into(),
            ));
        }
        let leading = (self.start_sample - start).clamp(0, length) as u32;
        let trailing = (start + length - self.end_sample).clamp(0, length) as u32;
        if leading == 0 && trailing == 0 {
            return Ok(());
        }
        // SAFETY: packet owns the initialized ten-byte skip-sample payload.
        // Counts are bounded by this AAC access unit's duration.
        unsafe {
            let data = ff::sys::av_packet_new_side_data(
                packet.as_mut_ptr(),
                ff::sys::AVPacketSideDataType::AV_PKT_DATA_SKIP_SAMPLES,
                10,
            );
            if data.is_null() {
                return Err(AppError::General(
                    "Cannot allocate FAAC trim metadata.".into(),
                ));
            }
            std::ptr::write_bytes(data, 0, 10);
            std::ptr::copy_nonoverlapping(leading.to_le_bytes().as_ptr(), data, 4);
            std::ptr::copy_nonoverlapping(trailing.to_le_bytes().as_ptr(), data.add(4), 4);
        }
        Ok(())
    }
}

pub(super) fn inspect(path: &Path) -> Result<Option<FaacDecodeWindow>> {
    let input = ff::format::input(path).map_err(|error| {
        AppError::General(format!(
            "Cannot inspect input timing for '{}': {error}",
            crate::errors::sanitize_path_for_display(path)
        ))
    })?;
    FaacDecodeWindow::from_input(&input)
}
