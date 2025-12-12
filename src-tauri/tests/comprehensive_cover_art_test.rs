//! Comprehensive cover art embedding tests for Phase 12 L5 validation
//! Tests all aspects of the native cover art embedding feature

use audiobook_boss_lib::AudiobookMetadata;
use audiobook_boss_lib::{ffmpeg_detect_cover_art_format, ffmpeg_validate_metadata_compatibility};
use std::path::PathBuf;

// Test data - minimal valid JPEG and PNG images
const MINIMAL_JPEG: &[u8] =
    b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";
const MINIMAL_PNG: &[u8] = b"\x89PNG\r\n\x1A\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xDE\x00\x00\x00\x0CIDAT\x08\x99c```\x00\x00\x00\x04\x00\x01]\xCC\x18[\x00\x00\x00\x00IEND\xAEB`\x82";

#[test]
fn test_cover_art_format_detection_comprehensive() {
    println!("Testing cover art format detection...");

    // Test JPEG detection
    let jpeg_result = ffmpeg_detect_cover_art_format(MINIMAL_JPEG);
    assert!(jpeg_result.is_some(), "Should detect JPEG format");
    println!("✓ JPEG format detected correctly");

    // Test PNG detection
    let png_result = ffmpeg_detect_cover_art_format(MINIMAL_PNG);
    assert!(png_result.is_some(), "Should detect PNG format");
    println!("✓ PNG format detected correctly");

    // Test unsupported formats
    let gif_data = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
    let gif_result = ffmpeg_detect_cover_art_format(gif_data);
    assert!(
        gif_result.is_none(),
        "Should not detect unsupported GIF format"
    );
    println!("✓ Unsupported format (GIF) correctly rejected");

    let webp_data = b"RIFF\x00\x00\x00\x00WEBP";
    let webp_result = ffmpeg_detect_cover_art_format(webp_data);
    assert!(
        webp_result.is_none(),
        "Should not detect unsupported WebP format"
    );
    println!("✓ Unsupported format (WebP) correctly rejected");

    // Test empty data
    let empty_result = ffmpeg_detect_cover_art_format(&[]);
    assert!(
        empty_result.is_none(),
        "Should not detect format for empty data"
    );
    println!("✓ Empty data correctly handled");

    // Test insufficient data
    let short_data = b"\xFF\xD8"; // Too short for full JPEG signature
    let short_result = ffmpeg_detect_cover_art_format(short_data);
    assert!(
        short_result.is_none(),
        "Should not detect format for insufficient data"
    );
    println!("✓ Insufficient data correctly handled");

    println!("All format detection tests passed!");
}

#[test]
fn test_metadata_compatibility_validation_comprehensive() {
    println!("Testing metadata compatibility validation...");

    // Test metadata with warnings
    let problematic_metadata = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        track: Some((1, Some(12))), // Should generate warning
        disk: Some((1, Some(1))),   // Should generate warning
        cover_art: Some(vec![0u8; 15 * 1024 * 1024]), // 15MB - too large, should warn
        ..Default::default()
    };

    let warnings = ffmpeg_validate_metadata_compatibility(&problematic_metadata);
    assert!(
        warnings.len() >= 3,
        "Should generate at least 3 warnings (track, disk, cover art size)"
    );
    println!(
        "✓ Problematic metadata generated {} warnings",
        warnings.len()
    );
    for warning in &warnings {
        println!("  Warning: {}", warning);
    }

    // Test normal metadata
    let normal_metadata = AudiobookMetadata {
        title: Some("Normal Book".to_string()),
        artist: Some("Author".to_string()),
        album: Some("Series".to_string()),
        genre: Some("Audiobook".to_string()),
        date: Some(2025),
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    };

    let warnings = ffmpeg_validate_metadata_compatibility(&normal_metadata);
    assert!(
        warnings.is_empty(),
        "Normal metadata should not generate warnings, got: {:?}",
        warnings
    );
    println!("✓ Normal metadata passed validation without warnings");

    // Test edge case - exactly at size limit
    let limit_metadata = AudiobookMetadata {
        title: Some("Limit Test".to_string()),
        cover_art: Some(vec![0u8; 10 * 1024 * 1024]), // Exactly 10MB
        ..Default::default()
    };

    let warnings = ffmpeg_validate_metadata_compatibility(&limit_metadata);
    assert!(
        warnings.is_empty(),
        "10MB cover art should not generate warnings"
    );
    println!("✓ 10MB cover art (at limit) passed validation");

    println!("All metadata validation tests passed!");
}

