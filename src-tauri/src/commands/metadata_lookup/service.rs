//! Canonical metadata lookup service.
//!
//! External metadata providers are inherently partial, unavailable, and uneven.
//! This module implements resilient provider aggregation with three explicit
//! degraded-mode behaviors, each with a trigger, observable diagnostic, and
//! rationale:
//!
//! 1. **ASIN detail unavailable → text search used**: When a query contains a valid
//!    ASIN, `search_online_metadata` tries `fetch_audnexus_book` first. If that
//!    precise route fails, it continues through the normal selected-provider search
//!    path and emits an `AsinDirectLookupUnavailableTextSearchUsed` diagnostic.
//!    Rationale: precise lookup is preferred, but graceful degradation keeps the
//!    feature useful when the provider endpoint is down.
//!
//! 2. **Selected source failed → partial results**: `collect_provider_searches`
//!    attempts each selected provider independently. If at least one returns
//!    usable results, the command succeeds with those results and a
//!    `SourceFailedPartialResults` diagnostic for each failed source. Terminal
//!    failure only occurs when *all* selected sources fail.
//!    Rationale: multi-provider resilience; discarding valid results because one
//!    provider is down is worse UX.
//!
//! 3. **Audnexus detail unavailable → Audible-only provenance**: Audnexus search
//!    uses Audible catalog hits plus Audnexus detail enrichment. When detail
//!    enrichment is unavailable for a specific hit, the Audible-derived result is
//!    returned with `audible_only: true` and an
//!    `AudnexusDetailUnavailableAudibleOnlyResult` diagnostic.
//!    Rationale: honest provenance; we keep the hit but mark it as unenriched.
//!
//! These behaviors are intentional architecture, not temporary workarounds.

use std::time::Duration;

use crate::errors::{AppError, Result};
use reqwest::Client;

use super::mapping::map_audible_item;
use super::parse::{extract_asin, extract_region_override, strip_region_overrides};
use super::providers::audible::fetch_audible_search;
use super::providers::audnexus::fetch_audnexus_book;
use super::providers::openlibrary::fetch_openlibrary_search;
use super::types::{
    MetadataLookupDiagnostic, MetadataLookupDiagnosticKind, MetadataLookupResponse, MetadataSource,
    OnlineMetadataResult,
};

const AUDNEXUS_DEFAULT_REGION: &str = "us";
const AUDNEXUS_USER_AGENT: &str = "audiobook-boss/metadata-lookup";
const AUDIBLE_MAX_CONCURRENCY: usize = 6;

pub(super) async fn search_online_metadata(
    query: String,
    sources: Option<Vec<MetadataSource>>,
    limit: Option<u8>,
) -> Result<MetadataLookupResponse> {
    let trimmed = query.trim();
    if trimmed.len() < 3 {
        return Err(AppError::InvalidInput(
            "Search query must be at least 3 characters.".to_string(),
        ));
    }

    let limit = limit.unwrap_or(8).clamp(1, 20);
    let sources_to_query =
        sources.unwrap_or_else(|| vec![MetadataSource::Audnexus, MetadataSource::Openlibrary]);
    let include_audnexus = sources_to_query.contains(&MetadataSource::Audnexus);
    let include_openlibrary = sources_to_query.contains(&MetadataSource::Openlibrary);

    let region =
        extract_region_override(trimmed).unwrap_or_else(|| AUDNEXUS_DEFAULT_REGION.to_string());

    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(AUDNEXUS_USER_AGENT)
        .build()
        .map_err(|e| {
            log::error!("Failed to configure metadata lookup client: {}", e);
            AppError::General("Failed to configure metadata lookup client".to_string())
        })?;

    let mut diagnostics = Vec::new();

    if let Some(asin) = extract_valid_audnexus_asin(trimmed, include_audnexus) {
        match fetch_audnexus_book(&client, &asin, &region).await {
            Ok(result) => {
                return Ok(MetadataLookupResponse {
                    results: vec![result],
                    diagnostics,
                });
            }
            Err(e) => {
                log::warn!(
                    "ASIN lookup failed for {}: {}. Continuing with text search.",
                    asin,
                    e
                );
                // Provider ASIN detail unavailable; continue through normal search and emit diagnostic.
                diagnostics.push(asin_text_search_diagnostic());
            }
        }
    }

    let search_query = strip_region_overrides(trimmed);
    if search_query.is_empty() {
        return Err(AppError::InvalidInput(
            "Search query must include a title, author, or ASIN.".to_string(),
        ));
    }

    let provider_output = collect_provider_searches(
        &client,
        &search_query,
        &region,
        include_audnexus,
        include_openlibrary,
        limit,
    )
    .await;
    diagnostics.extend(provider_output.diagnostics);

    if all_selected_sources_failed(
        provider_output.selected_source_count,
        provider_output.failed_source_count,
    ) {
        return Err(AppError::General(
            "All selected metadata sources failed. Please try again.".to_string(),
        ));
    }

    // Merge results: Audnexus first (higher priority), then OpenLibrary fills gaps
    let merged = merge_search_results(
        provider_output.audnexus_results,
        provider_output.openlibrary_results,
        limit,
    );

    Ok(MetadataLookupResponse {
        results: merged,
        diagnostics,
    })
}

