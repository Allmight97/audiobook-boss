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
