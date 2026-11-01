use super::*;
use serde_json::json;

#[test]
fn parse_library_titles_extracts_pdf_availability_without_raw_payload_leak() {
    let payload = json!({
        "items": [
            {
                "asin": "B000000001",
                "title": "Remote Book",
                "authors": [{"name": "Author One"}],
                "narrators": [{"name": "Narrator One"}],
                "runtime_length_min": 90,
                "product_images": {"500": "https://example.test/cover.jpg"},
                "details": {"pdf_url": "https://example.test/supplement.pdf"}
            }
        ]
    });

    let titles = parse_library_titles(&payload);

    assert_eq!(titles.len(), 1);
    let title = &titles[0];
    assert_eq!(title.provider_id, ProviderId::Audible);
    assert_eq!(title.title_id, "B000000001");
    assert_eq!(title.authors, vec!["Author One"]);
    assert_eq!(title.narrators, vec!["Narrator One"]);
    assert_eq!(title.duration_seconds, Some(5_400));
    assert_eq!(
        title.cover_url.as_deref(),
        Some("https://example.test/cover.jpg")
    );
    assert!(title.supplemental_pdf_available);
    assert_eq!(
        title.availability.status,
        RemoteTitleAvailabilityStatus::Available
    );
    assert!(title.availability.acquirable);
    assert!(title.unsupported_reasons.is_empty());
}

#[test]
fn parse_library_titles_marks_non_playable_library_items_unsupported() {
    let payload = json!({
        "items": [
            {
                "asin": "B000000001",
                "title": "Subscription Visible Book",
                "is_playable": false,
                "is_listenable": true,
                "is_ayce": true
            }
        ]
    });

    let titles = parse_library_titles(&payload);

    assert_eq!(titles.len(), 1);
    assert_eq!(
        titles[0].unsupported_reasons,
        vec![RemoteAcquisitionFailureKind::ProtectedUnsupported]
    );
    assert_eq!(
        titles[0].availability.status,
        RemoteTitleAvailabilityStatus::CatalogOnly
    );
    assert!(!titles[0].availability.acquirable);
    assert_eq!(titles[0].availability.label, "Audible catalog title");
}

#[test]
fn parse_library_titles_marks_revoked_items_as_returned_or_refunded() {
    let payload = json!({
        "items": [
            {
                "asin": "B000000001",
                "title": "Returned Book",
                "status": "Revoked",
                "is_playable": false
            }
        ]
    });

    let titles = parse_library_titles(&payload);

    assert_eq!(titles.len(), 1);
    assert_eq!(
        titles[0].availability.status,
        RemoteTitleAvailabilityStatus::Revoked
    );
    assert_eq!(titles[0].availability.label, "Returned/refunded in Audible");
}
