use audiobook_boss_lib::metadata::{ffmpeg_bridge, AudiobookMetadata};
use ffmpeg_next as ff;
use tempfile::TempDir;

// Focused unit tests for ffmpeg_bridge helpers (distinct from integration tests)

#[test]
fn test_metadata_to_ffmpeg_dict_minimal() {
    let md = AudiobookMetadata { title: Some("Title".into()), ..Default::default() };
    let dict = ffmpeg_bridge::metadata_to_ffmpeg_dict(&md).expect("conversion");
    assert_eq!(dict.get("title").map(|s| s.to_string()), Some("Title".into()));
    // media_type should always be present
    assert_eq!(dict.get("media_type").map(|s| s.to_string()), Some("2".into()));
}

#[test]
fn test_metadata_to_ffmpeg_dict_full_fields() {
    let md = AudiobookMetadata {
        title: Some("Book".into()),
        artist: Some("Author".into()),
        album: Some("Series".into()),
        composer: Some("Narrator".into()),
        genre: Some("Audiobook".into()),
        date: Some(2025),
        comment: Some("A comment".into()),
        description: Some("Long description".into()),
        series: Some("Series Name".into()),
        series_part: Some("2".into()),
        ..Default::default()
    };
    let dict = ffmpeg_bridge::metadata_to_ffmpeg_dict(&md).expect("conversion");
    for key in ["title","artist","album","composer","genre","date","year","comment","description","media_type"] { 
        assert!(dict.get(key).is_some(), "Missing key {key}");
    }
    for key in [
        "series",
        "series-part",
        "----:com.apple.iTunes:SERIES",
        "----:com.apple.iTunes:SERIES-PART",
        "MVNM",
        "MVIN",
    ] {
        assert!(dict.get(key).is_some(), "Missing key {key}");
    }
    // album_artist mirror
    assert_eq!(dict.get("album_artist").map(|s| s.to_string()), Some("Author".into()));
}

#[test]
fn test_validate_metadata_compatibility_warnings() {
    let md = AudiobookMetadata {
        track: Some((1, Some(10))),
        disk: Some((1, None)),
        cover_art: Some(vec![0u8; 11 * 1024 * 1024]), // 11MB triggers size warning (>10MB)
        ..Default::default()
    };
    let warnings = ffmpeg_bridge::validate_metadata_compatibility(&md);
    assert!(warnings.iter().any(|w| w.contains("Track number")));
    assert!(warnings.iter().any(|w| w.contains("Disk number")));
    assert!(warnings.iter().any(|w| w.contains("exceeds recommended size")));
}

#[test]
fn test_metadata_conversion() {
    let metadata = AudiobookMetadata {
        title: Some("Test Book".to_string()),
        artist: Some("Test Author".to_string()),
        album: Some("Test Series".to_string()),
        genre: Some("Audiobook".to_string()),
        date: Some(2025),
        description: Some("Test description".to_string()),
        ..Default::default()
    };

    let dict = ffmpeg_bridge::metadata_to_ffmpeg_dict(&metadata).expect("Metadata conversion should work");

    // Verify essential fields are present
    assert!(dict.get("title").is_some());
    assert!(dict.get("artist").is_some());
    assert!(dict.get("album").is_some());
    assert!(dict.get("media_type").is_some());
}

#[test]
fn test_metadata_validation() {
    let metadata = AudiobookMetadata {
        title: Some("Test".to_string()),
        track: Some((1, Some(12))),
        cover_art: Some(vec![0u8; 15 * 1024 * 1024]), // 15MB - too large
        ..Default::default()
    };

    let warnings = ffmpeg_bridge::validate_metadata_compatibility(&metadata);
    assert!(warnings.len() >= 2); // Should warn about track and cover art size
}

#[test]
fn test_detect_cover_art_format() {
    let jpeg = b\"\\xFF\\xD8\\xFFrest\"; // minimal marker
    let png = b\"\\x89PNG\\r\\n\\x1A\\nrest\"; // minimal signature
    let bad = b\"GIF89a\";
    assert_eq!(
        ffmpeg_bridge::detect_cover_art_format(jpeg),
        Some(ffmpeg_bridge::CoverFormat::Jpeg)
    );
    assert_eq!(
        ffmpeg_bridge::detect_cover_art_format(png),
        Some(ffmpeg_bridge::CoverFormat::Png)
    );
    assert_eq!(ffmpeg_bridge::detect_cover_art_format(bad), None);
}

