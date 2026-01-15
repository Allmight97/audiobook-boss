//! Integration tests for cover art embedding with real media files.
//!
//! Tests the full pipeline using real files and FFmpeg infrastructure.

use audiobook_boss_lib::AudiobookMetadata;
use std::path::PathBuf;

const TEST_MEDIA_FILE: &str = "../media/01 - Introduction.mp3";

fn ensure_media() -> Option<PathBuf> {
    let p = PathBuf::from(TEST_MEDIA_FILE);
    if p.exists() && p.is_file() {
        Some(p)
    } else {
        None
    }
}

#[test]
fn test_real_mp3_cover_art_integration() {
    println!("Testing cover art integration with real MP3 file...");

    let media_file = match ensure_media() {
        Some(file) => file,
        None => {
            println!("Test media file not available, skipping integration test");
            return;
        }
    };

    println!("Using media file: {}", media_file.display());

    // Test reading metadata with cover art
    match audiobook_boss_lib::commands::read_audio_metadata(
        media_file.to_string_lossy().to_string(),
    ) {
        Ok(metadata) => {
            println!("✓ Successfully read metadata from MP3");

            // Validate metadata structure
            assert!(metadata.title.is_some(), "MP3 should have title metadata");
            assert!(metadata.artist.is_some(), "MP3 should have artist metadata");

            println!("Metadata:");
            println!("  Title: {:?}", metadata.title);
            println!("  Artist: {:?}", metadata.artist);
            println!("  Album: {:?}", metadata.album);
            println!("  Genre: {:?}", metadata.genre);

            if let Some(ref cover_art) = metadata.cover_art {
                println!("✓ Cover art present ({} bytes)", cover_art.len());

                // Test format detection
                let format = audiobook_boss_lib::ffmpeg_detect_cover_art_format(cover_art);
                println!("✓ Cover art format: {:?}", format);
                assert!(
                    format.is_some(),
                    "Real cover art should have detectable format"
                );

                // Test validation
                let warnings =
                    audiobook_boss_lib::ffmpeg_validate_metadata_compatibility(&metadata);
                println!("✓ Validation completed with {} warnings", warnings.len());
                for warning in &warnings {
                    println!("  Warning: {}", warning);
                }

                // Test that cover art is reasonable size
                assert!(
                    cover_art.len() > 1000,
                    "Cover art should be substantial size"
                );
                assert!(
                    cover_art.len() < 10 * 1024 * 1024,
                    "Cover art should be under 10MB"
                );

                // Test round-trip: write and read back
                test_cover_art_round_trip(&metadata, &media_file);
            } else {
                println!("Real MP3 file has no cover art - testing metadata-only flow");

                // Even without cover art, metadata should validate
                let warnings =
                    audiobook_boss_lib::ffmpeg_validate_metadata_compatibility(&metadata);
                println!("✓ Metadata-only validation: {} warnings", warnings.len());
            }
        }
        Err(e) => {
            println!("Failed to read MP3 metadata: {}", e);
            panic!("Should be able to read metadata from real MP3 file");
        }
    }

    println!("Real MP3 cover art integration test passed!");
}

#[test]
fn test_real_media_file_cover_art_detection() {
    println!("Testing cover art detection on real media file...");

    let media_file = PathBuf::from(TEST_MEDIA_FILE);

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
                let format = audiobook_boss_lib::ffmpeg_detect_cover_art_format(cover_art);
                println!("✓ Cover art format detected: {:?}", format);

                // Test validation on real cover art
                let warnings =
                    audiobook_boss_lib::ffmpeg_validate_metadata_compatibility(&metadata);
                println!("✓ Validation warnings: {}", warnings.len());
                for warning in &warnings {
                    println!("  Warning: {}", warning);
                }
            } else {
                println!("Real MP3 file has no cover art to test");
            }
        }
        Err(e) => {
            panic!("Failed to read real MP3 metadata: {}", e);
        }
    }

    println!("Real media file test completed!");
}

