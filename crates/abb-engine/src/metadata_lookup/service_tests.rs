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
        cover: None,
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

mod through_providers {
    use super::*;
    use crate::remote_source::test_support::{Reply, StubServer};

    const OK: &str = "200 OK";
    const MISSING: &str = "404 Not Found";
    const DOWN: &str = "503 Service Unavailable";
    const AUDNEXUS_BOOK: &str = include_str!("providers/fixtures/audnexus_book.json");
    const AUDIBLE_SEARCH: &str = include_str!("providers/fixtures/audible_search.json");
    const OPENLIBRARY_SEARCH: &str = include_str!("providers/fixtures/openlibrary_search.json");

    type Answer = (&'static str, &'static str);

    async fn stubbed(
        answer: impl Fn(&str) -> Answer + Send + 'static,
    ) -> (StubServer, ProviderEndpoints) {
        let server = StubServer::start_routed(move |_, request| {
            let (status, body) = answer(&request.path_and_query);
            Reply::json(status, body)
        })
        .await;
        let endpoints = ProviderEndpoints::at(server.url(), server.url(), server.url());
        (server, endpoints)
    }

    fn healthy(path: &str) -> Answer {
        if path.starts_with("/books/") {
            (OK, AUDNEXUS_BOOK)
        } else if path.starts_with("/1.0/catalog/products") {
            (OK, AUDIBLE_SEARCH)
        } else {
            (OK, OPENLIBRARY_SEARCH)
        }
    }

    fn kinds(response: &MetadataLookupResponse) -> Vec<MetadataLookupDiagnosticKind> {
        response
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.kind.clone())
            .collect()
    }

    #[tokio::test]
    async fn an_asin_audnexus_knows_is_the_only_result_and_no_text_search_runs() {
        let (server, endpoints) = stubbed(healthy).await;

        let response = search_providers(&endpoints, "B002V1OF70".to_string(), None, None)
            .await
            .expect("lookup");

        assert_eq!(response.results.len(), 1);
        assert!(response.results[0].title.starts_with("Dune"));
        assert!(response.diagnostics.is_empty());
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].path_and_query.starts_with("/books/B002V1OF70"));
    }

    #[tokio::test]
    async fn an_asin_audnexus_cannot_find_falls_back_to_text_search_and_says_so() {
        let (_server, endpoints) = stubbed(|path| {
            if path.starts_with("/books/B01234ABCD") {
                (MISSING, "{}")
            } else {
                healthy(path)
            }
        })
        .await;

        let response = search_providers(&endpoints, "B01234ABCD".to_string(), None, None)
            .await
            .expect("lookup");

        assert_eq!(
            kinds(&response),
            [MetadataLookupDiagnosticKind::AsinDirectLookupUnavailableTextSearchUsed]
        );
        assert!(response
            .results
            .iter()
            .any(|result| result.source == MetadataSource::Audnexus
                && result.title.starts_with("Dune")));
    }

    #[tokio::test]
    async fn one_failing_source_leaves_the_other_results_and_a_diagnostic() {
        let (_server, endpoints) = stubbed(|path| {
            if path.starts_with("/search.json") {
                (DOWN, "{}")
            } else {
                healthy(path)
            }
        })
        .await;

        let response = search_providers(&endpoints, "Dune Herbert".to_string(), None, None)
            .await
            .expect("lookup");

        assert!(!response.results.is_empty());
        assert!(response
            .results
            .iter()
            .all(|result| result.source == MetadataSource::Audnexus));
        assert_eq!(
            response.diagnostics,
            [source_failed_diagnostic(MetadataSource::Openlibrary)]
        );
    }

    #[tokio::test]
    async fn every_selected_source_failing_fails_the_search() {
        let (_server, endpoints) = stubbed(|_| (DOWN, "{}")).await;

        let error = search_providers(&endpoints, "Dune Herbert".to_string(), None, None)
            .await
            .expect_err("all sources down");

        assert!(
            error
                .to_string()
                .contains("All selected metadata sources failed"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn an_audible_hit_audnexus_cannot_enrich_is_kept_and_marked_audible_only() {
        let (_server, endpoints) = stubbed(|path| {
            if path.starts_with("/books/") {
                (MISSING, "{}")
            } else {
                healthy(path)
            }
        })
        .await;

        let response = search_providers(
            &endpoints,
            "Dune Herbert".to_string(),
            Some(vec![MetadataSource::Audnexus]),
            None,
        )
        .await
        .expect("lookup");

        assert!(response
            .results
            .iter()
            .all(|result| result.audible_only == Some(true)));
        assert_eq!(
            kinds(&response),
            [MetadataLookupDiagnosticKind::AudnexusDetailUnavailableAudibleOnlyResult]
        );
    }
}