#[test]
fn test_add_cover_art_stream_pre_header_supported_and_unsupported() {
    ff::init().expect("ffmpeg init");
    let temp = TempDir::new().expect("temp");
    let output = temp.path().join("test.m4b");
    let mut octx = ff::format::output(&output).expect("create output");

    // Unsupported should return None
    let unsupported = b"GIF89a"; // triggers log warning path
    assert!(ffmpeg_bridge::add_cover_art_stream_pre_header(&mut octx, unsupported).is_none());

    // Supported JPEG returns Some
    let jpeg = b\"\\xFF\\xD8\\xFF\\xE0data\";
    let added = ffmpeg_bridge::add_cover_art_stream_pre_header(&mut octx, jpeg);
    assert!(matches!(added, Some((_idx, ffmpeg_bridge::CoverFormat::Jpeg))));
}

#[test]
fn test_validate_metadata_compatibility_dimension_warnings() {
    let small_jpeg = b\"\\xFF\\xD8\\xFF\\xC0\\x00\\x11\\x08\\x00\\x32\\x00\\x32\\x01\\x01\\x11\\x00\\x02\\x11\\x01\\x03\\x11\\x01\";
    let metadata_small = AudiobookMetadata {
        title: Some("Test".to_string()),
        cover_art: Some(small_jpeg.to_vec()),
        ..Default::default()
    };

    let warnings_small = ffmpeg_bridge::validate_metadata_compatibility(&metadata_small);
    assert!(warnings_small.iter().any(|w| w.contains("very small")));

    let large_png = b\"\\x89PNG\\r\\n\\x1A\\n\\x00\\x00\\x00\\rIHDR\\x00\\x00\\x0B\\xB9\\x00\\x00\\x0B\\xBA\\x08\\x02\\x00\\x00\\x00\";
    let metadata_large = AudiobookMetadata {
        title: Some("Test".to_string()),
        cover_art: Some(large_png.to_vec()),
        ..Default::default()
    };

    let warnings_large = ffmpeg_bridge::validate_metadata_compatibility(&metadata_large);
    assert!(warnings_large.iter().any(|w| w.contains("very large")));
}

#[test]
fn test_validate_metadata_compatibility_enhanced() {
    // Test with valid JPEG cover art
    let valid_jpeg =
        b\"\\xFF\\xD8\\xFF\\xE0\\x00\\x10JFIF\\x00\\x01\\x01\\x00\\x00\\x01\\x00\\x01\\x00\\x00\\xFF\\xD9\";
    let metadata_with_jpeg = AudiobookMetadata {
        title: Some("Test".to_string()),
        cover_art: Some(valid_jpeg.to_vec()),
        ..Default::default()
    };

    let warnings = ffmpeg_bridge::validate_metadata_compatibility(&metadata_with_jpeg);
    // Should not warn about format since JPEG is supported
    assert!(!warnings.iter().any(|w| w.contains("format not supported")));

    // Test with unsupported format
    let gif_data = b\"GIF89a\\x01\\x00\\x01\\x00\\x00\\x00\\x00!\";
    let metadata_with_gif = AudiobookMetadata {
        title: Some("Test".to_string()),
        cover_art: Some(gif_data.to_vec()),
        ..Default::default()
    };

    let warnings = ffmpeg_bridge::validate_metadata_compatibility(&metadata_with_gif);
    assert!(warnings.iter().any(|w| w.contains("format not supported")));

    // Test with empty cover art
    let metadata_empty_cover = AudiobookMetadata {
        title: Some("Test".to_string()),
        cover_art: Some(vec![]),
        ..Default::default()
    };

    let warnings = ffmpeg_bridge::validate_metadata_compatibility(&metadata_empty_cover);
    assert!(warnings.iter().any(|w| w.contains("empty")));
}