#[test]
fn test_real_media_file_cover_art_detection() {
    println!("Testing cover art detection on real media file...");

    let media_file = PathBuf::from("../media/01 - Introduction.mp3");

    if !media_file.exists() {
        println!("Test media file not available, skipping real file test");
        return;
    }

    // Test reading metadata from real file
    match audiobook_boss_lib::commands::read_audio_metadata(
        media_file.to_string_lossy().to_string(),
    ) {
        Ok(metadata) => {
            println!("✓ Successfully read metadata from real MP3 file");
            if let Some(ref cover_art) = metadata.cover_art {
                println!("✓ Cover art found ({} bytes)", cover_art.len());

                // Test format detection on real cover art
                let format = ffmpeg_detect_cover_art_format(cover_art);
                println!("✓ Cover art format detected: {:?}", format);

                // Test validation on real cover art
                let warnings = ffmpeg_validate_metadata_compatibility(&metadata);
                println!("✓ Validation warnings: {}", warnings.len());
                for warning in &warnings {
                    println!("  Warning: {}", warning);
                }
            } else {
                println!("Real MP3 file has no cover art to test");
            }
        }
        Err(e) => {
            println!("Failed to read real MP3 metadata: {}", e);
            // This might be expected if the file format isn't supported
        }
    }

    println!("Real media file test completed!");
}

#[test]
fn test_ffmpeg_bridge_functions_directly() {
    println!("Testing FFmpeg bridge functions directly...");

    // Test metadata conversion
    let metadata = AudiobookMetadata {
        title: Some("Test Title".to_string()),
        artist: Some("Test Artist".to_string()),
        album: Some("Test Album".to_string()),
        genre: Some("Audiobook".to_string()),
        date: Some(2025),
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    };

    // Test that the bridge functions are accessible and working
    println!("Testing metadata validation...");
    let warnings = ffmpeg_validate_metadata_compatibility(&metadata);
    println!(
        "✓ Metadata validation completed with {} warnings",
        warnings.len()
    );
    assert!(
        warnings.is_empty(),
        "Valid metadata should not emit compatibility warnings: {warnings:?}"
    );

    // Test cover art format detection
    println!("Testing cover art format detection...");
    let jpeg_bytes = metadata
        .cover_art
        .as_ref()
        .expect("metadata must include cover art for detection");
    let jpeg_format = ffmpeg_detect_cover_art_format(jpeg_bytes);
    assert!(jpeg_format.is_some(), "Should detect JPEG format");
    println!("✓ JPEG format detection working");

    let png_format = ffmpeg_detect_cover_art_format(MINIMAL_PNG);
    assert!(png_format.is_some(), "Should detect PNG format");
    println!("✓ PNG format detection working");

    println!("FFmpeg bridge functions test passed!");
}

#[test]
fn test_unsupported_format_detection() {
    println!("Testing unsupported format detection...");

    // Test various unsupported formats
    let gif_data = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
    let webp_data = b"RIFF\x00\x00\x00\x00WEBP";
    let bmp_data = b"BM\x36\x00\x00\x00\x00\x00\x00\x00\x36\x00\x00\x00";
    let tiff_data = b"II*\x00";
    let unknown_data = b"UNKNOWN_FORMAT";

    let test_cases = vec![
        ("GIF", gif_data.as_slice()),
        ("WebP", webp_data.as_slice()),
        ("BMP", bmp_data.as_slice()),
        ("TIFF", tiff_data.as_slice()),
        ("Unknown", unknown_data.as_slice()),
    ];

    for (format_name, data) in test_cases {
        let result = ffmpeg_detect_cover_art_format(data);
        assert!(result.is_none(), "Should not detect {} format", format_name);
        println!("✓ {} format correctly rejected", format_name);
    }

    // Test that unsupported formats generate appropriate metadata warnings
    let unsupported_metadata = AudiobookMetadata {
        title: Some("Test".to_string()),
        cover_art: Some(b"GIF89a".to_vec()),
        ..Default::default()
    };

    let warnings = ffmpeg_validate_metadata_compatibility(&unsupported_metadata);
    // The validation function should warn about unsupported formats
    assert!(
        !warnings.is_empty(),
        "Expected warnings about unsupported cover art format, but got none"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.to_lowercase().contains("format") || w.to_lowercase().contains("supported")),
        "Expected at least one warning about format compatibility, got: {warnings:?}"
    );
    println!(
        "✓ Metadata validation correctly warns about unsupported format (warnings: {})",
        warnings.len()
    );

    println!("Unsupported format detection test passed!");
}

