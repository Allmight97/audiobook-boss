//! Encoder v2 settings types and validation (Phase 2 — shrink.sh lift)
//!
//! This module defines the advanced encoder settings surface used by the
//! v2 command, along with validation helpers and encoder availability probes.

use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported encoder types for audiobooks
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum BitrateMode {
    Cbr,
    Cvbr,
    Vbr(u8),
}

/// Channel selection strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

/// Threading configuration for encoder
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum ThreadSetting {
    /// Let FFmpeg decide (maps to threads=0)
    Auto,
    /// Disable threading (maps to threads=1)
    Off,
    /// Fixed number of threads
    Fixed(u16),
}

/// Advanced encoder settings payload for v2 command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EncoderSettings {
    pub encoder_type: EncoderType,
    /// Allowed: 48|56|64|72|80|88|96|112|128 (kbps)
    pub bitrate_kbps: u16,
    pub bitrate_mode: BitrateMode,
    pub channels: ChannelConfig,
    /// Applies to FDK encoder only
    pub afterburner: bool,
    pub threads: ThreadSetting,
}

/// Whitelist of supported encoder bitrates for speech-oriented output
pub const VALID_ENCODER_BITRATES: &[u16] = &[48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128];

/// Valid range for thread count when using Fixed thread setting
pub const VALID_THREAD_COUNT_RANGE: std::ops::RangeInclusive<u16> = 1..=1024;

/// Validates encoder settings (no engine side-effects)
pub fn validate_encoder_settings(settings: &EncoderSettings) -> Result<()> {
    validate_bitrate(settings.bitrate_kbps)?;
    validate_bitrate_mode(settings.bitrate_mode)?;
    validate_encoder_mode_combo(settings.encoder_type, settings.bitrate_mode)?;
    validate_threads(settings.threads)?;

    if settings.afterburner && !is_encoder_available_by_name("libfdk_aac") {
        log::info!("afterburner flag present but libfdk_aac unavailable - will be ignored");
    }

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
        BitrateMode::Vbr(level) if (1..=5).contains(&level) => Ok(()),
        BitrateMode::Vbr(level) => Err(AppError::InvalidInput(format!(
            "Unsupported VBR level: {} (allowed 1..=5)",
            level
        ))),
    }
}

fn validate_encoder_mode_combo(encoder_type: EncoderType, mode: BitrateMode) -> Result<()> {
    let allowed = match encoder_type {
        EncoderType::Auto => matches!(mode, BitrateMode::Vbr(_)),
        EncoderType::FdkHeAac => matches!(mode, BitrateMode::Vbr(_)),
        EncoderType::AacAt => matches!(mode, BitrateMode::Cvbr),
        EncoderType::NativeAac => matches!(mode, BitrateMode::Cbr),
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "Bitrate mode {:?} is not supported for encoder {:?}",
            mode, encoder_type
        )))
    }
}

fn validate_threads(setting: ThreadSetting) -> Result<()> {
    match setting {
        ThreadSetting::Auto | ThreadSetting::Off => Ok(()),
        ThreadSetting::Fixed(n) if VALID_THREAD_COUNT_RANGE.contains(&n) => Ok(()),
        ThreadSetting::Fixed(n) => Err(AppError::InvalidInput(format!(
            "Invalid threads value: {} (allowed {}..={})",
            n,
            VALID_THREAD_COUNT_RANGE.start(),
            VALID_THREAD_COUNT_RANGE.end()
        ))),
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
    unsafe {
        let c_name = match CString::new(name) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let ptr = ffmpeg_next::sys::avcodec_find_encoder_by_name(c_name.as_ptr());
        !ptr.is_null()
    }
}

/// Runtime detection of available encoders.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EncoderAvailability {
    pub fdk_available: bool,
    pub aac_at_available: bool,
    pub native_aac_available: bool,
}

