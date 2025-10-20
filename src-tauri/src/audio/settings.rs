//! Audio processing settings validation and management

use super::{AudioSettings, ChannelConfig, SampleRateConfig};
use crate::errors::{AppError, Result};
use std::path::Path;

/// Validates audio processing settings
pub fn validate_audio_settings(settings: &AudioSettings) -> Result<()> {
    validate_bitrate(settings.bitrate)?;
    validate_sample_rate_config(&settings.sample_rate)?;
    validate_output_path(&settings.output_path)?;
    Ok(())
}

/// Validates bitrate is within acceptable range
fn validate_bitrate(bitrate: u32) -> Result<()> {
    if !(32..=128).contains(&bitrate) {
        return Err(AppError::InvalidInput(format!(
            "Bitrate must be between 32-128 kbps, got: {bitrate}"
        )));
    }
    Ok(())
}

/// Validates sample rate configuration
fn validate_sample_rate_config(config: &SampleRateConfig) -> Result<()> {
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
fn validate_output_path<P: AsRef<Path>>(path: P) -> Result<()> {
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

impl AudioSettings {
    /// Returns the standard audiobook preset.
    ///
    /// Chosen for typical spoken-word balance of size vs quality:
    /// - 64 kbps mono is common for audiobooks
    /// - Auto sample rate preserves original where possible
    ///
    /// Caller is expected to set a specific `output_path` before validation.
    pub fn audiobook_preset() -> Self {
        Self {
            bitrate: 64,
            channels: ChannelConfig::Mono,
            sample_rate: SampleRateConfig::Auto,
            // Placeholder path; tests overwrite. Keep consistent with Default extension (.m4b)
            output_path: std::path::PathBuf::from("output.m4b"),
        }
    }

    /// Returns a higher quality stereo preset suitable for music-heavy or
    /// ambience-rich audiobooks. Uses 128 kbps stereo @ 44.1 kHz.
    pub fn high_quality_preset() -> Self {
        Self {
            bitrate: 128,
            channels: ChannelConfig::Stereo,
            sample_rate: SampleRateConfig::Explicit(44100),
            output_path: std::path::PathBuf::from("output.m4b"),
        }
    }

    /// Returns a low bandwidth preset for minimal file size / slower networks.
    /// 32 kbps mono @ 22.05 kHz still preserves intelligibility for speech.
    pub fn low_bandwidth_preset() -> Self {
        Self {
            bitrate: 32,
            channels: ChannelConfig::Mono,
            sample_rate: SampleRateConfig::Explicit(22050),
            output_path: std::path::PathBuf::from("output.m4b"),
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

    // Removed: ffmpeg_layout() (CLI-oriented helper not used at runtime)
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
    fn test_validate_sample_rate_config_auto() {
        assert!(validate_sample_rate_config(&SampleRateConfig::Auto).is_ok());
    }

    #[test]
    fn test_validate_sample_rate_config_explicit_valid() {
        assert!(validate_sample_rate_config(&SampleRateConfig::Explicit(22050)).is_ok());
        assert!(validate_sample_rate_config(&SampleRateConfig::Explicit(32000)).is_ok());
        assert!(validate_sample_rate_config(&SampleRateConfig::Explicit(44100)).is_ok());
        assert!(validate_sample_rate_config(&SampleRateConfig::Explicit(48000)).is_ok());
    }

    #[test]
    fn test_validate_sample_rate_config_explicit_invalid() {
        assert!(validate_sample_rate_config(&SampleRateConfig::Explicit(12345)).is_err());
        assert!(validate_sample_rate_config(&SampleRateConfig::Explicit(16000)).is_err());
        assert!(validate_sample_rate_config(&SampleRateConfig::Explicit(8000)).is_err());
    }

    #[test]
    fn test_validate_output_path_valid() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output_path = temp_dir.path().join("test.m4b");
        assert!(validate_output_path(&output_path).is_ok());
    }

    #[test]
    fn test_validate_output_path_invalid_extension() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output_path = temp_dir.path().join("test.mp3");
        let result = validate_output_path(&output_path);
        assert!(result.is_err());
        let error_msg = result.expect_err("expected invalid extension").to_string();
        assert!(error_msg.contains(".m4b"));
    }

    #[test]
    fn test_validate_output_path_nonexistent_dir() {
        let result = validate_output_path("/nonexistent/dir/test.m4b");
        assert!(result.is_err());
        let err = result.expect_err("expected nonexistent dir error");
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn test_output_directory_write_permission_probe() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let result = validate_output_directory_writable(temp_dir.path());
        assert!(result.is_ok(), "Temp directory should be writable");

        // Test with file instead of directory
        let temp_file = temp_dir.path().join("not_a_dir.txt");
        std::fs::write(&temp_file, b"test").expect("create temp file");
        let result = validate_output_directory_writable(&temp_file);
        assert!(result.is_err(), "File should not be valid as directory");
        assert!(result
            .expect_err("expected not directory error")
            .to_string()
            .contains("not a directory"));
    }

    #[cfg(unix)]
    #[test]
    fn test_read_only_output_directory() {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().expect("create temp dir");

        // Make directory read-only
        let readonly_perms = Permissions::from_mode(0o444);
        std::fs::set_permissions(temp_dir.path(), readonly_perms)
            .expect("set readonly permissions");

        let result = validate_output_directory_writable(temp_dir.path());
        assert!(
            result.is_err(),
            "Read-only directory should fail write test"
        );
        assert!(result
            .expect_err("expected write permission error")
            .to_string()
            .contains("not writable"));

        // Restore permissions for cleanup
        let normal_perms = Permissions::from_mode(0o755);
        std::fs::set_permissions(temp_dir.path(), normal_perms).expect("restore permissions");
    }

    #[test]
    fn test_channel_config_methods() {
        assert_eq!(ChannelConfig::Mono.channel_count(), 1);
        assert_eq!(ChannelConfig::Stereo.channel_count(), 2);
    }

    #[test]
    fn test_sample_rate_config_methods() {
        // Test Auto configuration
        let auto_config = SampleRateConfig::Auto;
        assert!(auto_config.requires_detection());
        assert_eq!(auto_config.explicit_rate(), None);

        // Test Explicit configuration
        let explicit_config = SampleRateConfig::Explicit(44100);
        assert!(!explicit_config.requires_detection());
        assert_eq!(explicit_config.explicit_rate(), Some(44100));

        // Test different explicit rates
        let rates = [22050, 32000, 44100, 48000];
        for rate in rates {
            let config = SampleRateConfig::Explicit(rate);
            assert!(!config.requires_detection());
            assert_eq!(config.explicit_rate(), Some(rate));
        }
    }
}
