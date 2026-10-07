//! The decoded sample interval of HE-AAC MP4 files: ABB's own FAAC files,
//! recognized by their encoding-tool tag, and third-party files that declare
//! iTunSMPB gapless info (Apple's encoder and the `faac` frontend measured).
//!
//! Neither convention counts the HE decoder's SBR delay in its priming.
//! FFmpeg's native HE decoder emits that delay untrimmed, so its window adds
//! it; FAAD3 trims its reported delay itself, so its window holds priming
//! alone. Both read every access unit: an edit list can drop the final unit,
//! which holds the delayed tail.

use crate::errors::{AppError, Result};
use ff::{packet::Mut, Rescale};
use ffmpeg_next as ff;

pub(super) const ENCODING_TOOL: &str = "AudioBook Boss FAAC HE-AAC timing-2";
pub(super) const CORE_PRIMING: i64 = 2080;
pub(super) const SBR_DELAY: i64 = 962;

fn core_priming(tool: Option<&str>) -> Option<i64> {
    match tool {
        Some(ENCODING_TOOL) => Some(CORE_PRIMING),
        // Files written before upstream's one-sample HE input padding.
        Some("AudioBook Boss FAAC HE-AAC") => Some(2079),
        _ => None,
    }
}

/// Who removes the HE decoder's SBR delay from the decoded PCM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SbrDelayOwner {
    /// This window (FFmpeg's native decoder emits the delay untrimmed).
    Window,
    /// The decoder (FAAD3 reports and trims its own delay).
    Decoder,
}

/// Whether this is an ABB-produced FAAC HE file, whose timing only ABB knows.
pub(super) fn is_abb_faac_he(input: &ff::format::context::Input) -> bool {
    core_priming(input.metadata().get("encoder")).is_some()
}

/// iTunSMPB priming and playable length, in the track's time base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Gapless {
    priming: i64,
    samples: i64,
}

/// Reads iTunSMPB as FFmpeg's MP4 reader does: hex fields, priming in
/// 1..16384, a positive length.
fn itunes_gapless(value: &str) -> Option<Gapless> {
    let fields: Vec<&str> = value.split_whitespace().collect();
    let priming = i64::from_str_radix(fields.get(1)?, 16).ok()?;
    let samples = i64::from_str_radix(fields.get(3)?, 16).ok()?;
    (0 < priming && priming < 16384 && samples > 0).then_some(Gapless { priming, samples })
}

/// The best audio stream's time base, output rate, and duration.
struct AudioClock {
    time_base: ff::Rational,
    rate: i32,
    duration: i64,
}

fn audio_clock(input: &ff::format::context::Input) -> Result<AudioClock> {
    let stream = input
        .streams()
        .best(ff::media::Type::Audio)
        .ok_or_else(|| AppError::InvalidInput("HE-AAC file has no audio stream.".into()))?;
    let parameters = stream.parameters();
    // SAFETY: copy the sample rate while the stream's parameters are borrowed.
    let rate = unsafe { (*parameters.as_ptr()).sample_rate };
    if parameters.id() != ff::codec::Id::AAC || rate <= 0 || stream.duration() <= 0 {
        return Err(AppError::InvalidInput(
            "HE-AAC file has no valid playable sample interval.".into(),
        ));
    }
    Ok(AudioClock {
        time_base: stream.time_base(),
        rate,
        duration: stream.duration(),
    })
}

#[derive(Clone, Copy, Debug)]
pub(super) struct HeDecodeWindow {
    pub start_sample: i64,
    pub end_sample: i64,
    pub rate: u32,
}

impl HeDecodeWindow {
    /// The window for ABB's FAAC HE files, or for an HE-AAC file (`he`) that
    /// declares iTunSMPB. Any other input keeps FFmpeg's own trimming.
    pub fn from_input(
        input: &ff::format::context::Input,
        he: bool,
        sbr_delay_owner: SbrDelayOwner,
    ) -> Result<Option<Self>> {
        if let Some(priming) = core_priming(input.metadata().get("encoder")) {
            return Self::abb_faac(input, priming, sbr_delay_owner).map(Some);
        }
        if !he {
            return Ok(None);
        }
        let metadata = input.metadata();
        let Some(value) = metadata.get("iTunSMPB") else {
            return Ok(None);
        };
        let Some(gapless) = itunes_gapless(value) else {
            log::warn!("he_gapless status=ignored reason=unreadable_itunsmpb");
            return Ok(None);
        };
        Self::itunes(input, gapless, sbr_delay_owner).map(Some)
    }

