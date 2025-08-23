//! Encoder v2 settings types and validation (Phase 1 — no behavior change for v1)
//!
//! This module defines the advanced encoder settings surface used by the
//! upcoming v2 command, along with validation helpers. It also provides
//! small FFI helpers for encoder-by-name availability checks needed for
//! AAC-AT selection on macOS.

use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};

/// Supported encoder types for audiobooks
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncoderType {
    /// Apple AAC (AudioToolbox), macOS-only
    AacAt,
    /// HE-AAC v1 (AAC-LC + SBR)
    HeAacV1,
    /// HE-AAC v2 (AAC-LC + SBR + PS); stereo only
    HeAacV2,
}

/// AAC coder implementation for the native `aac` encoder
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AacCoder {
    Twoloop,
    Fast,
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
    /// Allowed: 56|64|72|80|88|96 (kbps)
    pub bitrate_kbps: u16,
    /// Allowed: 1|2
    pub channels: u8,
    /// Native AAC only; ignored for AAC-AT
    pub aac_coder: Option<AacCoder>,
    /// FDK-only; ignored otherwise
    pub afterburner: Option<bool>,
    pub threads: ThreadSetting,
}

/// Whitelist of supported encoder bitrates for speech-oriented output
pub const VALID_ENCODER_BITRATES: &[u16] = &[56, 64, 72, 80, 88, 96];

/// Valid range for thread count when using Fixed thread setting
pub const VALID_THREAD_COUNT_RANGE: std::ops::RangeInclusive<u16> = 1..=1024;

/// Validates encoder settings (no engine side-effects)
pub fn validate_encoder_settings(settings: &EncoderSettings) -> Result<()> {
    validate_bitrate(settings.bitrate_kbps)?;
    validate_channels(settings.channels)?;
    validate_profile_channel_combo(settings.encoder_type, settings.channels)?;
    validate_threads(settings.threads)?;

    // Afterburner notice: only applicable to libfdk_aac encoders.
    if settings.afterburner.unwrap_or(false)
        && !is_encoder_available_by_name("libfdk_aac")
    {
        log::info!(
            "afterburner flag present but libfdk_aac unavailable - will be ignored"
        );
    }

    // aac_coder is best-effort; no validation error even if not honored.
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

fn validate_channels(channels: u8) -> Result<()> {
    match channels {
        1 | 2 => Ok(()),
        n => Err(AppError::InvalidInput(format!(
            "Unsupported channel count: {}. Allowed: 1 or 2",
            n
        ))),
    }
}

fn validate_profile_channel_combo(encoder_type: EncoderType, channels: u8) -> Result<()> {
    if matches!(encoder_type, EncoderType::HeAacV2) && channels != 2 {
        return Err(AppError::InvalidInput(
            "HE-AAC v2 is stereo-only (Parametric Stereo). Use HE-AAC v1 for mono.".to_string(),
        ));
    }
    Ok(())
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

/// Resolves the requested encoder name, falling back to native `aac` when unavailable.
/// This does not open the encoder; it only chooses the preferred name.
pub fn resolve_encoder_name(encoder_type: EncoderType) -> &'static str {
    match encoder_type {
        EncoderType::AacAt => {
            // AAC-AT is macOS-only; prefer it when present.
            if cfg!(target_os = "macos") && is_encoder_available_by_name("aac_at") {
                "aac_at"
            } else {
                "aac"
            }
        }
        // For HE profiles we still use underlying AAC encoder with profile flags.
        EncoderType::HeAacV1 | EncoderType::HeAacV2 => "aac",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::HeAacV1,
            bitrate_kbps: 64,
            channels: 1,
            aac_coder: None,
            afterburner: None,
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
        let mut s = base_settings();
        s.bitrate_kbps = 48; // not in whitelist
        assert!(validate_encoder_settings(&s).is_err());
    }

    #[test]
    fn test_he_aac_v2_requires_stereo() {
        let mut s = base_settings();
        s.encoder_type = EncoderType::HeAacV2;
        s.channels = 2;
        assert!(validate_encoder_settings(&s).is_ok());

        s.channels = 1;
        let err = validate_encoder_settings(&s).expect_err("expected mono rejection for HE-AAC v2");
        assert!(err.to_string().to_lowercase().contains("stereo"));
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
        let err = validate_encoder_settings(&s).expect_err("expected rejection for thread count 1025");
        assert!(err.to_string().contains("Invalid threads value: 1025"));
        assert!(err.to_string().contains("1..=1024"));
        
        s.threads = ThreadSetting::Fixed(2000);
        let err = validate_encoder_settings(&s).expect_err("expected rejection for thread count 2000");
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
}