pub fn detect_available_encoders() -> EncoderAvailability {
    EncoderAvailability {
        fdk_available: is_encoder_available_by_name("libfdk_aac"),
        aac_at_available: cfg!(target_os = "macos") && is_encoder_available_by_name("aac_at"),
        native_aac_available: is_encoder_available_by_name("aac"),
    }
}

/// Resolves the actual encoder to use based on requested type + availability.
pub fn resolve_encoder_type(
    requested: &EncoderSettings,
    availability: &EncoderAvailability,
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
        fallback => {
            log::warn!(
                "encoder fallback: requested={:?} availability={:?}",
                fallback,
                availability
            );
            if availability.native_aac_available {
                EncoderType::NativeAac
            } else {
                fallback
            }
        }
    }
}

/// Resolves the requested encoder name, falling back to native `aac` when unavailable.
/// This does not open the encoder; it only chooses the preferred name.
pub fn resolve_encoder_name(encoder_type: EncoderType) -> &'static str {
    match encoder_type {
        EncoderType::Auto | EncoderType::NativeAac => "aac",
        EncoderType::FdkHeAac => "libfdk_aac",
        EncoderType::AacAt => "aac_at",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
        }
    }

    #[test]
    fn test_validate_bitrate_whitelist() {
        for &br in VALID_ENCODER_BITRATES {
            let mut s = base_settings();
            s.bitrate_kbps = br;
            assert!(validate_encoder_settings(&s).is_ok());
        }
        // Test invalid bitrates (outside expanded 48-128 range)
        let mut s = base_settings();
        s.bitrate_kbps = 32; // below minimum
        assert!(validate_encoder_settings(&s).is_err());

        s.bitrate_kbps = 192; // above maximum
        assert!(validate_encoder_settings(&s).is_err());
    }

    #[test]
    fn test_bitrate_mode_validation() {
        let mut s = base_settings();
        s.bitrate_mode = BitrateMode::Vbr(5);
        assert!(validate_encoder_settings(&s).is_ok());

        s.bitrate_mode = BitrateMode::Vbr(0);
        assert!(validate_encoder_settings(&s).is_err());

        s.bitrate_mode = BitrateMode::Vbr(6);
        assert!(validate_encoder_settings(&s).is_err());
    }

    #[test]
    fn test_encoder_mode_combo_validation() {
        let mut s = base_settings();
        s.encoder_type = EncoderType::FdkHeAac;
        s.bitrate_mode = BitrateMode::Vbr(3);
        assert!(validate_encoder_settings(&s).is_ok());

        s.bitrate_mode = BitrateMode::Cbr;
        assert!(validate_encoder_settings(&s).is_err());

        s.encoder_type = EncoderType::AacAt;
        s.bitrate_mode = BitrateMode::Cvbr;
        assert!(validate_encoder_settings(&s).is_ok());
        s.bitrate_mode = BitrateMode::Cbr;
        assert!(validate_encoder_settings(&s).is_err());

        s.encoder_type = EncoderType::NativeAac;
        s.bitrate_mode = BitrateMode::Cbr;
        assert!(validate_encoder_settings(&s).is_ok());
        s.bitrate_mode = BitrateMode::Vbr(3);
        assert!(validate_encoder_settings(&s).is_err());

        // Auto remains VBR-only
        s.encoder_type = EncoderType::Auto;
        s.bitrate_mode = BitrateMode::Vbr(3);
        assert!(validate_encoder_settings(&s).is_ok());
        s.bitrate_mode = BitrateMode::Cvbr;
        assert!(validate_encoder_settings(&s).is_err());
    }

    #[test]
    fn test_threads_validation() {
        let mut s = base_settings();

        // Valid cases
        s.threads = ThreadSetting::Auto;
        assert!(validate_encoder_settings(&s).is_ok());

        s.threads = ThreadSetting::Off;
        assert!(validate_encoder_settings(&s).is_ok());

        s.threads = ThreadSetting::Fixed(1);
        assert!(validate_encoder_settings(&s).is_ok());

        s.threads = ThreadSetting::Fixed(4);
        assert!(validate_encoder_settings(&s).is_ok());

        s.threads = ThreadSetting::Fixed(1024);
        assert!(validate_encoder_settings(&s).is_ok());

        // Invalid cases - below range
        s.threads = ThreadSetting::Fixed(0);
        let err = validate_encoder_settings(&s).expect_err("expected rejection for thread count 0");
        assert!(err.to_string().contains("Invalid threads value: 0"));
        assert!(err.to_string().contains("1..=1024"));

        // Invalid cases - above range
        s.threads = ThreadSetting::Fixed(1025);
        let err =
            validate_encoder_settings(&s).expect_err("expected rejection for thread count 1025");
        assert!(err.to_string().contains("Invalid threads value: 1025"));
        assert!(err.to_string().contains("1..=1024"));

        s.threads = ThreadSetting::Fixed(2000);
        let err =
            validate_encoder_settings(&s).expect_err("expected rejection for thread count 2000");
        assert!(err.to_string().contains("Invalid threads value: 2000"));
        assert!(err.to_string().contains("1..=1024"));
    }

    #[test]
    fn test_thread_constant_consistency() {
        // Test that the constant matches the expected range
        assert_eq!(VALID_THREAD_COUNT_RANGE, 1..=1024);
        assert_eq!(*VALID_THREAD_COUNT_RANGE.start(), 1);
        assert_eq!(*VALID_THREAD_COUNT_RANGE.end(), 1024);
    }

    #[test]
    fn test_validate_threads_direct() {
        // Test the validate_threads function directly
        assert!(validate_threads(ThreadSetting::Auto).is_ok());
        assert!(validate_threads(ThreadSetting::Off).is_ok());

        // Test boundary values
        assert!(validate_threads(ThreadSetting::Fixed(1)).is_ok());
        assert!(validate_threads(ThreadSetting::Fixed(1024)).is_ok());

        // Test invalid values
        assert!(validate_threads(ThreadSetting::Fixed(0)).is_err());
        assert!(validate_threads(ThreadSetting::Fixed(1025)).is_err());

        // Verify error message format
        let err = validate_threads(ThreadSetting::Fixed(0)).expect_err("expected error");
        assert!(err.to_string().contains("Invalid threads value: 0"));
        assert!(err.to_string().contains("(allowed 1..=1024)"));
    }

    #[test]
    fn test_detect_available_encoders_struct_defaults() {
        // We can't guarantee availability on CI, but the function should always return a struct.
        let availability = detect_available_encoders();
        assert!(
            availability.native_aac_available
                || availability.aac_at_available
                || availability.fdk_available,
            "At least one encoder should be available in the environment"
        );
    }

    #[test]
    fn test_resolve_encoder_type_prefers_available() {
        let availability = EncoderAvailability {
            fdk_available: true,
            aac_at_available: true,
            native_aac_available: true,
        };
        let resolved = resolve_encoder_type(
            &EncoderSettings {
                encoder_type: EncoderType::Auto,
                ..base_settings()
            },
            &availability,
        );
        assert_eq!(resolved, EncoderType::FdkHeAac);

        let availability_no_fdk = EncoderAvailability {
            fdk_available: false,
            aac_at_available: true,
            native_aac_available: true,
        };
        let resolved = resolve_encoder_type(
            &EncoderSettings {
                encoder_type: EncoderType::Auto,
                ..base_settings()
            },
            &availability_no_fdk,
        );
        assert_eq!(resolved, EncoderType::AacAt);

        let availability_none = EncoderAvailability {
            fdk_available: false,
            aac_at_available: false,
            native_aac_available: true,
        };
        let resolved = resolve_encoder_type(
            &EncoderSettings {
                encoder_type: EncoderType::Auto,
                ..base_settings()
            },
            &availability_none,
        );
        assert_eq!(resolved, EncoderType::NativeAac);
    }
}
