use super::{AudiobookFormat, EncoderType, FaacProfile, SampleRateConfig};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::path::Path;

const SUPPORTED_SAMPLE_RATES: &[u32] = &[
    7350, 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
];
const FAAC_SAMPLE_RATES: &[u32] = &[32000, 44100, 48000];

pub fn validate_sample_rate_config(config: &SampleRateConfig) -> Result<()> {
    match config {
        SampleRateConfig::Auto => Ok(()),
        SampleRateConfig::Explicit(rate) => validate_explicit_sample_rate(*rate),
    }
}

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

pub(crate) fn encoder_sample_rates(encoder: EncoderType, profile: FaacProfile) -> &'static [u32] {
    match encoder {
        EncoderType::Opus => &[8000, 12000, 16000, 24000, 48000],
        EncoderType::Faac if profile == FaacProfile::HeAacV1 => FAAC_SAMPLE_RATES,
        _ => SUPPORTED_SAMPLE_RATES,
    }
}

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

pub(crate) fn check_output_directory(dir: &Path, allow_missing: bool) -> Result<()> {
    if !dir.exists() {
        if allow_missing {
            return Ok(());
        }
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
    Ok(())
}

fn probe_output_directory_writable(dir: &Path) -> Result<()> {
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
            if let Err(error) = std::fs::remove_file(&temp_file) {
                log::warn!("Could not remove the output directory write probe: {error}");
            }
            Ok(())
        }
        Err(e) => Err(AppError::FileValidation(format!(
            "Output directory not writable: {e}"
        ))),
    }
}

pub(crate) fn validate_output_target(path: &Path, format: AudiobookFormat) -> Result<()> {
    if let Some(parent) = path.parent() {
        check_output_directory(parent, false)?;
        probe_output_directory_writable(parent)?;
    }
    let wanted = format.extension();
    match path.extension().and_then(|value| value.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case(wanted) => Ok(()),
        Some(extension) => Err(AppError::InvalidInput(format!(
            "Output must be .{wanted}, got: .{extension}"
        ))),
        None => Err(AppError::InvalidInput(format!(
            "Output must have the .{wanted} extension"
        ))),
    }
}

/// Auto retains supported source rates, otherwise uses the next supported rate,
/// capped at the encoder/profile maximum. Explicit requests are validated unchanged.
pub(crate) fn automatic_sample_rate(
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
#[path = "settings_tests.rs"]
mod tests;
