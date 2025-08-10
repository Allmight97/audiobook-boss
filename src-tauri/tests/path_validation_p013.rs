// Integration test for P0.1.3 - symlink policy and output directory validation
// Tests symlink handling and write permission probing

use audiobook_boss_lib::commands::validate_audio_settings;
use audiobook_boss_lib::audio::{AudioSettings, ChannelConfig, SampleRateConfig};
use tempfile::TempDir;
use std::fs::Permissions;

#[cfg(unix)]
#[test]
fn test_read_only_output_directory_integration() {
    use std::os::unix::fs::PermissionsExt;
    
    let temp_dir = TempDir::new().expect("create temp dir");
    let readonly_dir = temp_dir.path().join("readonly");
    std::fs::create_dir(&readonly_dir).expect("create readonly dir");
    
    // Make directory read-only
    let readonly_perms = Permissions::from_mode(0o444);
    std::fs::set_permissions(&readonly_dir, readonly_perms).expect("set readonly permissions");
    
    // Test settings validation with read-only output directory
    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: readonly_dir.join("output.m4b"),
    };
    
    let result = validate_audio_settings(settings);
    assert!(result.is_err(), "Should reject read-only output directory");
    assert!(result.expect_err("expected write permission error").to_string().contains("not writable"));
    
    // Restore permissions for cleanup
    let normal_perms = Permissions::from_mode(0o755);
    std::fs::set_permissions(&readonly_dir, normal_perms).expect("restore permissions");
}

#[test]
fn test_output_path_validation_integration() {
    let temp_dir = TempDir::new().expect("create temp dir");
    
    // Test valid output path
    let valid_settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: temp_dir.path().join("valid_output.m4b"),
    };
    let result = validate_audio_settings(valid_settings);
    assert!(result.is_ok(), "Valid settings should pass validation");
    
    // Test invalid extension
    let invalid_ext_settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: temp_dir.path().join("invalid_output.mp3"),
    };
    let result = validate_audio_settings(invalid_ext_settings);
    assert!(result.is_err(), "Should reject non-.m4b extension");
    assert!(result.expect_err("expected extension error").to_string().contains(".m4b"));
    
    // Test nonexistent parent directory
    let nonexistent_dir_settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Auto,
        output_path: "/nonexistent/directory/output.m4b".into(),
    };
    let result = validate_audio_settings(nonexistent_dir_settings);
    assert!(result.is_err(), "Should reject nonexistent parent directory");
    assert!(result.expect_err("expected directory error").to_string().contains("does not exist"));
}
