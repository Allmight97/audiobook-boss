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
}

impl fmt::Display for EncoderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            EncoderType::Auto => "auto",
            EncoderType::FdkHeAac => "fdk_he_aac",
            EncoderType::AacAt => "aac_at",
            EncoderType::NativeAac => "native_aac",
        };
        write!(f, "{}", label)
    }
}

/// Bitrate/quality control mode per encoder
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum BitrateMode {
    Cbr,
    Cvbr,
    Vbr(u8),
}

/// Bitrate mode capability without encoder-specific values.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BitrateModeKind {
    Cbr,
    Cvbr,
    Vbr,
}

impl BitrateModeKind {
    fn from_mode(mode: BitrateMode) -> Self {
        match mode {
            BitrateMode::Cbr => Self::Cbr,
            BitrateMode::Cvbr => Self::Cvbr,
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
    /// Allowed: 48|56|64|72|80|88|96|104|112|120|128 (kbps).
    /// Ignored by VBR-only encoders (FDK): the VBR level owns bitrate there.
    pub bitrate_kbps: u16,
    pub bitrate_mode: BitrateMode,
    pub channels: ChannelConfig,
    /// Applies to FDK encoder only
    pub afterburner: bool,
}

/// Whitelist of supported encoder bitrates for speech-oriented output
pub const VALID_ENCODER_BITRATES: &[u16] = &[48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128];

/// Valid VBR level range for encoders that support VBR.
pub const VALID_VBR_LEVEL_RANGE: std::ops::RangeInclusive<u8> = 1..=5;

/// Default VBR level for audiobook speech output.
pub const DEFAULT_VBR_LEVEL: u8 = 3;

const ALL_ENCODER_TYPES: [EncoderType; 4] = [
    EncoderType::Auto,
    EncoderType::FdkHeAac,
    EncoderType::AacAt,
    EncoderType::NativeAac,
];
const AUTO_ENCODER_RESOLUTION_ORDER: [EncoderType; 3] = [
    EncoderType::FdkHeAac,
    EncoderType::AacAt,
    EncoderType::NativeAac,
];
const VBR_ONLY: [BitrateModeKind; 1] = [BitrateModeKind::Vbr];
const CVBR_ONLY: [BitrateModeKind; 1] = [BitrateModeKind::Cvbr];
const CBR_ONLY: [BitrateModeKind; 1] = [BitrateModeKind::Cbr];

pub fn all_encoder_types() -> [EncoderType; 4] {
    ALL_ENCODER_TYPES
}

pub fn auto_encoder_resolution_order() -> [EncoderType; 3] {
    AUTO_ENCODER_RESOLUTION_ORDER
}

pub fn allowed_bitrate_mode_kinds_for(encoder_type: EncoderType) -> &'static [BitrateModeKind] {
    match encoder_type {
        EncoderType::Auto | EncoderType::FdkHeAac => &VBR_ONLY,
        EncoderType::AacAt => &CVBR_ONLY,
        EncoderType::NativeAac => &CBR_ONLY,
    }
}

pub fn default_bitrate_mode_for(encoder_type: EncoderType) -> BitrateMode {
    match encoder_type {
        EncoderType::Auto | EncoderType::FdkHeAac => BitrateMode::Vbr(DEFAULT_VBR_LEVEL),
        EncoderType::AacAt => BitrateMode::Cvbr,
        EncoderType::NativeAac => BitrateMode::Cbr,
    }
}

/// Validates encoder settings (no engine side-effects)
pub fn validate_encoder_settings(settings: &EncoderSettings) -> Result<()> {
    validate_bitrate(settings.bitrate_kbps)?;
    validate_bitrate_mode(settings.bitrate_mode)?;
    validate_encoder_mode_combo(settings.encoder_type, settings.bitrate_mode)?;

    Ok(())
}

fn validate_bitrate(bitrate_kbps: u16) -> Result<()> {
    if VALID_ENCODER_BITRATES.contains(&bitrate_kbps) {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "Unsupported bitrate_kbps: {}. Valid: {:?}",
            bitrate_kbps, VALID_ENCODER_BITRATES
        )))
    }
}

fn validate_bitrate_mode(mode: BitrateMode) -> Result<()> {
    match mode {
        BitrateMode::Cbr | BitrateMode::Cvbr => Ok(()),
        BitrateMode::Vbr(level) if VALID_VBR_LEVEL_RANGE.contains(&level) => Ok(()),
        BitrateMode::Vbr(level) => Err(AppError::InvalidInput(format!(
            "Unsupported VBR level: {} (allowed 1..=5)",
            level
        ))),
    }
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
        explicit => explicit,
    }
}

pub fn encoder_available(
    requested: EncoderType,
    availability: &crate::audio::toolchain::EncoderAvailability,
) -> bool {
    match requested {
        EncoderType::Auto => true,
        EncoderType::FdkHeAac => availability.fdk_available,
        EncoderType::AacAt => availability.aac_at_available,
        EncoderType::NativeAac => availability.native_aac_available,
    }
}

pub fn validate_requested_encoder_available(
    requested: EncoderType,
    availability: &crate::audio::toolchain::EncoderAvailability,
) -> Result<()> {
    if encoder_available(requested, availability) {
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
    }
}

#[cfg(test)]
mod aac_at_message_tests {
    use super::*;
    use crate::audio::toolchain::{EncoderAvailability, EncoderCapabilitySource};

    fn availability_without_aac_at() -> EncoderAvailability {
        EncoderAvailability {
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
