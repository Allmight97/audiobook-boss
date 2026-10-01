//! Encoder settings types and validation
//!
//! This module defines the advanced encoder settings surface used by the
//! processing command, along with validation helpers and encoder selection rules.

use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported encoder types for audiobooks
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EncoderType {
    /// Resolves to the Native NMR encoder.
    Auto,
    /// Apple AAC (AudioToolbox), macOS-only
    AacAt,
    /// Native FFmpeg AAC encoder (aac)
    NativeAac,
    /// Bundled FAAC AAC-LC / HE-AAC v1.
    Faac,
    /// Opus via bundled libopus.
    Opus,
}

impl fmt::Display for EncoderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            EncoderType::Auto => "auto",
            EncoderType::AacAt => "aac_at",
            EncoderType::NativeAac => "native_aac",
            EncoderType::Faac => "faac",
            EncoderType::Opus => "opus",
        };
        write!(f, "{}", label)
    }
}

/// FAAC resolves Auto once from the requested output configuration.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum FaacProfile {
    #[default]
    Auto,
    AacLc,
    HeAacV1,
}

pub const FAAC_QUALITY_PRESETS: &[u16] = &[50, 100, 200];
pub const DEFAULT_FAAC_QUALITY: u16 = 100;

/// Bitrate/quality control mode per encoder
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum BitrateMode {
    Cbr,
    Cvbr,
    Abr,
    Vbr(u16),
    /// Variable bitrate driven by target kbps (Opus).
    VbrTarget,
}

/// Bitrate mode capability without encoder-specific values.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BitrateModeKind {
    Cbr,
    Cvbr,
    Abr,
    Vbr,
}

impl BitrateModeKind {
    fn from_mode(mode: BitrateMode) -> Self {
        match mode {
            BitrateMode::Cbr => Self::Cbr,
            BitrateMode::Cvbr => Self::Cvbr,
            BitrateMode::Abr => Self::Abr,
            BitrateMode::Vbr(_) | BitrateMode::VbrTarget => Self::Vbr,
        }
    }
}

/// Channel selection strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ChannelConfig {
    Auto,
    Mono,
    Stereo,
}

impl ChannelConfig {
    pub fn forced_channels(self) -> Option<u8> {
        match self {
            ChannelConfig::Auto => None,
            ChannelConfig::Mono => Some(1),
            ChannelConfig::Stereo => Some(2),
        }
    }
}

/// Advanced encoder settings payload
///
/// AAC encoders (native `aac`, `aac_at`) do not frame-thread, so
/// there is deliberately no thread setting here; encoding always uses the
/// encoder's single-threaded path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderSettings {
    pub encoder_type: EncoderType,
    /// Target kbps. Native AAC additionally checks the resolved rate/channel ceiling.
    /// Ignored in VBR mode, where the encoder's quality setting owns bitrate.
    pub bitrate_kbps: u16,
    pub bitrate_mode: BitrateMode,
    pub channels: ChannelConfig,
    /// Native NMR search speed, upstream default 0.
    #[serde(default)]
    pub native_aac_speed: u8,
    #[serde(default)]
    pub faac_profile: FaacProfile,
}

impl Default for EncoderSettings {
    fn default() -> Self {
        Self {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps: 65,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Auto,
            native_aac_speed: 0,
            faac_profile: FaacProfile::Auto,
        }
    }
}

impl EncoderSettings {
    pub(in crate::audio) fn resolve_encoder(&mut self, encoder: EncoderType) {
        if self.encoder_type == EncoderType::Auto
            && !allowed_bitrate_mode_kinds_for(encoder)
                .contains(&BitrateModeKind::from_mode(self.bitrate_mode))
        {
            self.bitrate_mode = default_bitrate_mode_for(encoder);
        }
        self.encoder_type = encoder;
    }
}

pub const ALL_ENCODER_TYPES: [EncoderType; 5] = [
    EncoderType::Auto,
    EncoderType::AacAt,
    EncoderType::NativeAac,
    EncoderType::Faac,
    EncoderType::Opus,
];

const MAX_ENCODER_BITRATE: u16 = 1152;
pub const NATIVE_SPEED_MAX: u8 = 4;

/// Target bitrate bounds before rate/channel ceilings; capabilities and validation share them.
pub fn encoder_bitrate_range(encoder: EncoderType) -> std::ops::RangeInclusive<u16> {
    if encoder == EncoderType::Opus {
        6..=510
    } else {
        1..=MAX_ENCODER_BITRATE
    }
}

pub fn allowed_bitrate_mode_kinds_for(encoder_type: EncoderType) -> &'static [BitrateModeKind] {
    match encoder_type {
        EncoderType::Opus => &[BitrateModeKind::Vbr],
        EncoderType::AacAt => &[BitrateModeKind::Cvbr],
        EncoderType::Auto | EncoderType::NativeAac => &[BitrateModeKind::Cbr],
        EncoderType::Faac => &[BitrateModeKind::Abr, BitrateModeKind::Vbr],
    }
}

pub fn default_bitrate_mode_for(encoder_type: EncoderType) -> BitrateMode {
    match encoder_type {
        EncoderType::Auto | EncoderType::NativeAac => BitrateMode::Cbr,
        EncoderType::AacAt => BitrateMode::Cvbr,
        EncoderType::Faac => BitrateMode::Abr,
        EncoderType::Opus => BitrateMode::VbrTarget,
    }
}