    fn itunes(
        input: &ff::format::context::Input,
        gapless: Gapless,
        sbr_delay_owner: SbrDelayOwner,
    ) -> Result<Self> {
        let clock = audio_clock(input)?;
        let to_output = |ticks: i64| ticks.rescale(clock.time_base, ff::Rational(1, clock.rate));
        let window = Self::new(
            to_output(gapless.priming),
            to_output(gapless.samples),
            clock.rate,
            sbr_delay_owner,
        )?;
        log::info!(
            "he_gapless status=applied source=itunsmpb start_sample={} end_sample={} sbr_delay_owner={sbr_delay_owner:?}",
            window.start_sample,
            window.end_sample
        );
        Ok(window)
    }

    fn abb_faac(
        input: &ff::format::context::Input,
        priming: i64,
        sbr_delay_owner: SbrDelayOwner,
    ) -> Result<Self> {
        let clock = audio_clock(input)?;
        let samples = clock
            .duration
            .rescale(clock.time_base, ff::Rational(1, clock.rate));
        Self::new(priming, samples, clock.rate, sbr_delay_owner)
    }

    fn new(priming: i64, samples: i64, rate: i32, sbr_delay_owner: SbrDelayOwner) -> Result<Self> {
        let start_sample = match sbr_delay_owner {
            SbrDelayOwner::Window => priming + SBR_DELAY,
            SbrDelayOwner::Decoder => priming,
        };
        let end_sample = start_sample
            .checked_add(samples)
            .filter(|end| *end > start_sample)
            .ok_or_else(|| {
                AppError::InvalidInput("HE-AAC playable interval is out of range.".into())
            })?;
        Ok(Self {
            start_sample,
            end_sample,
            rate: rate as u32,
        })
    }

    /// Packet skip metadata lets the decoder own frame slicing before
    /// resampling. It replaces the skip metadata FFmpeg derived from iTunSMPB.
    pub fn apply(self, packet: &mut ff::Packet, time_base: ff::Rational) -> Result<()> {
        let samples = ff::Rational(1, self.rate as i32);
        let start = packet
            .pts()
            .ok_or_else(|| AppError::InvalidInput("HE-AAC packet has no timestamp.".into()))?
            .rescale(time_base, samples);
        let length = packet.duration().rescale(time_base, samples);
        if start < 0 || length != 2048 || start > i64::MAX - length {
            return Err(AppError::InvalidInput(
                "HE-AAC packet has an invalid access-unit interval.".into(),
            ));
        }
        let leading = (self.start_sample - start).clamp(0, length) as u32;
        let trailing = (start + length - self.end_sample).clamp(0, length) as u32;
        let has_skip = packet
            .side_data()
            .any(|side| side.kind() == ff::codec::packet::side_data::Type::SkipSamples);
        if leading == 0 && trailing == 0 && !has_skip {
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
                    "Cannot allocate HE-AAC trim metadata.".into(),
                ));
            }
            std::ptr::write_bytes(data, 0, 10);
            std::ptr::copy_nonoverlapping(leading.to_le_bytes().as_ptr(), data, 4);
            std::ptr::copy_nonoverlapping(trailing.to_le_bytes().as_ptr(), data.add(4), 4);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_provenance_preserves_old_he_timing_and_excludes_other_encoders() {
        for (tool, expected) in [
            (Some("AudioBook Boss FAAC HE-AAC"), Some(2079)),
            (Some("AudioBook Boss FAAC HE-AAC timing-2"), Some(2080)),
            (Some("AudioBook Boss FAAC AAC-LC"), None),
            (Some("FAAC"), None),
            (None, None),
        ] {
            assert_eq!(core_priming(tool), expected);
        }
    }

    #[test]
    fn itunsmpb_reads_priming_and_length_like_ffmpeg() {
        let apple = " 00000000 00000840 00000518 000000000006BAA8 00000000 00000000";
        assert_eq!(
            itunes_gapless(apple),
            Some(Gapless {
                priming: 0x840,
                samples: 0x6BAA8
            })
        );
        for unusable in [
            "",
            "garbage",
            " 00000000 00000000 00000518 000000000006BAA8",
            " 00000000 00004000 0 1",
        ] {
            assert_eq!(itunes_gapless(unusable), None, "{unusable:?}");
        }
    }
}
