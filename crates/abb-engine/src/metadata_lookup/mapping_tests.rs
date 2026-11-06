use super::*;
use crate::metadata_lookup::providers::audible::AudibleSearchResponse;

fn audnexus_reply() -> AudnexusBook {
    serde_json::from_str(include_str!("providers/fixtures/audnexus_book.json"))
        .expect("the Audnexus book reply parses")
}

fn audible_reply() -> AudibleSearchResponse {
    serde_json::from_str(include_str!("providers/fixtures/audible_search.json"))
        .expect("the Audible search reply parses")
}

#[test]
fn an_audnexus_book_reply_fills_the_lookup_fields() {
    let result = map_audnexus_book(audnexus_reply());

    assert_eq!(result.source, MetadataSource::Audnexus);
    assert_eq!(result.source_id, "B002V1OF70");
    assert_eq!(result.title, "Dune: Book One of the Dune Chronicles");
    assert_eq!(result.authors, ["Frank Herbert"]);
    assert_eq!(result.narrators, ["Scott Brick", "Orlagh Cassidy"]);
    assert_eq!(result.series.as_deref(), Some("Dune"));
    assert_eq!(result.series_part.as_deref(), Some("1"));
    assert_eq!(result.subseries.as_deref(), Some("The Dune Sequence"));
    assert_eq!(result.subseries_part.as_deref(), Some("12"));
    assert_eq!(
        result.description.as_deref(),
        Some("<p>The short summary.</p>")
    );
    assert_eq!(result.published_date.as_deref(), Some("2007-01"));
    assert_eq!(result.duration_seconds, Some(1262 * 60));
    assert_eq!(
        result.cover_url.as_deref(),
        Some("https://m.media-amazon.com/images/I/fixture-cover.jpg")
    );
    assert_eq!(result.audible_only, None);
}

#[test]
fn an_audible_search_reply_maps_the_fallback_result() {
    let mut products = audible_reply().products.into_iter();
    let result = map_audible_item(products.next().expect("first product"));

    assert_eq!(result.source, MetadataSource::Audnexus);
    assert_eq!(result.source_id, "B002V1OF70");
    assert_eq!(result.title, "Dune: Book One of the Dune Chronicles");
    assert_eq!(result.authors, ["Frank Herbert"]);
    assert_eq!(result.narrators, ["Scott Brick", "Orlagh Cassidy"]);
    assert_eq!(
        result.description.as_deref(),
        Some("<p>The publisher summary.</p>")
    );
    assert_eq!(result.published_date.as_deref(), Some("2007-01"));
    assert_eq!(result.duration_seconds, Some(1262 * 60));
    assert_eq!(
        result.cover_url.as_deref(),
        Some("https://m.media-amazon.com/images/I/fixture-cover._SL1024_.jpg")
    );
    assert_eq!(result.audible_only, Some(true));
}

#[test]
fn an_audible_fallback_result_keeps_the_series_the_reply_lists() {
    let result = map_audible_item(audible_reply().products.remove(0));

    assert_eq!(result.series.as_deref(), Some("Dune"));
    assert_eq!(result.series_part.as_deref(), Some("1"));
    assert_eq!(result.subseries.as_deref(), Some("The Dune Sequence"));
    assert_eq!(result.subseries_part.as_deref(), Some("12"));
}

#[test]
fn an_audible_product_with_only_its_title_maps_to_empty_fields() {
    let result = map_audible_item(audible_reply().products.remove(1));

    assert_eq!(result.title, "A Standalone Book");
    assert_eq!(
        result.description.as_deref(),
        Some("<p>Only a merchandising summary.</p>")
    );
    assert!(result.authors.is_empty());
    assert_eq!(result.series, None);
    assert_eq!(result.series_part, None);
    assert_eq!(result.cover_url, None);
    assert_eq!(result.duration_seconds, None);
}
