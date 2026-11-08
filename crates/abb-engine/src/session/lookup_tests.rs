use super::*;

fn stored(title: Option<&str>, album: Option<&str>) -> AudiobookMetadata {
    AudiobookMetadata {
        title: title.map(str::to_string),
        album: album.map(str::to_string),
        ..Default::default()
    }
}

#[test]
fn the_title_query_prefers_the_book_over_a_track_name() {
    let path = Path::new("/books/01 - Some_Book.part.m4b");
    let query = |stored: &AudiobookMetadata| title_query(Some(stored), path);

    assert_eq!(query(&stored(Some(" Dune "), Some("Other"))), "Dune");
    assert_eq!(query(&stored(Some("Chapter 12"), Some("Dune"))), "Dune");
    assert_eq!(query(&stored(Some("disc3"), Some("Dune"))), "Dune");
    assert_eq!(query(&stored(Some("Chapter 12"), None)), "Chapter 12");
    assert_eq!(query(&stored(None, Some("Dune"))), "Dune");
    assert_eq!(
        query(&stored(Some("Chapterhouse"), Some("Dune"))),
        "Chapterhouse"
    );
    assert_eq!(title_query(None, path), "Some Book part");
    assert_eq!(
        title_query(Some(&stored(None, None)), path),
        "Some Book part"
    );
}

#[test]
fn the_author_query_falls_back_from_artist_to_narrator() {
    let both = AudiobookMetadata {
        artist: Some(" Frank Herbert ".to_string()),
        composer: Some("Narrator".to_string()),
        ..Default::default()
    };
    let narrator_only = AudiobookMetadata {
        artist: Some("  ".to_string()),
        composer: Some("Narrator".to_string()),
        ..Default::default()
    };

    assert_eq!(author_query(Some(&both)), "Frank Herbert");
    assert_eq!(author_query(Some(&narrator_only)), "Narrator");
    assert_eq!(author_query(None), "");
}

#[test]
fn a_result_supplies_only_the_fields_it_has() {
    let result = OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: "B00".to_string(),
        title: "Dune".to_string(),
        authors: vec!["Frank Herbert".to_string(), "Other".to_string()],
        narrators: Vec::new(),
        series: Some("Dune Saga".to_string()),
        series_part: Some("1".to_string()),
        subseries: None,
        subseries_part: None,
        description: None,
        published_date: Some("1965".to_string()),
        duration_seconds: None,
        cover_url: None,
        cover: None,
        audible_only: None,
    };

    assert_eq!(
        result_metadata(&result),
        AudiobookMetadata {
            title: Some("Dune".to_string()),
            album: Some("Dune".to_string()),
            artist: Some("Frank Herbert, Other".to_string()),
            series: Some("Dune Saga".to_string()),
            series_part: Some("1".to_string()),
            date: Some("1965".to_string()),
            ..Default::default()
        }
    );
}
