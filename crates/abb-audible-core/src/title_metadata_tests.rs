use super::*;
use serde_json::json;

#[test]
fn title_metadata_reads_the_title_and_either_pdf_alias() {
    for key in ["pdf_url", "pdfUrl"] {
        let metadata = title_metadata_from_value(&json!({
            "product": { "title": "Book", key: "https://example.test/book.pdf" }
        }));
        assert_eq!(metadata.title.as_deref(), Some("Book"), "{key}");
        assert_eq!(
            metadata.supplemental_pdf_url.as_deref(),
            Some("https://example.test/book.pdf"),
            "{key}"
        );
    }
}

#[test]
fn a_blank_title_and_an_empty_pdf_url_read_as_absent() {
    let metadata = title_metadata_from_value(&json!({"title": "  ", "pdf_url": ""}));
    assert_eq!(metadata.title, None);
    assert_eq!(metadata.supplemental_pdf_url, None);
}
