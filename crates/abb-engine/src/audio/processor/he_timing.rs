//! The decoded sample interval of HE-AAC MP4 files: ABB's own FAAC files,
//! recognized by their encoding-tool tag, and third-party files that declare
//! iTunSMPB gapless info (Apple's encoder and the `faac` frontend measured).
//!
//! Neither convention counts the HE decoder's SBR delay in its priming.
//! FFmpeg's native HE decoder emits that delay untrimmed, so the window adds
//! it. FAAD3 trims its reported delay itself, so its window holds priming
//! alone. Both read every access unit: an edit list can drop the final unit,
//! which holds the delayed tail.

use crate::errors::{AppError, Result};
use abb_ffmpeg::{CodecId, Input, Packet, Rational, SkipSamples};

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

fn tag(input: &Input, key: &str) -> Option<String> {
    input.tags().get(key).map(abb_ffmpeg::Text::display)
}

/// Whether a container `encoder` tag names ABB's FAAC HE-AAC output.
pub(super) fn is_abb_faac_he(encoder_tag: Option<&str>) -> bool {
    core_priming(encoder_tag).is_some()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SbrDelayOwner {
    Window,
    Decoder,
}

/// iTunSMPB priming, playable length, and padding, in the track's time base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Gapless {
    priming: i64,
    samples: i64,
    remainder: i64,
}

/// Reads iTunSMPB as FFmpeg's MP4 reader does: four hex fields of at most 16
/// digits, counts up to 2^40, priming in 1..16384, a positive length.
fn itunes_gapless(value: &str) -> Option<Gapless> {
    let fields = value
        .split_whitespace()
        .take(4)
        .map(|field| (field.len() <= 16).then(|| u64::from_str_radix(field, 16).ok())?)
        .collect::<Option<Vec<u64>>>()?;
    let [_, priming, remainder, samples] = fields[..] else {
        return None;
    };
    if [priming, remainder, samples]
        .iter()
        .any(|count| *count > 1 << 40)
    {
        return None;
    }
    let (priming, samples, remainder) = (priming as i64, samples as i64, remainder as i64);
    (0 < priming && priming < 16384 && samples > 0).then_some(Gapless {
        priming,
        samples,
        remainder,
    })
}

fn agrees_with_duration(samples: i64, coded: i64, duration: i64) -> bool {
    [samples, coded]
        .iter()
        .any(|len| (duration - len).abs() <= 2048)
}

struct AudioClock {
    time_base: Rational,
    rate: i32,
    duration: i64,
}

fn audio_clock(input: &Input) -> Result<AudioClock> {
    let stream = input
        .best_audio_stream()
        .and_then(|index| input.streams().into_iter().nth(index))
        .ok_or_else(|| AppError::InvalidInput("HE-AAC file has no audio stream.".into()))?;
    let rate = stream.sample_rate;
    if stream.codec != CodecId::AAC || rate <= 0 {
        return Err(AppError::InvalidInput(
            "HE-AAC file has no valid playable sample interval.".into(),
        ));
    }
    Ok(AudioClock {
        time_base: stream.time_base,
        rate,
        duration: stream.duration.unwrap_or(i64::MIN),
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
        input: &Input,
        he: bool,
        sbr_delay_owner: SbrDelayOwner,
    ) -> Result<Option<Self>> {
        if let Some(priming) = core_priming(tag(input, "encoder").as_deref()) {
            return Self::abb_faac(input, priming, sbr_delay_owner).map(Some);
        }
        if !he {
            return Ok(None);
        }
        let Some(value) = tag(input, "iTunSMPB") else {
            return Ok(None);
        };
        let Some(gapless) = itunes_gapless(&value) else {
            log::warn!("he_gapless status=ignored reason=unreadable_itunsmpb");
            return Ok(None);
        };
        Self::itunes(input, gapless, sbr_delay_owner)
    }

    fn itunes(
        input: &Input,
        gapless: Gapless,
        sbr_delay_owner: SbrDelayOwner,
    ) -> Result<Option<Self>> {
        let clock = audio_clock(input)?;
        let to_output =
            |ticks: i64| Rational::rescale(ticks, clock.time_base, Rational::new(1, clock.rate));
        let (priming, samples) = (to_output(gapless.priming), to_output(gapless.samples));
        let coded = priming + samples + to_output(gapless.remainder);
        if clock.duration <= 0 || !agrees_with_duration(samples, coded, to_output(clock.duration)) {
            log::warn!("he_gapless status=ignored reason=itunsmpb_disagrees_with_duration");
            return Ok(None);
        }
        let window = Self::new(priming, samples, clock.rate, sbr_delay_owner)?;
        log::info!(
            "he_gapless status=applied source=itunsmpb start_sample={} end_sample={} sbr_delay_owner={sbr_delay_owner:?}",
            window.start_sample,
            window.end_sample
        );
        Ok(Some(window))
    }

    fn abb_faac(input: &Input, priming: i64, sbr_delay_owner: SbrDelayOwner) -> Result<Self> {
        let clock = audio_clock(input)?;
        if clock.duration <= 0 {
            return Err(AppError::InvalidInput(
                "HE-AAC file has no valid playable sample interval.".into(),
            ));
        }
        let samples = Rational::rescale(
            clock.duration,
            clock.time_base,
            Rational::new(1, clock.rate),
        );
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
    pub fn apply(self, packet: &mut Packet, time_base: Rational) -> Result<()> {
        let samples = Rational::new(1, self.rate as i32);
        let pts = packet
            .pts()
            .ok_or_else(|| AppError::InvalidInput("HE-AAC packet has no timestamp.".into()))?;
        let start = Rational::rescale(pts, time_base, samples);
        let length = Rational::rescale(packet.duration(), time_base, samples);
        if start < 0 || length <= 0 || start > i64::MAX - length {
            return Err(AppError::InvalidInput(
                "HE-AAC packet has an invalid access-unit interval.".into(),
            ));
        }
        let leading = (self.start_sample - start).clamp(0, length) as u32;
        let trailing = (start + length - self.end_sample).clamp(0, length) as u32;
        let has_skip = !matches!(packet.skip_samples(), Ok(None));
        if leading == 0 && trailing == 0 && !has_skip {
            return Ok(());
        }
        packet
            .set_skip_samples(SkipSamples {
                start: leading,
                end: trailing,
                ..SkipSamples::default()
            })
            .map_err(|_| AppError::General("Cannot allocate HE-AAC trim metadata.".into()))?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "he_timing_tests.rs"]
mod tests;