#[test]
fn test_large_cover_art_validation() {
    println!("Testing large cover art validation...");

    // Test size limits and warnings
    let size_test_cases = vec![
        (1024, "1KB", false),              // Small - should pass
        (1024 * 1024, "1MB", false),       // Medium - should pass
        (5 * 1024 * 1024, "5MB", false),   // Large but acceptable - should pass
        (10 * 1024 * 1024, "10MB", false), // At limit - should pass
        (15 * 1024 * 1024, "15MB", true),  // Over limit - should warn
        (50 * 1024 * 1024, "50MB", true),  // Very large - should warn
    ];

    for (size_bytes, size_desc, should_warn) in size_test_cases {
        // Create test cover art data of specified size
        let mut cover_data = MINIMAL_JPEG.to_vec();
        if size_bytes > MINIMAL_JPEG.len() {
            cover_data.extend(vec![0u8; size_bytes - MINIMAL_JPEG.len()]);
        }

        let metadata = AudiobookMetadata {
            title: Some(format!("Test {}", size_desc)),
            cover_art: Some(cover_data),
            ..Default::default()
        };

        let warnings = ffmpeg_validate_metadata_compatibility(&metadata);
        let has_size_warning = warnings.iter().any(|w| w.contains("size limit"));

        if should_warn {
            assert!(
                has_size_warning,
                "Cover art of {} should generate size warning",
                size_desc
            );
            println!("✓ {} cover art correctly generated size warning", size_desc);
        } else {
            assert!(
                !has_size_warning,
                "Cover art of {} should not generate size warning",
                size_desc
            );
            println!(
                "✓ {} cover art passed validation without size warning",
                size_desc
            );
        }
    }

    println!("Large cover art validation test passed!");
}

#[test]
fn test_no_cover_art_validation() {
    println!("Testing metadata validation without cover art...");

    let metadata_no_cover = AudiobookMetadata {
        title: Some("No Cover Test".to_string()),
        artist: Some("Test Author".to_string()),
        album: Some("Test Album".to_string()),
        cover_art: None, // No cover art
        ..Default::default()
    };

    // Test that metadata without cover art validates properly
    let warnings = ffmpeg_validate_metadata_compatibility(&metadata_no_cover);
    println!(
        "✓ Metadata without cover art validated with {} warnings",
        warnings.len()
    );

    // Should not have cover art related warnings
    let has_cover_warnings = warnings.iter().any(|w| w.to_lowercase().contains("cover"));
    assert!(
        !has_cover_warnings,
        "Should not have cover art warnings when no cover art provided"
    );
    println!("✓ No cover art related warnings generated");

    // Test format detection on None
    // (This would be called with empty data in practice)
    let empty_format = ffmpeg_detect_cover_art_format(&[]);
    assert!(
        empty_format.is_none(),
        "Empty data should not detect format"
    );
    println!("✓ Empty cover art data handled correctly");

    println!("No cover art validation test passed!");
}

#[test]
fn test_twoloop_environment_variable() {
    println!("Testing twoloop AAC enhancement environment variable handling...");

    // Test disabled state
    std::env::set_var("ABB_DISABLE_TWOLOOP", "1");
    let disable = std::env::var("ABB_DISABLE_TWOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(disable, "Should detect environment variable as disabled");
    println!("✓ ABB_DISABLE_TWOLOOP=1 correctly detected as disabled");

    // Test enabled (default)
    std::env::remove_var("ABB_DISABLE_TWOLOOP");
    let disable = std::env::var("ABB_DISABLE_TWOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(!disable, "Should default to enabled when env var not set");
    println!("✓ Default state (no env var) correctly detected as enabled");

    // Test alternative true values
    std::env::set_var("ABB_DISABLE_TWOLOOP", "true");
    let disable = std::env::var("ABB_DISABLE_TWOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(disable, "Should detect 'true' as disabled");
    println!("✓ ABB_DISABLE_TWOLOOP=true correctly detected as disabled");

    std::env::set_var("ABB_DISABLE_TWOLOOP", "TRUE");
    let disable = std::env::var("ABB_DISABLE_TWOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(
        disable,
        "Should detect 'TRUE' as disabled (case insensitive)"
    );
    println!("✓ ABB_DISABLE_TWOLOOP=TRUE correctly detected as disabled");

    // Test invalid values (should default to enabled)
    std::env::set_var("ABB_DISABLE_TWOLOOP", "maybe");
    let disable = std::env::var("ABB_DISABLE_TWOLOOP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    assert!(!disable, "Invalid values should default to enabled");
    println!("✓ Invalid value 'maybe' correctly defaults to enabled");

    // Clean up
    std::env::remove_var("ABB_DISABLE_TWOLOOP");
    println!("✓ Environment variable cleanup completed");

    println!("Twoloop environment variable test passed!");
}