#[derive(Debug, Default)]
struct ProviderSearchOutput {
    audnexus_results: Vec<OnlineMetadataResult>,
    openlibrary_results: Vec<OnlineMetadataResult>,
    selected_source_count: usize,
    failed_source_count: usize,
    diagnostics: Vec<MetadataLookupDiagnostic>,
}

async fn collect_provider_searches(
    client: &Client,
    search_query: &str,
    region: &str,
    include_audnexus: bool,
    include_openlibrary: bool,
    limit: u8,
) -> ProviderSearchOutput {
    let audnexus_handle = if include_audnexus {
        let client = client.clone();
        let search_query = search_query.to_string();
        let region = region.to_string();
        Some(tokio::spawn(async move {
            fetch_audnexus_with_audible(&client, &search_query, &region, limit).await
        }))
    } else {
        None
    };

    let openlibrary_handle = if include_openlibrary {
        let client = client.clone();
        let search_query = search_query.to_string();
        Some(tokio::spawn(async move {
            fetch_openlibrary_search(&client, &search_query, limit).await
        }))
    } else {
        None
    };

    let mut output = ProviderSearchOutput::default();

    if let Some(handle) = audnexus_handle {
        output.selected_source_count += 1;
        match handle.await {
            Ok(Ok(audnexus_output)) => {
                output.audnexus_results = audnexus_output.results;
                output.diagnostics.extend(audnexus_output.diagnostics);
            }
            Ok(Err(error)) => {
                output.failed_source_count += 1;
                log::warn!("Audnexus search failed: {}", error);
                output
                    .diagnostics
                    .push(source_failed_diagnostic(MetadataSource::Audnexus));
            }
            Err(error) => {
                output.failed_source_count += 1;
                log::error!("Audnexus task panicked: {}", error);
                output
                    .diagnostics
                    .push(source_failed_diagnostic(MetadataSource::Audnexus));
            }
        }
    }

    if let Some(handle) = openlibrary_handle {
        output.selected_source_count += 1;
        match handle.await {
            Ok(Ok(results)) => output.openlibrary_results = results,
            Ok(Err(error)) => {
                output.failed_source_count += 1;
                log::warn!("OpenLibrary search failed: {}", error);
                output
                    .diagnostics
                    .push(source_failed_diagnostic(MetadataSource::Openlibrary));
            }
            Err(error) => {
                output.failed_source_count += 1;
                log::error!("OpenLibrary task panicked: {}", error);
                output
                    .diagnostics
                    .push(source_failed_diagnostic(MetadataSource::Openlibrary));
            }
        }
    }

    output
}

#[derive(Debug, Default)]
struct AudnexusSearchOutput {
    results: Vec<OnlineMetadataResult>,
    diagnostics: Vec<MetadataLookupDiagnostic>,
}

