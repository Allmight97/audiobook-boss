//! Audio processing settings validation and management

use super::{AudioSettings, ChannelConfig};
use crate::errors::{AppError, Result};
use std::path::Path;

/// Validates audio processing settings
pub fn validate_audio_settings(settings: &AudioSettings) -> Result<()> {
    validate_bitrate(settings.bitrate)?;
    validate_sample_rate(settings.sample_rate)?;
    validate_output_path(&settings.output_path)?;
    Ok(())
}

/// Validates bitrate is within acceptable range
fn validate_bitrate(bitrate: u32) -> Result<()> {
    if !(32..=128).contains(&bitrate) {
        return Err(AppError::InvalidInput(
            format!("Bitrate must be between 32-128 kbps, got: {bitrate}")
        ));
    }
    Ok(())
}

/// Validates sample rate is supported
fn validate_sample_rate(sample_rate: u32) -> Result<()> {
    let valid_rates = [8000, 11025, 16000, 22050, 44100, 48000];
    if !valid_rates.contains(&sample_rate) {
        return Err(AppError::InvalidInput(
            format!("Unsupported sample rate: {sample_rate}. Valid rates: {valid_rates:?}")
        ));
    }
    Ok(())
}

/// Validates output path is writable
fn validate_output_path<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    
    // Check if parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(AppError::FileValidation(
                format!("Output directory does not exist: {}", parent.display())
            ));
        }
    }
    
    // Check file extension
    match path.extension().and_then(|s| s.to_str()) {
        Some("m4b") => Ok(()),
        Some(ext) => Err(AppError::InvalidInput(
            format!("Output must be .m4b file, got: .{ext}")
        )),
        None => Err(AppError::InvalidInput(
            "Output file must have .m4b extension".to_string()
        )),
    }
}

impl AudioSettings {
    /// Creates settings optimized for audiobooks
    #[allow(dead_code)]
    pub fn audiobook_preset() -> Self {
        Self {
            bitrate: 64,  // Good quality for speech
            channels: ChannelConfig::Mono,  // Most audiobooks are mono
            sample_rate: 22050,  // Sufficient for speech
            output_path: "audiobook.m4b".into(),
        }
    }
    
    /// Creates high-quality settings
    #[allow(dead_code)]
    pub fn high_quality_preset() -> Self {
        Self {
            bitrate: 128,
            channels: ChannelConfig::Stereo,
            sample_rate: 44100,
            output_path: "audiobook_hq.m4b".into(),
        }
    }
    
    /// Creates low-bandwidth settings
    #[allow(dead_code)]
    pub fn low_bandwidth_preset() -> Self {
        Self {
            bitrate: 32,
            channels: ChannelConfig::Mono,
            sample_rate: 16000,
            output_path: "audiobook_low.m4b".into(),
        }
    }
}

impl ChannelConfig {
    /// Returns the number of channels
    pub fn channel_count(&self) -> u8 {
        match self {
            ChannelConfig::Mono => 1,
            ChannelConfig::Stereo => 2,
        }
    }
    
    /// Returns FFmpeg channel layout string
    #[allow(dead_code)]
    pub fn ffmpeg_layout(&self) -> &'static str {
        match self {
            ChannelConfig::Mono => "mono",
            ChannelConfig::Stereo => "stereo",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_bitrate_valid() {
        assert!(validate_bitrate(64).is_ok());
        assert!(validate_bitrate(32).is_ok());
        assert!(validate_bitrate(128).is_ok());
    }

    #[test]
    fn test_validate_bitrate_invalid() {
        assert!(validate_bitrate(16).is_err());
        assert!(validate_bitrate(256).is_err());
    }

    #[test]
    fn test_validate_sample_rate_valid() {
        assert!(validate_sample_rate(22050).is_ok());
        assert!(validate_sample_rate(44100).is_ok());
    }

    #[test]
    fn test_validate_sample_rate_invalid() {
        assert!(validate_sample_rate(12345).is_err());
    }

    #[test]
    fn test_validate_output_path_valid() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test.m4b");
        assert!(validate_output_path(&output_path).is_ok());
    }

    #[test]
    fn test_validate_output_path_invalid_extension() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test.mp3");
        let result = validate_output_path(&output_path);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains(".m4b"));
    }

    #[test]
    fn test_validate_output_path_nonexistent_dir() {
        let result = validate_output_path("/nonexistent/dir/test.m4b");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_audiobook_preset() {
        let settings = AudioSettings::audiobook_preset();
        assert_eq!(settings.bitrate, 64);
        assert!(matches!(settings.channels, ChannelConfig::Mono));
        assert_eq!(settings.sample_rate, 22050);
    }

    #[test]
    fn test_channel_config_methods() {
        assert_eq!(ChannelConfig::Mono.channel_count(), 1);
        assert_eq!(ChannelConfig::Stereo.channel_count(), 2);
        assert_eq!(ChannelConfig::Mono.ffmpeg_layout(), "mono");
        assert_eq!(ChannelConfig::Stereo.ffmpeg_layout(), "stereo");
    }
}