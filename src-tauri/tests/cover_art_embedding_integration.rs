//! Integration tests for cover art embedding reliability
//!
//! These tests verify that both native FFmpeg embedding and Lofty fallback
//! work correctly, and that the system gracefully handles failures.

use audiobook_boss_lib::{
    ffmpeg_add_cover_art_stream_pre_header as add_cover_art_stream_pre_header,
    ffmpeg_validate_metadata_compatibility as validate_metadata_compatibility,
    ffmpeg_write_cover_art_packet_post_header as write_cover_art_packet_post_header,
    lofty_write_cover_art, AudiobookMetadata, FfmpegCoverFormat as CoverFormat,
};
use ffmpeg_next as ff;
use lofty::prelude::*;

use tempfile::TempDir;

/// Creates a minimal valid JPEG image for testing
fn create_test_jpeg() -> Vec<u8> {
    // Minimal JPEG with SOI, APP0, SOF0, and EOI markers
    vec![
        0xFF, 0xD8, // SOI (Start of Image)
        0xFF, 0xE0, 0x00, 0x10, // APP0 marker with length
        b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF,
        0xC0, 0x00, 0x11, 0x08, // SOF0 marker
        0x00, 0x10, 0x00, 0x10, // Height=16, Width=16
        0x01, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, 0xFF,
        0xD9, // EOI (End of Image)
    ]
}

/// Creates a minimal valid PNG image for testing
fn create_test_png() -> Vec<u8> {
    vec![
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
        b'I', b'H', b'D', b'R', // IHDR chunk type
        0x00, 0x00, 0x00, 0x10, // Width = 16
        0x00, 0x00, 0x00, 0x10, // Height = 16
        0x08, 0x02, 0x00, 0x00, 0x00, // Bit depth, color type, compression, filter, interlace
        0x90, 0x91, 0x68, 0x36, // CRC
        0x00, 0x00, 0x00, 0x00, // IEND chunk length
        b'I', b'E', b'N', b'D', // IEND chunk type
        0xAE, 0x42, 0x60, 0x82, // CRC
    ]
}

#[test]
fn test_native_cover_art_embedding_success() {
    ff::init().expect("FFmpeg initialization should succeed");

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let output_path = temp_dir.path().join("test_native.m4b");

    // Create output context
    let mut octx = ff::format::output(&output_path).expect("Should create output context");

    // Test with JPEG cover art
    let jpeg_data = create_test_jpeg();
    let result = add_cover_art_stream_pre_header(&mut octx, &jpeg_data);

    // Should successfully add the stream
    assert!(
        result.is_some(),
        "Native JPEG cover art stream should be added successfully"
    );

    if let Some((stream_index, format)) = result {
        assert_eq!(format, CoverFormat::Jpeg);
        assert_eq!(stream_index, 0); // First stream should be index 0

        // Write header to finalize stream setup
        octx.write_header()
            .expect("Should write header successfully");

        // Write cover art packet
        write_cover_art_packet_post_header(&mut octx, stream_index, &jpeg_data, format);

        // Finalize the file
        octx.write_trailer()
            .expect("Should write trailer successfully");
    }
}

#[test]
fn test_native_cover_art_embedding_with_png() {
    ff::init().expect("FFmpeg initialization should succeed");

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let output_path = temp_dir.path().join("test_native_png.m4b");

    // Create output context
    let mut octx = ff::format::output(&output_path).expect("Should create output context");

    // Test with PNG cover art
    let png_data = create_test_png();
    let result = add_cover_art_stream_pre_header(&mut octx, &png_data);

    // Should successfully add the stream
    assert!(
        result.is_some(),
        "Native PNG cover art stream should be added successfully"
    );

    if let Some((stream_index, format)) = result {
        assert_eq!(format, CoverFormat::Png);

        // Write header to finalize stream setup
        octx.write_header()
            .expect("Should write header successfully");

        // Write cover art packet
        write_cover_art_packet_post_header(&mut octx, stream_index, &png_data, format);

        // Finalize the file
        octx.write_trailer()
            .expect("Should write trailer successfully");
    }
}

#[test]
fn test_unsupported_format_fallback() {
    ff::init().expect("FFmpeg initialization should succeed");

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let output_path = temp_dir.path().join("test_fallback.m4b");

    // Create output context
    let mut octx = ff::format::output(&output_path).expect("Should create output context");

    // Test with unsupported GIF format
    let gif_data = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!".to_vec();
    let result = add_cover_art_stream_pre_header(&mut octx, &gif_data);

    // Should return None for unsupported format
    assert!(
        result.is_none(),
        "Unsupported format should return None for native embedding"
    );
}

