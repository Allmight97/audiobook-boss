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
fn automatic_lookup_searches_audnexus_and_open_library() {
    assert_eq!(
        LookupSource::Auto.sources(),
        [MetadataSource::Audnexus, MetadataSource::Openlibrary]
    );
    assert_eq!(LookupSource::Audnexus.sources(), [MetadataSource::Audnexus]);
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

#[test]
fn opening_with_several_titles_queues_them_and_opening_with_none_says_so() {
    let mut lookup = LookupState::default();
    let queued = |name: &str| QueuedTitle {
        title_id: name.to_string(),
        path: PathBuf::from(format!("/books/{name}.m4b")),
    };

    lookup.open(
        vec![queued("a"), queued("b")],
        Some(&stored(Some("A"), None)),
    );
    let snapshot = lookup.snapshot(1);
    assert!(snapshot.open && snapshot.is_queue_mode);
    assert_eq!(snapshot.apply_mode, LookupApplyMode::Queue);
    assert_eq!(snapshot.title_query, "A");
    assert_eq!(
        snapshot.queue_position.map(|position| position.total),
        Some(2)
    );

    lookup.open(Vec::new(), None);
    let snapshot = lookup.snapshot(2);
    assert_eq!(snapshot.status, Some(LookupStatus::NoValidTitle));
    assert_eq!(snapshot.apply_mode, LookupApplyMode::Current);
    assert!(snapshot.queue_position.is_none());
}
