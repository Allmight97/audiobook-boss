use super::*;

#[test]
fn a_result_cover_uses_the_openlibrary_large_cover_address() {
    let doc = OpenLibraryDoc {
        key: "/works/OL1W".into(),
        title: "Feedback".into(),
        author_name: Vec::new(),
        cover_i: Some(8739161),
        first_publish_year: None,
        description: None,
    };
    assert_eq!(
        map_openlibrary_doc(doc).cover_url.as_deref(),
        Some("https://covers.openlibrary.org/b/id/8739161-L.jpg")
    );
}

#[test]
fn an_openlibrary_search_reply_maps_author_year_and_cover() {
    let reply: OpenLibrarySearchResponse =
        serde_json::from_str(include_str!("fixtures/openlibrary_search.json"))
            .expect("the OpenLibrary search reply parses");
    let results: Vec<_> = reply.docs.into_iter().map(map_openlibrary_doc).collect();

    assert_eq!(results.len(), 2);
    let dune = &results[0];
    assert_eq!(dune.source, MetadataSource::Openlibrary);
    assert_eq!(dune.source_id, "/works/OL893415W");
    assert_eq!(dune.title, "Dune");
    assert_eq!(dune.authors, ["Frank Herbert"]);
    assert_eq!(dune.published_date.as_deref(), Some("1965"));
    assert_eq!(
        dune.cover_url.as_deref(),
        Some("https://covers.openlibrary.org/b/id/12345-L.jpg")
    );
    assert_eq!(dune.audible_only, Some(false));

    let sparse = &results[1];
    assert!(sparse.authors.is_empty());
    assert_eq!(sparse.published_date, None);
    assert_eq!(sparse.cover_url, None);
}
