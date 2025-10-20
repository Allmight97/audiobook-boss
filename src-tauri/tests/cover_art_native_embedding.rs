//! Tests native ffmpeg-next cover art embedding.
//! Tests the native cover art embedding path using ffmpeg-next bridge functions.

use audiobook_boss_lib::AudiobookMetadata;

#[test]
fn test_cover_art_format_detection() {
    // Test the cover art format detection function

    // Test JPEG detection
    let jpeg_data = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";
    let jpeg_format = audiobook_boss_lib::ffmpeg_detect_cover_art_format(jpeg_data);
    assert!(jpeg_format.is_some(), "Should detect JPEG format");

    // Test PNG detection
    let png_data = b"\x89PNG\r\n\x1A\nrest";
    let png_format = audiobook_boss_lib::ffmpeg_detect_cover_art_format(png_data);
    assert!(png_format.is_some(), "Should detect PNG format");

    // Test unsupported format
    let gif_data = b"GIF89a";
    let gif_format = audiobook_boss_lib::ffmpeg_detect_cover_art_format(gif_data);
    assert!(
        gif_format.is_none(),
        "Should not detect unsupported formats"
    );
}

#[test]
fn test_metadata_compatibility_validation() {
    // Test metadata compatibility validation function
    let metadata = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        track: Some((1, Some(12))), // Should generate warning
        cover_art: Some(vec![0u8; 15 * 1024 * 1024]), // 15MB - too large, should warn
        ..Default::default()
    };

    let warnings = audiobook_boss_lib::ffmpeg_validate_metadata_compatibility(&metadata);
    assert!(
        warnings.len() >= 2,
        "Should generate at least 2 warnings for track and cover art size"
    );

    // Test that normal metadata doesn't generate warnings
    let normal_metadata = AudiobookMetadata {
        title: Some("Normal Book".to_string()),
        artist: Some("Author".to_string()),
        cover_art: Some(vec![0u8; 1024]), // 1KB - reasonable size
        ..Default::default()
    };

    let warnings = audiobook_boss_lib::ffmpeg_validate_metadata_compatibility(&normal_metadata);
    assert!(
        warnings.is_empty(),
        "Normal metadata should not generate warnings"
    );
}

#[test]
fn test_twoloop_environment_variable_handling() {
    // Test that environment variable parsing works correctly

    // Test disabled
    std::env::set_var("ABB_DISABLE_TWOOLOOP", "1");
    let disable = std::env::var("ABB_DISABLE_TWOOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(disable, "Should detect environment variable as disabled");

    // Test enabled (no env var)
    std::env::remove_var("ABB_DISABLE_TWOOLOOP");
    let disable = std::env::var("ABB_DISABLE_TWOOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(!disable, "Should default to enabled when env var not set");

    // Test alternative true values
    std::env::set_var("ABB_DISABLE_TWOOLOOP", "true");
    let disable = std::env::var("ABB_DISABLE_TWOOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(disable, "Should detect 'true' as disabled");

    // Clean up
    std::env::remove_var("ABB_DISABLE_TWOOLOOP");
}