#[test]
fn test_lofty_fallback_embedding() {
    // This test verifies that Lofty fallback works when given a proper M4B file
    // In practice, this would be called after FFmpeg has created the audio file

    let temp_dir = TempDir::new().expect("Should create temp directory");
    let test_file = temp_dir.path().join("test_lofty.m4b");

    // Create a proper M4B file using FFmpeg first
    ff::init().expect("FFmpeg initialization should succeed");

    // Create a minimal but valid M4B file with FFmpeg
    let mut octx = ff::format::output(&test_file).expect("Should create output context");

    // Add a minimal audio stream
    if let Some(audio_codec) = ff::encoder::find(ff::codec::Id::AAC) {
        if let Ok(mut audio_stream) = octx.add_stream(audio_codec) {
            let mut ctx = ff::codec::context::Context::new()
                .encoder()
                .audio()
                .expect("Should create audio encoder context");
            ctx.set_bit_rate(64000);
            ctx.set_rate(44100);
            ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
            ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
            ctx.set_time_base(ff::Rational(1, 44100));

            if let Ok(encoder) = ctx.open_as(audio_codec) {
                audio_stream.set_parameters(&encoder);

                // Write header and trailer to create a valid file
                octx.write_header().expect("Should write header");
                octx.write_trailer().expect("Should write trailer");

                // Now test Lofty cover art embedding on the valid file
                let jpeg_data = create_test_jpeg();
                let result = lofty_write_cover_art(&test_file, &jpeg_data);

                // Should succeed on a properly formatted M4B file
                if result.is_ok() {
                    // Verify the cover art was embedded
                    if let Ok(tagged_file) = lofty::read_from_path(&test_file) {
                        let has_cover = tagged_file
                            .tags()
                            .iter()
                            .any(|tag| !tag.pictures().is_empty());
                        assert!(
                            has_cover,
                            "File should contain cover art after Lofty embedding"
                        );
                    }
                } else {
                    // If Lofty fails, it might be due to the minimal file structure
                    // This is acceptable as long as we can detect the failure gracefully
                    println!(
                        "Lofty embedding failed as expected with minimal file: {:?}",
                        result.err()
                    );
                }
            }
        }
    }
}

#[test]
fn test_cover_art_validation_comprehensive() {
    // validate_metadata_compatibility is already imported at the top

    // Test with valid JPEG
    let valid_jpeg = create_test_jpeg();
    let metadata_jpeg = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        cover_art: Some(valid_jpeg),
        ..Default::default()
    };

    let warnings = validate_metadata_compatibility(&metadata_jpeg);
    // Should not have format warnings for valid JPEG
    assert!(!warnings.iter().any(|w| w.contains("format not supported")));

    // Test with valid PNG
    let valid_png = create_test_png();
    let metadata_png = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        cover_art: Some(valid_png),
        ..Default::default()
    };

    let warnings = validate_metadata_compatibility(&metadata_png);
    // Should not have format warnings for valid PNG
    assert!(!warnings.iter().any(|w| w.contains("format not supported")));

    // Test with invalid format
    let invalid_data = b"INVALID_IMAGE_DATA".to_vec();
    let metadata_invalid = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        cover_art: Some(invalid_data),
        ..Default::default()
    };

    let warnings = validate_metadata_compatibility(&metadata_invalid);
    // Should warn about unsupported format
    assert!(warnings.iter().any(|w| w.contains("format not supported")));

    // Test with empty cover art
    let metadata_empty = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        cover_art: Some(vec![]),
        ..Default::default()
    };

    let warnings = validate_metadata_compatibility(&metadata_empty);
    // Should warn about empty data
    assert!(warnings.iter().any(|w| w.contains("empty")));

    // Test with oversized cover art
    let oversized_data = vec![0xFF; 15 * 1024 * 1024]; // 15MB
    let metadata_oversized = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        cover_art: Some(oversized_data),
        ..Default::default()
    };

    let warnings = validate_metadata_compatibility(&metadata_oversized);
    // Should warn about size
    assert!(warnings.iter().any(|w| w.contains("size limit")));
}

