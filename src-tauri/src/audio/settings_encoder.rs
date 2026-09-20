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
    /// Auto-detect best available (FDK > Apple > Native AAC)
    Auto,
    /// FDK HE-AAC VBR (libfdk_aac)
    FdkHeAac,
    /// Apple AAC (AudioToolbox), macOS-only
    AacAt,
    /// Native FFmpeg AAC encoder (aac)
    NativeAac,
    /// Bundled FAAC AAC-LC / HE-AAC v1.
    Faac,
}

impl fmt::Display for EncoderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            EncoderType::Auto => "auto",
            EncoderType::FdkHeAac => "fdk_he_aac",
            EncoderType::AacAt => "aac_at",
            EncoderType::NativeAac => "native_aac",
            EncoderType::Faac => "faac",
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
            BitrateMode::Vbr(_) => Self::Vbr,
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
/// AAC encoders (native `aac`, `aac_at`, `libfdk_aac`) do not frame-thread, so
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
    /// Applies to FDK encoder only
    pub afterburner: bool,
    /// Native NMR search speed, upstream default 0.
    #[serde(default)]
    pub native_aac_speed: u8,
    #[serde(default)]
    pub faac_profile: FaacProfile,
}

/// Valid FDK VBR level range.
pub const VALID_VBR_LEVEL_RANGE: std::ops::RangeInclusive<u16> = 1..=5;

/// Default FDK VBR level for audiobook speech output.
pub const DEFAULT_VBR_LEVEL: u16 = 3;

const ALL_ENCODER_TYPES: [EncoderType; 5] = [
    EncoderType::Auto,
    EncoderType::FdkHeAac,
    EncoderType::AacAt,
    EncoderType::NativeAac,
    EncoderType::Faac,
];
const AUTO_MODES: [BitrateModeKind; 3] = [
    BitrateModeKind::Vbr,
    BitrateModeKind::Cvbr,
    BitrateModeKind::Cbr,
];
const VBR_ONLY: [BitrateModeKind; 1] = [BitrateModeKind::Vbr];
const CVBR_ONLY: [BitrateModeKind; 1] = [BitrateModeKind::Cvbr];
const CBR_ONLY: [BitrateModeKind; 1] = [BitrateModeKind::Cbr];
const FAAC_MODES: [BitrateModeKind; 2] = [BitrateModeKind::Abr, BitrateModeKind::Vbr];

pub const MAX_ENCODER_BITRATE: u16 = 1152;
pub const NATIVE_SPEED_MAX: u8 = 4;

pub fn all_encoder_types() -> [EncoderType; 5] {
    ALL_ENCODER_TYPES
}

pub fn allowed_bitrate_mode_kinds_for(encoder_type: EncoderType) -> &'static [BitrateModeKind] {
    match encoder_type {
        EncoderType::Auto => &AUTO_MODES,
        EncoderType::FdkHeAac => &VBR_ONLY,
        EncoderType::AacAt => &CVBR_ONLY,
        EncoderType::NativeAac => &CBR_ONLY,
        EncoderType::Faac => &FAAC_MODES,
    }
}

pub fn default_bitrate_mode_for(encoder_type: EncoderType) -> BitrateMode {
    match encoder_type {
        EncoderType::Auto | EncoderType::FdkHeAac => BitrateMode::Vbr(DEFAULT_VBR_LEVEL),
        EncoderType::AacAt => BitrateMode::Cvbr,
        EncoderType::NativeAac => BitrateMode::Cbr,
        EncoderType::Faac => BitrateMode::Abr,
    }
}

/// Validates encoder settings (no engine side-effects)
pub fn validate_encoder_settings(settings: &EncoderSettings) -> Result<()> {
    if matches!(
        settings.bitrate_mode,
        BitrateMode::Cbr | BitrateMode::Cvbr | BitrateMode::Abr
    ) && (settings.bitrate_kbps == 0 || settings.bitrate_kbps > MAX_ENCODER_BITRATE)
    {
        return Err(AppError::InvalidInput(format!(
            "Target bitrate must be 1..={MAX_ENCODER_BITRATE} kbps; the encoder, sample rate and channels may lower this ceiling."
        )));
    }
    if matches!(
        settings.encoder_type,
        EncoderType::NativeAac | EncoderType::Auto
    ) && settings.native_aac_speed > NATIVE_SPEED_MAX
    {
        return Err(AppError::InvalidInput("NMR speed must be 0..=4".into()));
    }
    validate_bitrate_mode(settings.encoder_type, settings.bitrate_mode)?;
    validate_encoder_mode_combo(settings.encoder_type, settings.bitrate_mode)?;

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
    let valid = if encoder == EncoderType::Faac {
        FAAC_QUALITY_PRESETS.contains(&quality)
    } else {
        VALID_VBR_LEVEL_RANGE.contains(&quality)
    };
    if valid {
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

/// One-time ffmpeg init to ensure codec discovery works before FFI calls
fn ensure_ffmpeg_initialized() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = ffmpeg_next::init();
    });
}