async fn fetch_audnexus_with_audible(
    client: &Client,
    search_query: &str,
    region: &str,
    limit: u8,
) -> Result<AudnexusSearchOutput> {
    let audible_items = fetch_audible_search(client, search_query, region, limit).await?;
    if audible_items.is_empty() {
        return Ok(AudnexusSearchOutput::default());
    }

    let mut combined = Vec::new();
    let mut diagnostics = Vec::new();
    for chunk in audible_items.chunks(AUDIBLE_MAX_CONCURRENCY) {
        let mut handles = Vec::new();
        for item in chunk {
            let client = client.clone();
            let region = region.to_string();
            let item = item.clone();
            let handle = tokio::spawn(async move {
                match fetch_audnexus_book(&client, &item.asin, &region).await {
                    Ok(result) => (result, false),
                    Err(err) => {
                        log::warn!("Audnexus lookup failed for {}: {}", item.asin, err);
                        // Audnexus detail enrichment unavailable; return Audible-derived result with provenance marker.
                        (map_audible_item(item), true)
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            match handle.await {
                Ok((result, used_audible_only_provenance)) => {
                    if used_audible_only_provenance {
                        push_audible_only_diagnostic_once(&mut diagnostics);
                    }
                    combined.push(result);
                }
                Err(err) => {
                    log::error!("Audnexus lookup task failed: {}", err);
                }
            }
        }
    }

    Ok(AudnexusSearchOutput {
        results: combined,
        diagnostics,
    })
}

fn asin_text_search_diagnostic() -> MetadataLookupDiagnostic {
    MetadataLookupDiagnostic {
        kind: MetadataLookupDiagnosticKind::AsinDirectLookupUnavailableTextSearchUsed,
        source: Some(MetadataSource::Audnexus),
        message: "Audnexus ASIN lookup failed, so ABB searched by title/author text instead."
            .to_string(),
    }
}

fn source_failed_diagnostic(source: MetadataSource) -> MetadataLookupDiagnostic {
    // Provider failed; emit diagnostic and continue with available results from other sources.
    MetadataLookupDiagnostic {
        kind: MetadataLookupDiagnosticKind::SourceFailedPartialResults,
        source: Some(source),
        message: "One selected metadata source failed; ABB is showing available results."
            .to_string(),
    }
}

fn audible_only_diagnostic() -> MetadataLookupDiagnostic {
    MetadataLookupDiagnostic {
        kind: MetadataLookupDiagnosticKind::AudnexusDetailUnavailableAudibleOnlyResult,
        source: Some(MetadataSource::Audnexus),
        message:
            "Some Audible results could not be enriched by Audnexus and are marked Audible-only."
                .to_string(),
    }
}

fn push_audible_only_diagnostic_once(diagnostics: &mut Vec<MetadataLookupDiagnostic>) {
    if diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == MetadataLookupDiagnosticKind::AudnexusDetailUnavailableAudibleOnlyResult
    }) {
        return;
    }

    diagnostics.push(audible_only_diagnostic());
}

fn merge_search_results(
    audnexus_results: Vec<OnlineMetadataResult>,
    openlibrary_results: Vec<OnlineMetadataResult>,
    limit: u8,
) -> Vec<OnlineMetadataResult> {
    let mut merged: Vec<OnlineMetadataResult> = Vec::new();
    let mut seen_source_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut audnexus_content_keys: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    // Add Audnexus results first (higher priority)
    for result in audnexus_results {
        let source_key = source_dedupe_key(&result);
        if seen_source_keys.insert(source_key) {
            if let Some(content_key) = content_dedupe_key(&result) {
                audnexus_content_keys.insert(content_key);
            }
            merged.push(result);
        }
    }

    // Add OpenLibrary results (fill gaps)
    for result in openlibrary_results {
        let source_key = source_dedupe_key(&result);
        if !seen_source_keys.insert(source_key) {
            continue;
        }

        if let Some(content_key) = content_dedupe_key(&result) {
            if audnexus_content_keys.contains(&content_key) {
                continue;
            }
        }

        merged.push(result);
    }

    // Respect the limit
    merged.truncate(limit as usize);
    merged
}

fn source_dedupe_key(result: &OnlineMetadataResult) -> String {
    format!(
        "{}:{}",
        result.source_id,
        result.title.trim().to_lowercase()
    )
}

fn content_dedupe_key(result: &OnlineMetadataResult) -> Option<String> {
    let title = result.title.trim().to_lowercase();
    if title.is_empty() {
        return None;
    }

    let mut authors: Vec<String> = result
        .authors
        .iter()
        .map(|author| author.trim().to_lowercase())
        .filter(|author| !author.is_empty())
        .collect();
    if authors.is_empty() {
        return None;
    }

    authors.sort_unstable();
    Some(format!("{}:{}", title, authors.join("|")))
}

fn extract_valid_audnexus_asin(query: &str, include_audnexus: bool) -> Option<String> {
    if !include_audnexus {
        return None;
    }

    let asin = extract_asin(query)?;
    if asin.starts_with('B') && asin.chars().skip(1).any(|c| c.is_ascii_digit()) {
        Some(asin)
    } else {
        log::debug!(
            "Extracted potential ASIN '{}' but doesn't look like valid ASIN format",
            asin
        );
        None
    }
}

fn all_selected_sources_failed(selected_source_count: usize, failed_source_count: usize) -> bool {
    selected_source_count > 0 && failed_source_count == selected_source_count
}

#[cfg(test)]
mod tests {
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
}
