//! Audio processing settings validation utilities (v2-only)

use super::SampleRateConfig;
use crate::errors::{AppError, Result};
use std::path::Path;

/// Validates sample rate configuration
pub fn validate_sample_rate_config(config: &SampleRateConfig) -> Result<()> {
    match config {
        SampleRateConfig::Auto => Ok(()), // Auto is always valid
        SampleRateConfig::Explicit(rate) => validate_explicit_sample_rate(*rate),
    }
}

/// Validates explicit sample rate is supported
fn validate_explicit_sample_rate(sample_rate: u32) -> Result<()> {
    let valid_rates = [22050, 32000, 44100, 48000];
    if !valid_rates.contains(&sample_rate) {
        return Err(AppError::InvalidInput(format!(
            "Unsupported sample rate: {sample_rate}. Valid rates: {valid_rates:?}"
        )));
    }
    Ok(())
}

/// Validates output directory is writable by creating and removing a temp file
fn validate_output_directory_writable<P: AsRef<Path>>(dir_path: P) -> Result<()> {
    let dir = dir_path.as_ref();

    if !dir.exists() {
        return Err(AppError::FileValidation(format!(
            "Output directory does not exist: {}",
            dir.display()
        )));
    }

    if !dir.is_dir() {
        return Err(AppError::FileValidation(format!(
            "Output path is not a directory: {}",
            dir.display()
        )));
    }

    // Probe write permission by creating and removing a temp file
    let temp_file = dir.join(".audiobook_boss_write_test");
    match std::fs::write(&temp_file, b"test") {
        Ok(_) => {
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
        Some("m4b") => Ok(()),
        Some(ext) => Err(AppError::InvalidInput(format!(
            "Output must be .m4b file, got: .{ext}"
        ))),
        None => Err(AppError::InvalidInput(
            "Output file must have .m4b extension".to_string(),
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
