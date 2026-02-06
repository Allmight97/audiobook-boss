use std::time::Duration;

use crate::errors::{AppError, Result};
use reqwest::Client;

use super::mapping::map_audible_item;
use super::parse::{extract_asin, extract_region_override, strip_region_overrides};
use super::providers::audible::fetch_audible_search;
use super::providers::audnexus::fetch_audnexus_book;
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
    let _sources = sources.unwrap_or_else(|| vec![MetadataSource::Audnexus]);

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

    if let Some(asin) = extract_asin(trimmed) {
        let result = fetch_audnexus_book(&client, &asin, &region).await?;
        return Ok(vec![result]);
    }

    let search_query = strip_region_overrides(trimmed);
    if search_query.is_empty() {
        return Err(AppError::InvalidInput(
            "Search query must include a title, author, or ASIN.".to_string(),
        ));
    }
    let audible_items = fetch_audible_search(&client, &search_query, &region, limit).await?;
    if audible_items.is_empty() {
        return Ok(Vec::new());
    }

    let mut combined = Vec::new();
    for chunk in audible_items.chunks(AUDIBLE_MAX_CONCURRENCY) {
        let mut handles = Vec::new();
        for item in chunk {
            let client = client.clone();
            let region = region.clone();
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
