use super::*;
use serde_json::json;

#[test]
fn a_library_page_reads_items_under_either_array_name() {
    for key in ["items", "products"] {
        let items = library_items_from_page(&json!({
            "response": { key: [
                {"asin": "B1", "title": "One"},
                {"sku": "S2"},
                {"title": "No id"}
            ]}
        }));
        assert_eq!(
            items
                .iter()
                .map(|item| item.title_id.as_str())
                .collect::<Vec<_>>(),
            vec!["B1", "S2"],
            "{key}"
        );
    }
}

#[test]
fn an_item_reads_people_cover_pdf_and_availability_facts() {
    let items = library_items_from_page(&json!({"items": [{
        "asin": "B1",
        "title": "One",
        "authors": [{"name": "A"}, "B"],
        "narrators": [{"name": "N"}],
        "productImages": {"240": "https://example.test/small.jpg", "500": "https://example.test/large.jpg"},
        "details": {"pdfUrl": "https://example.test/book.pdf"},
        "status": "Revoked",
        "is_playable": false,
        "is_ayce": true
    }]}));

    let item = &items[0];
    assert_eq!(item.title.as_deref(), Some("One"));
    assert_eq!(item.authors, vec!["A", "B"]);
    assert_eq!(item.narrators, vec!["N"]);
    assert_eq!(
        item.cover_url.as_deref(),
        Some("https://example.test/large.jpg")
    );
    assert!(item.supplemental_pdf_available);
    assert!(item.revoked);
    assert_eq!(item.playable, Some(false));
    assert_eq!(item.ayce, Some(true));
}

#[test]
fn image_url_beats_cover_url_beats_product_images() {
    let cover = |item: serde_json::Value| {
        library_items_from_page(&json!({"items": [item]}))[0]
            .cover_url
            .clone()
    };
    assert_eq!(
        cover(
            json!({"asin": "B", "cover_url": "c", "image_url": "i", "product_images": {"500": "p"}})
        ),
        Some("i".to_string())
    );
    assert_eq!(
        cover(json!({"asin": "B", "cover_url": "c", "product_images": {"500": "p"}})),
        Some("c".to_string())
    );
    assert_eq!(
        cover(json!({"asin": "B", "product_images": {"500": "p"}})),
        Some("p".to_string())
    );
}