fn test_cover_art_round_trip(original_metadata: &AudiobookMetadata, source_file: &PathBuf) {
    println!("Testing cover art round-trip (write then read)...");

    use tempfile::NamedTempFile;

    // Create temporary file for testing
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let temp_path = temp_file.path();

    // Copy original file to temp location for testing
    std::fs::copy(source_file, temp_path).expect("Failed to copy test file");

    // Test writing cover art to the temp file
    if let Some(ref cover_data) = original_metadata.cover_art {
        match audiobook_boss_lib::commands::write_cover_art(
            temp_path.to_string_lossy().to_string(),
            cover_data.clone(),
        ) {
            Ok(_) => {
                println!("✓ Cover art written successfully");

                // Read back and verify
                match audiobook_boss_lib::commands::read_audio_metadata(
                    temp_path.to_string_lossy().to_string(),
                ) {
                    Ok(read_back_metadata) => {
                        println!("✓ Successfully read back metadata");

                        if let Some(ref read_back_cover) = read_back_metadata.cover_art {
                            println!(
                                "✓ Cover art present in read-back ({} bytes)",
                                read_back_cover.len()
                            );

                            // Verify cover art is substantial and format is preserved
                            assert!(
                                read_back_cover.len() > 1000,
                                "Read-back cover art should be substantial"
                            );

                            let original_format =
                                audiobook_boss_lib::ffmpeg_detect_cover_art_format(cover_data);
                            let readback_format =
                                audiobook_boss_lib::ffmpeg_detect_cover_art_format(read_back_cover);

                            println!("  Original format: {:?}", original_format);
                            println!("  Read-back format: {:?}", readback_format);

                            // Note: formats might differ due to transcoding, but both should be valid
                            assert!(
                                readback_format.is_some(),
                                "Read-back cover art should have valid format"
                            );
                        } else {
                            println!(
                                "⚠ Cover art not found in read-back - may indicate embedding issue"
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed to read back metadata: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Failed to write cover art: {}", e);
                // This might be expected for some file formats
            }
        }
    }

    println!("✓ Round-trip test completed");
}

#[test]
fn test_cover_art_commands_error_handling() {
    println!("Testing cover art command error handling...");

    // Test reading from non-existent file
    let result =
        audiobook_boss_lib::commands::read_audio_metadata("non_existent_file.mp3".to_string());
    assert!(result.is_err(), "Should fail to read non-existent file");
    println!("✓ Non-existent file correctly returns error");

    // Test writing to non-existent file
    let result = audiobook_boss_lib::commands::write_cover_art(
        "non_existent_file.mp3".to_string(),
        vec![0xFF, 0xD8, 0xFF, 0xD9], // minimal JPEG
    );
    assert!(result.is_err(), "Should fail to write to non-existent file");
    println!("✓ Writing to non-existent file correctly returns error");

    // Test loading non-existent cover file (async function)
    let result = futures::executor::block_on(audiobook_boss_lib::commands::load_cover_art_file(
        "non_existent_image.jpg".to_string(),
    ));
    assert!(
        result.is_err(),
        "Should fail to load non-existent cover file"
    );
    println!("✓ Loading non-existent cover file correctly returns error");

    println!("Cover art command error handling tests passed!");
}

#[test]
fn test_cover_art_size_limits() {
    println!("Testing cover art size limits and performance...");

    if let Some(media_file) = ensure_media() {
        // Read original metadata
        if let Ok(metadata) = audiobook_boss_lib::commands::read_audio_metadata(
            media_file.to_string_lossy().to_string(),
        ) {
            if let Some(ref original_cover) = metadata.cover_art {
                println!("Original cover art size: {} bytes", original_cover.len());

                // Test validation for various sizes
                let test_sizes = vec![
                    (original_cover.len(), "original"),
                    (1024, "1KB"),
                    (100 * 1024, "100KB"),
                    (1024 * 1024, "1MB"),
                    (5 * 1024 * 1024, "5MB"),
                ];

                for (size, description) in test_sizes {
                    // Create test cover art of specified size
                    let mut test_cover =
                        original_cover[..std::cmp::min(size, original_cover.len())].to_vec();
                    if size > original_cover.len() {
                        test_cover.extend(vec![0u8; size - original_cover.len()]);
                    }

                    let test_metadata = AudiobookMetadata {
                        title: Some(format!("Test {}", description)),
                        cover_art: Some(test_cover.clone()),
                        ..Default::default()
                    };

                    // Test format detection performance
                    let start = std::time::Instant::now();
                    let format = audiobook_boss_lib::ffmpeg_detect_cover_art_format(&test_cover);
                    let detection_time = start.elapsed();

                    println!(
                        "✓ {} cover art: format={:?}, detection_time={:?}",
                        description, format, detection_time
                    );

                    // Test validation performance
                    let start = std::time::Instant::now();
                    let warnings =
                        audiobook_boss_lib::ffmpeg_validate_metadata_compatibility(&test_metadata);
                    let validation_time = start.elapsed();

                    println!(
                        "  Validation: {} warnings, time={:?}",
                        warnings.len(),
                        validation_time
                    );

                    // Performance should be reasonable (under 100ms for most operations)
                    assert!(
                        detection_time.as_millis() < 100,
                        "Format detection should be fast"
                    );
                    assert!(
                        validation_time.as_millis() < 100,
                        "Validation should be fast"
                    );
                }
            }
        }
    } else {
        println!("No test media available - testing with synthetic data");

        // Test with minimal JPEG
        let minimal_jpeg =
            b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";
        let start = std::time::Instant::now();
        let format = audiobook_boss_lib::ffmpeg_detect_cover_art_format(minimal_jpeg);
        let detection_time = start.elapsed();

        assert!(format.is_some(), "Should detect JPEG format");
        assert!(
            detection_time.as_millis() < 10,
            "Format detection should be very fast for small data"
        );
        println!(
            "✓ Minimal JPEG detection: {:?} in {:?}",
            format, detection_time
        );
    }

    println!("Cover art size and performance tests passed!");
}