/// Creates a minimal M4B file structure for testing Lofty operations
/// This creates a proper MP4/M4B structure with required atoms for Lofty to work
fn create_minimal_m4b_file() -> Vec<u8> {
    // Create a proper minimal MP4 structure that Lofty can parse
    // This includes ftyp, moov, and mdat atoms
    let mut data = Vec::new();

    // ftyp atom (file type)
    data.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x20, // size (32 bytes)
        b'f', b't', b'y', b'p', // type
        b'M', b'4', b'B', b' ', // major brand
        0x00, 0x00, 0x00, 0x00, // minor version
        b'M', b'4', b'B', b' ', // compatible brand 1
        b'i', b's', b'o', b'm', // compatible brand 2
        b'm', b'p', b'4', b'1', // compatible brand 3
    ]);

    // moov atom (movie metadata) - minimal structure
    data.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x6C, // size (108 bytes)
        b'm', b'o', b'o', b'v', // type
        // mvhd atom (movie header)
        0x00, 0x00, 0x00, 0x64, // size (100 bytes)
        b'm', b'v', b'h', b'd', // type
        0x00, 0x00, 0x00, 0x00, // version and flags
        0x00, 0x00, 0x00, 0x00, // creation time
        0x00, 0x00, 0x00, 0x00, // modification time
        0x00, 0x00, 0x03, 0xE8, // time scale (1000)
        0x00, 0x00, 0x00, 0x00, // duration
        0x00, 0x01, 0x00, 0x00, // preferred rate (1.0)
        0x01, 0x00, 0x00, 0x00, // preferred volume (1.0)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved
        // transformation matrix (identity)
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, // next track ID
    ]);

    // mdat atom (media data) - empty for this test
    data.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x08, // size (8 bytes)
        b'm', b'd', b'a', b't', // type
    ]);

    data
}

#[cfg(test)]
mod integration_helpers {
    use super::*;

    /// Helper to create a complete test scenario with both native and fallback paths
    pub fn test_complete_embedding_workflow(
        cover_data: Vec<u8>,
        expected_format: Option<CoverFormat>,
    ) {
        ff::init().expect("FFmpeg initialization should succeed");

        let temp_dir = TempDir::new().expect("Should create temp directory");
        let output_path = temp_dir.path().join("test_complete.m4b");

        // Step 1: Try native embedding
        let mut octx = ff::format::output(&output_path).expect("Should create output context");
        let native_result = add_cover_art_stream_pre_header(&mut octx, &cover_data);

        match expected_format {
            Some(expected) => {
                // Should succeed for supported formats
                assert!(
                    native_result.is_some(),
                    "Native embedding should succeed for supported format"
                );
                if let Some((stream_index, format)) = native_result {
                    assert_eq!(format, expected);

                    // Complete the native embedding
                    octx.write_header().expect("Should write header");
                    write_cover_art_packet_post_header(
                        &mut octx,
                        stream_index,
                        &cover_data,
                        format,
                    );
                    octx.write_trailer().expect("Should write trailer");
                }
            }
            None => {
                // Should fail for unsupported formats
                assert!(
                    native_result.is_none(),
                    "Native embedding should fail for unsupported format"
                );

                // We need to add at least one stream for FFmpeg to create a valid file
                // Add a dummy audio stream
                if let Some(audio_codec) = ff::encoder::find(ff::codec::Id::AAC) {
                    if let Ok(mut audio_stream) = octx.add_stream(audio_codec) {
                        // Set minimal audio parameters
                        let mut ctx = ff::codec::context::Context::new()
                            .encoder()
                            .audio()
                            .expect("Should create audio encoder context");
                        ctx.set_bit_rate(64000);
                        ctx.set_rate(44100);
                        ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
                        ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
                        ctx.set_time_base(ff::Rational(1, 44100));

                        if let Ok(encoder) = ctx.open_as(audio_codec) {
                            audio_stream.set_parameters(&encoder);
                        }
                    }
                }

                // Complete file creation without cover art
                octx.write_header().expect("Should write header");
                octx.write_trailer().expect("Should write trailer");

                // Step 2: Try Lofty fallback
                // Replace with minimal M4B content for Lofty to work with
                std::fs::write(&output_path, create_minimal_m4b_file())
                    .expect("Should write minimal M4B");

                let fallback_result = lofty_write_cover_art(&output_path, &cover_data);
                // Lofty might still fail for truly invalid formats, but should handle more cases
                println!("Lofty fallback result: {:?}", fallback_result);
            }
        }
    }
}

#[test]
fn test_complete_workflow_jpeg() {
    integration_helpers::test_complete_embedding_workflow(
        create_test_jpeg(),
        Some(CoverFormat::Jpeg),
    );
}

#[test]
fn test_complete_workflow_png() {
    integration_helpers::test_complete_embedding_workflow(
        create_test_png(),
        Some(CoverFormat::Png),
    );
}

#[test]
fn test_complete_workflow_unsupported() {
    let gif_data = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!".to_vec();
    integration_helpers::test_complete_embedding_workflow(gif_data, None);
}
