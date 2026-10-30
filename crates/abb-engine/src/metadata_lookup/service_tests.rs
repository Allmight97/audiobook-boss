use super::*;

fn make_result_with_authors(
    source: MetadataSource,
    source_id: &str,
    title: &str,
    authors: &[&str],
) -> OnlineMetadataResult {
    OnlineMetadataResult {
        source,
        source_id: source_id.to_string(),
        title: title.to_string(),
        authors: authors.iter().map(|author| author.to_string()).collect(),
        narrators: Vec::new(),
        series: None,
        series_part: None,
        subseries: None,
        subseries_part: None,
        description: None,
        published_date: None,
        duration_seconds: None,
        cover_url: None,
        audible_only: None,
    }
}

#[test]
fn extract_valid_audnexus_asin_requires_audnexus_selection() {
    let asin = "B01234ABCD";
    assert_eq!(
        extract_valid_audnexus_asin(asin, false),
        None,
        "OpenLibrary-only selection should not use Audnexus ASIN direct lookup"
    );
    assert_eq!(
        extract_valid_audnexus_asin(asin, true),
        Some(asin.to_string())
    );
}

#[test]
fn extract_valid_audnexus_asin_rejects_false_positive_tokens() {
    assert_eq!(
        extract_valid_audnexus_asin("Zen and the Art of Motorcycle Maintenance", true),
        None
    );
}

#[test]
fn all_selected_sources_failed_only_when_every_selected_source_fails() {
    assert!(all_selected_sources_failed(2, 2));
    assert!(all_selected_sources_failed(1, 1));
    assert!(!all_selected_sources_failed(2, 1));
    assert!(!all_selected_sources_failed(1, 0));
    assert!(!all_selected_sources_failed(0, 0));
}

#[test]
fn diagnostics_name_explicit_lookup_degradation_paths() {
    assert_eq!(
        asin_text_search_diagnostic().kind,
        MetadataLookupDiagnosticKind::AsinDirectLookupUnavailableTextSearchUsed
    );
    assert_eq!(
        source_failed_diagnostic(MetadataSource::Openlibrary).kind,
        MetadataLookupDiagnosticKind::SourceFailedPartialResults
    );
    assert_eq!(
        audible_only_diagnostic().kind,
        MetadataLookupDiagnosticKind::AudnexusDetailUnavailableAudibleOnlyResult
    );
}

#[test]
fn audible_only_diagnostic_is_reported_once_per_search() {
    let mut diagnostics = Vec::new();

    push_audible_only_diagnostic_once(&mut diagnostics);
    push_audible_only_diagnostic_once(&mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].kind,
        MetadataLookupDiagnosticKind::AudnexusDetailUnavailableAudibleOnlyResult
    );
}

#[test]
fn merge_search_results_keeps_audnexus_priority_and_respects_limit() {
    let audnexus_results = vec![
        make_result_with_authors(MetadataSource::Audnexus, "aud-1", "Alpha", &["Author One"]),
        make_result_with_authors(MetadataSource::Audnexus, "aud-2", "Beta", &["Author Two"]),
    ];
    let openlibrary_results = vec![
        make_result_with_authors(
            MetadataSource::Openlibrary,
            "ol-duplicate",
            "Alpha",
            &["Author One"],
        ),
        make_result_with_authors(
            MetadataSource::Openlibrary,
            "ol-3",
            "Gamma",
            &["Author Three"],
        ),
    ];

    let merged = merge_search_results(audnexus_results, openlibrary_results, 3);

    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].source, MetadataSource::Audnexus);
    assert_eq!(merged[0].title, "Alpha");
    assert_eq!(merged[1].source, MetadataSource::Audnexus);
    assert_eq!(merged[1].title, "Beta");
    assert_eq!(merged[2].source, MetadataSource::Openlibrary);
    assert_eq!(merged[2].title, "Gamma");
}

#[test]
fn merge_search_results_keeps_openlibrary_variant_when_authors_differ() {
    let audnexus_results = vec![make_result_with_authors(
        MetadataSource::Audnexus,
        "aud-1",
        "Alpha",
        &["Author One"],
    )];
    let openlibrary_results = vec![make_result_with_authors(
        MetadataSource::Openlibrary,
        "ol-1",
        "Alpha",
        &["Different Author"],
    )];

    let merged = merge_search_results(audnexus_results, openlibrary_results, 8);

    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].source, MetadataSource::Audnexus);
    assert_eq!(merged[1].source, MetadataSource::Openlibrary);
    assert_eq!(merged[1].title, "Alpha");
}