/// Validates encoder settings (no engine side-effects)
pub fn validate_encoder_settings(settings: &EncoderSettings) -> Result<()> {
    if (settings.encoder_type == EncoderType::Opus)
        != (settings.bitrate_mode == BitrateMode::VbrTarget)
    {
        return Err(AppError::InvalidInput(
            "Opus uses a VBR target bitrate.".into(),
        ));
    }
    // VBR quality owns bitrate; every other mode takes a target.
    let bitrate_range = encoder_bitrate_range(settings.encoder_type);
    if !matches!(settings.bitrate_mode, BitrateMode::Vbr(_))
        && !bitrate_range.contains(&settings.bitrate_kbps)
    {
        return Err(AppError::InvalidInput(format!(
            "Target bitrate must be {}..={} kbps; the encoder, sample rate and channels may lower this ceiling.",
            bitrate_range.start(),
            bitrate_range.end()
        )));
    }
    if matches!(
        settings.encoder_type,
        EncoderType::NativeAac | EncoderType::Auto
    ) && settings.native_aac_speed > NATIVE_SPEED_MAX
    {
        return Err(AppError::InvalidInput(format!(
            "NMR speed must be 0..={NATIVE_SPEED_MAX}"
        )));
    }
    validate_encoder_mode_combo(settings.encoder_type, settings.bitrate_mode)?;
    validate_bitrate_mode(settings.encoder_type, settings.bitrate_mode)?;

    Ok(())
}

pub(super) fn validate_native_target_bitrate(
    bitrate_kbps: u16,
    sample_rate: u32,
    channels: u32,
) -> Result<()> {
    let max_bps = 6 * u64::from(sample_rate) * u64::from(channels);
    if u64::from(bitrate_kbps) * 1000 > max_bps {
        return Err(AppError::InvalidInput(format!(
            "Native target bitrate exceeds {} kbps at {sample_rate} Hz / {channels} channel(s)",
            max_bps / 1000
        )));
    }
    Ok(())
}

fn validate_bitrate_mode(encoder: EncoderType, mode: BitrateMode) -> Result<()> {
    let BitrateMode::Vbr(quality) = mode else {
        return Ok(());
    };
    if encoder == EncoderType::Faac && FAAC_QUALITY_PRESETS.contains(&quality) {
        return Ok(());
    }
    Err(AppError::InvalidInput(format!(
        "Unsupported VBR quality {quality} for {encoder}."
    )))
}

fn validate_encoder_mode_combo(encoder_type: EncoderType, mode: BitrateMode) -> Result<()> {
    let allowed =
        allowed_bitrate_mode_kinds_for(encoder_type).contains(&BitrateModeKind::from_mode(mode));
    if allowed {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "Bitrate mode {:?} is not supported for encoder {:?}",
            mode, encoder_type
        )))
    }
}

/// Probe the required NMR controls on the named encoder's private options.
/// Opening each requested configuration still performs authoritative readback.
fn is_native_nmr_available() -> bool {
    use ffmpeg_next as ff;
    let Some(codec) = ff::encoder::find_by_name("aac") else {
        return false;
    };
    let mut ctx = ff::codec::context::Context::new_with_codec(codec);
    // SAFETY: ctx owns its private options; C literals are NUL-terminated.
    unsafe {
        if ctx.as_mut_ptr().is_null() {
            return false;
        }
        ff::ffi::av_opt_set(
            ctx.as_mut_ptr().cast(),
            c"aac_coder".as_ptr(),
            c"nmr".as_ptr(),
            ff::ffi::AV_OPT_SEARCH_CHILDREN,
        ) >= 0
            && ff::ffi::av_opt_set_int(
                ctx.as_mut_ptr().cast(),
                c"aac_nmr_speed".as_ptr(),
                4,
                ff::ffi::AV_OPT_SEARCH_CHILDREN,
            ) >= 0
    }
}

pub(super) fn linked_encoder_available(encoder: EncoderType) -> bool {
    match encoder {
        EncoderType::NativeAac => is_native_nmr_available(),
        EncoderType::AacAt => {
            cfg!(target_os = "macos") && ffmpeg_next::encoder::find_by_name("aac_at").is_some()
        }
        EncoderType::Faac => true,
        EncoderType::Opus => ffmpeg_next::encoder::find_by_name("libopus").is_some(),
        EncoderType::Auto => false,
    }
}

/// The encoder Auto resolves to.
pub(super) const AUTO_ENCODER: EncoderType = EncoderType::NativeAac;

/// Rejects an unavailable linked encoder (Native, Apple, Opus).
pub(super) fn validate_encoder_available(requested: EncoderType, available: bool) -> Result<()> {
    if available {
        return Ok(());
    }
    let message = match requested {
        // Platform-truthful: on macOS the encoder exists but this build lacks
        // it; elsewhere AudioToolbox does not exist at all.
        EncoderType::AacAt if cfg!(target_os = "macos") => {
            "Apple AAC is unavailable in this build."
        }
        EncoderType::AacAt => "Apple AAC (aac_at) is only available on macOS.",
        EncoderType::NativeAac => "Native AAC (FFmpeg) is unavailable in this build.",
        EncoderType::Opus => "Opus is unavailable in this build.",
        EncoderType::Auto | EncoderType::Faac => {
            unreachable!("{requested} is not a linked encoder")
        }
    };
    Err(AppError::InvalidInput(message.into()))
}

/// Resolves the requested FFmpeg encoder name for the chosen encoder type.
/// This does not open the encoder; it only maps the already-resolved selection.
pub fn resolve_encoder_name(encoder_type: EncoderType) -> &'static str {
    match encoder_type {
        EncoderType::Auto => unreachable!("resolve_encoder_name requires a resolved encoder type"),
        EncoderType::NativeAac => "aac",
        EncoderType::AacAt => "aac_at",
        EncoderType::Faac => "faac",
        EncoderType::Opus => "libopus",
    }
}
