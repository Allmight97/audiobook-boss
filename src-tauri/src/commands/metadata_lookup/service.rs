use std::time::Duration;

use crate::errors::{AppError, Result};
use reqwest::Client;

use super::mapping::map_audible_item;
use super::parse::{extract_asin, extract_region_override, strip_region_overrides};
use super::providers::audible::fetch_audible_search;
use super::providers::audnexus::fetch_audnexus_book;
use super::providers::openlibrary::fetch_openlibrary_search;
use super::types::{MetadataSource, OnlineMetadataResult};

const AUDNEXUS_DEFAULT_REGION: &str = "us";
const AUDNEXUS_USER_AGENT: &str = "audiobook-boss/metadata-lookup";
const AUDIBLE_MAX_CONCURRENCY: usize = 6;

pub(super) async fn search_online_metadata(
    query: String,
    sources: Option<Vec<MetadataSource>>,
    limit: Option<u8>,
) -> Result<Vec<OnlineMetadataResult>> {
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

    if let Some(asin) = extract_valid_audnexus_asin(trimmed, include_audnexus) {
        match fetch_audnexus_book(&client, &asin, &region).await {
            Ok(result) => return Ok(vec![result]),
            Err(e) => {
                log::warn!(
                    "ASIN lookup failed for {}: {}. Continuing with text search.",
                    asin,
                    e
                );
                // Continue to text search instead of returning error
            }
        }
    }

    let search_query = strip_region_overrides(trimmed);
    if search_query.is_empty() {
        return Err(AppError::InvalidInput(
            "Search query must include a title, author, or ASIN.".to_string(),
        ));
    }

    // Concurrent searches
    let audnexus_handle = if include_audnexus {
        let client = client.clone();
        let search_query = search_query.clone();
        let region = region.clone();
        Some(tokio::spawn(async move {
            fetch_audnexus_with_audible(&client, &search_query, &region, limit).await
        }))
    } else {
        None
    };

    let openlibrary_handle = if include_openlibrary {
        let client = client.clone();
        let search_query = search_query.clone();
        Some(tokio::spawn(async move {
            fetch_openlibrary_search(&client, &search_query, limit).await
        }))
    } else {
        None
    };

    // Collect results
    let mut audnexus_results: Vec<OnlineMetadataResult> = Vec::new();
    let mut openlibrary_results: Vec<OnlineMetadataResult> = Vec::new();
    let mut selected_source_count = 0usize;
    let mut failed_source_count = 0usize;

    if let Some(handle) = audnexus_handle {
        selected_source_count += 1;
        match handle.await {
            Ok(Ok(results)) => audnexus_results = results,
            Ok(Err(e)) => {
                failed_source_count += 1;
                log::warn!("Audnexus search failed: {}", e);
            }
            Err(e) => {
                failed_source_count += 1;
                log::error!("Audnexus task panicked: {}", e);
            }
        }
    }

    if let Some(handle) = openlibrary_handle {
        selected_source_count += 1;
        match handle.await {
            Ok(Ok(results)) => openlibrary_results = results,
            Ok(Err(e)) => {
                failed_source_count += 1;
                log::warn!("OpenLibrary search failed: {}", e);
            }
            Err(e) => {
                failed_source_count += 1;
                log::error!("OpenLibrary task panicked: {}", e);
            }
        }
    }

    if all_selected_sources_failed(selected_source_count, failed_source_count) {
        return Err(AppError::General(
            "All selected metadata sources failed. Please try again.".to_string(),
        ));
    }

    // Merge results: Audnexus first (higher priority), then OpenLibrary fills gaps
    let merged = merge_search_results(audnexus_results, openlibrary_results, limit);

    Ok(merged)
}

async fn fetch_audnexus_with_audible(
    client: &Client,
    search_query: &str,
    region: &str,
    limit: u8,
) -> Result<Vec<OnlineMetadataResult>> {
    let audible_items = fetch_audible_search(client, search_query, region, limit).await?;
    if audible_items.is_empty() {
        return Ok(Vec::new());
    }

    let mut combined = Vec::new();
    for chunk in audible_items.chunks(AUDIBLE_MAX_CONCURRENCY) {
        let mut handles = Vec::new();
        for item in chunk {
            let client = client.clone();
            let region = region.to_string();
            let item = item.clone();
            let handle = tokio::spawn(async move {
                match fetch_audnexus_book(&client, &item.asin, &region).await {
                    Ok(result) => result,
                    Err(err) => {
                        log::warn!("Audnexus lookup failed for {}: {}", item.asin, err);
                        map_audible_item(item)
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            match handle.await {
                Ok(result) => combined.push(result),
                Err(err) => {
                    log::error!("Audnexus lookup task failed: {}", err);
                }
            }
        }
    }

    Ok(combined)
}

fn merge_search_results(
    audnexus_results: Vec<OnlineMetadataResult>,
    openlibrary_results: Vec<OnlineMetadataResult>,
    limit: u8,
) -> Vec<OnlineMetadataResult> {
    let mut merged: Vec<OnlineMetadataResult> = Vec::new();
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Add Audnexus results first (higher priority)
    for result in audnexus_results {
        let key = format!("{}:{}", result.source_id, result.title.to_lowercase());
        if seen_keys.insert(key) {
            merged.push(result);
        }
    }

    // Add OpenLibrary results (fill gaps)
    for result in openlibrary_results {
        let key = format!("{}:{}", result.source_id, result.title.to_lowercase());
        if seen_keys.insert(key) {
            merged.push(result);
        }
    }

    // Respect the limit
    merged.truncate(limit as usize);
    merged
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

    fn make_result(source: MetadataSource, source_id: &str, title: &str) -> OnlineMetadataResult {
        OnlineMetadataResult {
            source,
            source_id: source_id.to_string(),
            title: title.to_string(),
            authors: Vec::new(),
            narrators: Vec::new(),
            series: None,
            series_part: None,
            subseries: None,
            subseries_part: None,
            description: None,
            published_year: None,
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
    fn merge_search_results_keeps_audnexus_priority_and_respects_limit() {
        let audnexus_results = vec![
            make_result(MetadataSource::Audnexus, "aud-1", "Alpha"),
            make_result(MetadataSource::Audnexus, "aud-2", "Beta"),
        ];
        let openlibrary_results = vec![
            make_result(MetadataSource::Openlibrary, "aud-1", "Alpha"),
            make_result(MetadataSource::Openlibrary, "ol-3", "Gamma"),
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
}
