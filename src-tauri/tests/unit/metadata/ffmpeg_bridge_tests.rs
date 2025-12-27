use audiobook_boss_lib::metadata::{AudiobookMetadata, ffmpeg_bridge};

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