/// Checks whether an encoder by name is available in the current FFmpeg build
pub fn is_encoder_available_by_name(name: &str) -> bool {
    use std::ffi::CString;
    ensure_ffmpeg_initialized();
    let result = unsafe {
        let c_name = match CString::new(name) {
            Ok(s) => s,
            Err(_) => {
                log::warn!("🔍 Encoder check '{}': invalid C string", name);
                return false;
            }
        };
        let ptr = ffmpeg_next::sys::avcodec_find_encoder_by_name(c_name.as_ptr());
        !ptr.is_null()
    };
    log::debug!(
        "🔍 Encoder check '{}': {}",
        name,
        if result { "FOUND" } else { "NOT FOUND" }
    );
    result
}

/// Probe the required NMR controls on the named encoder's private options.
/// Opening each requested configuration still performs authoritative readback.
pub(crate) fn is_native_nmr_available() -> bool {
    use ffmpeg_next as ff;
    ensure_ffmpeg_initialized();
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

/// Resolves the actual encoder to use based on requested type + availability.
pub fn resolve_encoder_type(
    requested: &EncoderSettings,
    availability: &crate::audio::toolchain::EncoderAvailability,
) -> EncoderType {
    match requested.encoder_type {
        EncoderType::Auto => {
            if availability.fdk_available {
                EncoderType::FdkHeAac
            } else if availability.aac_at_available {
                EncoderType::AacAt
            } else {
                EncoderType::NativeAac
            }
        }
        EncoderType::FdkHeAac if availability.fdk_available => EncoderType::FdkHeAac,
        EncoderType::AacAt if availability.aac_at_available => EncoderType::AacAt,
        EncoderType::NativeAac if availability.native_aac_available => EncoderType::NativeAac,
        EncoderType::Faac => EncoderType::Faac,
        explicit => explicit,
    }
}

pub fn encoder_available(
    requested: EncoderType,
    availability: &crate::audio::toolchain::EncoderAvailability,
) -> bool {
    match requested {
        EncoderType::Auto | EncoderType::Faac => true,
        EncoderType::FdkHeAac => availability.fdk_available,
        EncoderType::AacAt => availability.aac_at_available,
        EncoderType::NativeAac => availability.native_aac_available,
    }
}

pub fn validate_requested_encoder_available(
    requested: EncoderType,
    availability: &crate::audio::toolchain::EncoderAvailability,
) -> Result<()> {
    validate_encoder_available(requested, encoder_available(requested, availability))
}

pub(super) fn validate_encoder_available(requested: EncoderType, available: bool) -> Result<()> {
    if available {
        return Ok(());
    }

    let message = match requested {
        EncoderType::Auto => return Ok(()),
        EncoderType::FdkHeAac => {
            return Err(AppError::toolchain_required(
                "FDK AAC requires a validated external FFmpeg toolchain.",
            ));
        }
        // Platform-truthful: on macOS the encoder exists but this build lacks
        // it; elsewhere AudioToolbox does not exist at all.
        EncoderType::AacAt => {
            if cfg!(target_os = "macos") {
                "Apple AAC is unavailable in this build.".to_string()
            } else {
                "Apple AAC (aac_at) is only available on macOS.".to_string()
            }
        }
        EncoderType::NativeAac => "Native AAC (FFmpeg) is unavailable in this build.".to_string(),
        EncoderType::Faac => unreachable!("FAAC is bundled and always available"),
    };

    Err(AppError::InvalidInput(message))
}

/// Resolves the requested FFmpeg encoder name for the chosen encoder type.
/// This does not open the encoder; it only maps the already-resolved selection.
pub fn resolve_encoder_name(encoder_type: EncoderType) -> &'static str {
    match encoder_type {
        EncoderType::Auto => unreachable!("resolve_encoder_name requires a resolved encoder type"),
        EncoderType::NativeAac => "aac",
        EncoderType::FdkHeAac => "libfdk_aac",
        EncoderType::AacAt => "aac_at",
        EncoderType::Faac => "faac",
    }
}

#[cfg(test)]
mod aac_at_message_tests {
    use super::*;
    use crate::audio::toolchain::{EncoderAvailability, EncoderCapabilitySource};

    fn availability_without_aac_at() -> EncoderAvailability {
        EncoderAvailability {
            fdk_setup_supported: true,
            fdk_available: false,
            fdk_source: EncoderCapabilitySource::None,
            aac_at_available: false,
            native_aac_available: true,
            auto_encoder: EncoderType::NativeAac,
            detected_toolchain_path: None,
            status_message: String::new(),
        }
    }

    /// Per-OS assertion pattern from `processor/streams.rs`: the rejection
    /// message must be truthful about WHY Apple AAC is unavailable.
    #[test]
    fn aac_at_unavailable_message_is_platform_truthful() {
        let err = validate_requested_encoder_available(
            EncoderType::AacAt,
            &availability_without_aac_at(),
        )
        .expect_err("aac_at must be rejected when unavailable");
        let message = err.to_string();

        #[cfg(target_os = "macos")]
        assert!(
            message.contains("Apple AAC is unavailable in this build."),
            "unexpected message: {message}"
        );
        #[cfg(not(target_os = "macos"))]
        assert!(
            message.contains("Apple AAC (aac_at) is only available on macOS."),
            "unexpected message: {message}"
        );
    }
}
