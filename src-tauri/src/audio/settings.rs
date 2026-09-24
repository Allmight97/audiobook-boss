//! Audio processing settings validation utilities

use super::{EncoderType, FaacProfile, SampleRateConfig};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::path::Path;

const SUPPORTED_SAMPLE_RATES: &[u32] = &[
    7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
];
const FAAC_SAMPLE_RATES: &[u32] = &[32000, 44100, 48000];

/// Validates sample rate configuration
pub fn validate_sample_rate_config(config: &SampleRateConfig) -> Result<()> {
    match config {
        SampleRateConfig::Auto => Ok(()), // Auto is always valid
        SampleRateConfig::Explicit(rate) => validate_explicit_sample_rate(*rate),
    }
}

/// Validates explicit sample rate is supported
fn validate_explicit_sample_rate(sample_rate: u32) -> Result<()> {
    if !SUPPORTED_SAMPLE_RATES.contains(&sample_rate) {
        return Err(AppError::InvalidInput(format!(
            "Unsupported sample rate: {sample_rate}. Valid rates: {SUPPORTED_SAMPLE_RATES:?}"
        )));
    }
    Ok(())
}

pub fn supported_sample_rates() -> &'static [u32] {
    SUPPORTED_SAMPLE_RATES
}

/// Returns the output rates supported by the selected encoder. The general
/// sample-rate list remains unchanged for existing encoders; FAAC HE-AAC has
/// a narrower ABB-supported set for its explicit HE profile.
pub(crate) fn encoder_sample_rates(encoder: EncoderType, profile: FaacProfile) -> &'static [u32] {
    match encoder {
        EncoderType::FdkHeAac => super::FdkProfile::AacLc.sample_rates(),
        EncoderType::Opus => &[8000, 12000, 16000, 24000, 48000],
        EncoderType::Faac if profile == FaacProfile::HeAacV1 => FAAC_SAMPLE_RATES,
        _ => SUPPORTED_SAMPLE_RATES,
    }
}

/// Validates an explicit rate against both the general audio settings and the
/// selected encoder's supported rates.
pub(crate) fn validate_encoder_sample_rate(
    encoder: EncoderType,
    profile: FaacProfile,
    config: &SampleRateConfig,
) -> Result<()> {
    validate_sample_rate_config(config)?;
    if let SampleRateConfig::Explicit(rate) = config {
        let supported = encoder_sample_rates(encoder, profile);
        if !supported.contains(rate) {
            return Err(AppError::InvalidInput(format!(
                "{encoder} does not support {rate} Hz. Choose one of {supported:?}."
            )));
        }
    }
    Ok(())
}

/// Validates output directory is writable by creating and removing a temp file
fn validate_output_directory_writable<P: AsRef<Path>>(dir_path: P) -> Result<()> {
    let dir = dir_path.as_ref();

    if !dir.exists() {
        return Err(AppError::FileValidation(format!(
            "Output directory does not exist: {}",
            sanitize_path_for_display(dir)
        )));
    }

    if !dir.is_dir() {
        return Err(AppError::FileValidation(format!(
            "Output path is not a directory: {}",
            sanitize_path_for_display(dir)
        )));
    }

    // Probe write permission by creating and removing a temp file
    let temp_file = dir.join(format!(
        ".audiobook_boss_write_test_{}",
        uuid::Uuid::new_v4()
    ));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_file)
    {
        Ok(file) => {
            drop(file);
            // Clean up test file
            let _ = std::fs::remove_file(&temp_file);
            Ok(())
        }
        Err(e) => Err(AppError::FileValidation(format!(
            "Output directory not writable: {e}"
        ))),
    }
}

/// Validates output path is writable
pub fn validate_output_path<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    // Validate parent directory exists and is writable
    if let Some(parent) = path.parent() {
        validate_output_directory_writable(parent)?;
    }

    // Check file extension
    match path.extension().and_then(|s| s.to_str()) {
        Some("m4b" | "m4a" | "mka") => Ok(()),
        Some(ext) => Err(AppError::InvalidInput(format!(
            "Encoded output must be .m4b, .m4a or .mka, got: .{ext}"
        ))),
        None => Err(AppError::InvalidInput(
            "Encoded output requires an .m4b, .m4a or .mka extension".to_string(),
        )),
    }
}

/// Validates an output path for a byte-preserved source artifact.
pub fn validate_preserved_output_path<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        validate_output_directory_writable(parent)?;
    }
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("m4b" | "m4a" | "mp3" | "mka") => Ok(()),
        Some(extension) => Err(AppError::InvalidInput(format!(
            "Preserved output must be .m4b, .m4a, .mka or .mp3, got: .{extension}"
        ))),
        None => Err(AppError::InvalidInput(
            "Preserved output must have a supported audio extension".to_string(),
        )),
    }
}

impl SampleRateConfig {
    /// Returns whether this configuration requires sample rate detection
    pub fn requires_detection(&self) -> bool {
        matches!(self, SampleRateConfig::Auto)
    }

    /// Returns the sample rate value if explicit, None if auto
    pub fn explicit_rate(&self) -> Option<u32> {
        match self {
            SampleRateConfig::Explicit(rate) => Some(*rate),
            SampleRateConfig::Auto => None,
        }
    }
}

/// Auto retains supported source rates, otherwise uses the next supported rate,
/// capped at the encoder/profile maximum. Explicit requests are validated unchanged.
pub(in crate::audio) fn automatic_sample_rate(
    encoder: EncoderType,
    profile: FaacProfile,
    source_rate: u32,
) -> u32 {
    let rates = encoder_sample_rates(encoder, profile);
    rates
        .iter()
        .copied()
        .find(|rate| *rate >= source_rate)
        .unwrap_or_else(|| *rates.last().expect("encoder sample rates are nonempty"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_rate_changes_only_to_satisfy_the_encoder_profile() {
        for (encoder, profile, source, expected) in [
            (EncoderType::Faac, FaacProfile::HeAacV1, 22_050, 32_000),
            (EncoderType::Faac, FaacProfile::HeAacV1, 96_000, 48_000),
            (EncoderType::Faac, FaacProfile::AacLc, 22_050, 22_050),
            (EncoderType::NativeAac, FaacProfile::Auto, 44_100, 44_100),
            (EncoderType::Opus, FaacProfile::Auto, 22_050, 24_000),
            (EncoderType::Opus, FaacProfile::Auto, 44_100, 48_000),
        ] {
            assert_eq!(automatic_sample_rate(encoder, profile, source), expected);
        }
    }

    #[test]
    fn faac_rejects_an_explicit_rate_outside_its_capability() {
        let error = validate_encoder_sample_rate(
            EncoderType::Faac,
            FaacProfile::HeAacV1,
            &SampleRateConfig::Explicit(22050),
        )
        .expect_err("FAAC should reject rates below 32 kHz");
        assert!(error.to_string().contains("does not support 22050 Hz"));
    }
}
