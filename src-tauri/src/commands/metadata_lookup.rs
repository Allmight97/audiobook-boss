use crate::errors::{AppError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const AUDNEXUS_BASE_URL: &str = "https://api.audnex.us";
const AUDNEXUS_DEFAULT_REGION: &str = "us";
const AUDNEXUS_USER_AGENT: &str = "audiobook-boss/metadata-lookup";
const AUDNEXUS_ALLOWED_REGIONS: &[&str] =
    &["au", "ca", "de", "es", "fr", "in", "it", "jp", "us", "uk"];

const AUDIBLE_BASE_DOMAIN: &str = "https://api.audible";
const AUDIBLE_SEARCH_PATH: &str = "1.0/catalog/products";
const AUDIBLE_RESPONSE_GROUPS: &str =
    "contributors,product_desc,product_attrs,product_extended_attrs,media,product_details,series";
const AUDIBLE_IMAGE_SIZES: &str = "500,1024";
const AUDIBLE_SEARCH_SORT: &str = "Relevance";
const AUDIBLE_MAX_CONCURRENCY: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataSource {
    Audnexus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMetadataResult {
    pub source: MetadataSource,
    pub source_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_part: Option<String>,
    pub subseries: Option<String>,
    pub subseries_part: Option<String>,
    pub description: Option<String>,
    pub published_year: Option<i32>,
    pub duration_seconds: Option<u32>,
    pub cover_url: Option<String>,
    pub audible_only: Option<bool>,
}

#[tauri::command]
pub async fn search_online_metadata(
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudnexusBook {
    asin: String,
    title: String,
    subtitle: Option<String>,
    authors: Vec<AudnexusPerson>,
    narrators: Vec<AudnexusPerson>,
    series_primary: Option<AudnexusSeries>,
    series_secondary: Option<AudnexusSeries>,
    description: Option<String>,
    summary: Option<String>,
    release_date: Option<String>,
    runtime_length_min: Option<f64>,
    image: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AudnexusPerson {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AudnexusSeries {
    name: String,
    position: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct AudibleSearchResponse {
    products: Vec<AudibleSearchItem>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct AudibleSearchItem {
    asin: String,
    title: Option<String>,
    subtitle: Option<String>,
    authors: Option<Vec<AudiblePerson>>,
    narrators: Option<Vec<AudiblePerson>>,
    release_date: Option<String>,
    runtime_length_min: Option<f64>,
    publisher_summary: Option<String>,
    merchandising_summary: Option<String>,
    product_images: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Clone)]
struct AudiblePerson {
    name: String,
}

async fn fetch_audnexus_book(
    client: &Client,
    asin: &str,
    region: &str,
) -> Result<OnlineMetadataResult> {
    let mut url = reqwest::Url::parse(&format!("{}/books/{}", AUDNEXUS_BASE_URL, asin))
        .map_err(|_| AppError::General("Failed to build Audnexus URL".to_string()))?;
    url.query_pairs_mut().append_pair("region", region);

    let response = client.get(url).send().await.map_err(|e| {
        log::warn!("Audnexus request failed: {}", e);
        AppError::General("Audnexus request failed".to_string())
    })?;

    if !response.status().is_success() {
        return Err(AppError::General("Audnexus request failed".to_string()));
    }

    let payload: AudnexusBook = response.json().await.map_err(|e| {
        log::warn!("Audnexus response parse failed: {}", e);
        AppError::General("Audnexus response parse failed".to_string())
    })?;

    Ok(map_audnexus_book(payload))
}

async fn fetch_audible_search(
    client: &Client,
    query: &str,
    region: &str,
    limit: u8,
) -> Result<Vec<AudibleSearchItem>> {
    let tld = region_to_tld(region);
    let base = format!("{}.{}", AUDIBLE_BASE_DOMAIN, tld);
    let mut url = reqwest::Url::parse(&format!("{}/{}", base, AUDIBLE_SEARCH_PATH))
        .map_err(|_| AppError::General("Failed to build Audible URL".to_string()))?;
    url.query_pairs_mut()
        .append_pair("response_groups", AUDIBLE_RESPONSE_GROUPS)
        .append_pair("products_sort_by", AUDIBLE_SEARCH_SORT)
        .append_pair("num_results", &limit.to_string())
        .append_pair("image_sizes", AUDIBLE_IMAGE_SIZES)
        .append_pair("keywords", query);

    let response = client.get(url).send().await.map_err(|e| {
        log::warn!("Audible search request failed: {}", e);
        AppError::General("Audible search request failed".to_string())
    })?;

    if !response.status().is_success() {
        return Err(AppError::General(
            "Audible search request failed".to_string(),
        ));
    }

    let payload: AudibleSearchResponse = response.json().await.map_err(|e| {
        log::warn!("Audible response parse failed: {}", e);
        AppError::General("Audible response parse failed".to_string())
    })?;

    Ok(payload.products)
}

fn map_audnexus_book(book: AudnexusBook) -> OnlineMetadataResult {
    let title = book.title.trim().to_string();
    let title = match book
        .subtitle
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(subtitle) => format!("{}: {}", title, subtitle),
        None => title,
    };

    let authors = book.authors.into_iter().map(|p| p.name).collect();
    let narrators = book.narrators.into_iter().map(|p| p.name).collect();

    let (mut series, series_part) = match book.series_primary {
        Some(series) => (Some(series.name), clean_series_part(series.position)),
        None => (None, None),
    };
    let (mut subseries, mut subseries_part) = match book.series_secondary {
        Some(series) => (Some(series.name), clean_series_part(series.position)),
        None => (None, None),
    };

    if subseries.is_none() {
        if let (Some(series_name), Some(series_part_value)) =
            (series.as_deref(), series_part.as_deref())
        {
            if let Some((primary, secondary)) = split_primary_series_name(series_name) {
                series = Some(primary);
                subseries = Some(secondary);
                subseries_part = Some(series_part_value.to_string());
            }
        }
    }

    let description = book.summary.or(book.description);
    let published_year = parse_year(book.release_date.as_deref());
    let duration_seconds = book
        .runtime_length_min
        .map(|minutes| (minutes * 60.0).round() as u32);
    let cover_url = normalize_https_url(book.image);

    OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: book.asin,
        title,
        authors,
        narrators,
        series,
        series_part,
        subseries,
        subseries_part,
        description,
        published_year,
        duration_seconds,
        cover_url,
        audible_only: None,
    }
}

fn map_audible_item(item: AudibleSearchItem) -> OnlineMetadataResult {
    let title = item
        .title
        .as_deref()
        .unwrap_or(item.asin.as_str())
        .trim()
        .to_string();
    let title = match item
        .subtitle
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(subtitle) => format!("{}: {}", title, subtitle),
        None => title,
    };

    let authors = item
        .authors
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.name)
        .collect();
    let narrators = item
        .narrators
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.name)
        .collect();
    let description = item.publisher_summary.or(item.merchandising_summary);
    let published_year = parse_year(item.release_date.as_deref());
    let duration_seconds = item
        .runtime_length_min
        .map(|minutes| (minutes * 60.0).round() as u32);
    let cover_url = pick_audible_image_url(item.product_images);

    OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: item.asin,
        title,
        authors,
        narrators,
        series: None,
        series_part: None,
        subseries: None,
        subseries_part: None,
        description,
        published_year,
        duration_seconds,
        cover_url,
        audible_only: Some(true),
    }
}

fn clean_series_part(value: Option<String>) -> Option<String> {
    let raw = value?.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut started = false;
    let mut seen_dot = false;

    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            out.push(ch);
            started = true;
            continue;
        }
        if started && ch == '.' && !seen_dot {
            out.push(ch);
            seen_dot = true;
            continue;
        }
        if started {
            break;
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn split_primary_series_name(name: &str) -> Option<(String, String)> {
    let (series, rest) = name.split_once(',')?;
    let series = series.trim();
    let rest = rest.trim();
    if series.is_empty() || rest.is_empty() {
        return None;
    }
    let lowered = rest.to_ascii_lowercase();
    let is_prefixed = ["part", "book", "vol", "vol.", "volume"]
        .iter()
        .any(|prefix| lowered.starts_with(prefix));
    if !is_prefixed {
        return None;
    }
    let normalized = rest.replace(": ", " - ");
    Some((series.to_string(), normalized))
}

fn pick_audible_image_url(images: Option<HashMap<String, String>>) -> Option<String> {
    let images = images?;
    if let Some(url) = images.get("1024") {
        return normalize_https_url(Some(url.clone()));
    }
    if let Some(url) = images.get("500") {
        return normalize_https_url(Some(url.clone()));
    }
    let mut best: Option<(u32, String)> = None;
    for (key, value) in images {
        if let Ok(size) = key.parse::<u32>() {
            let entry = best.as_ref().map(|(s, _)| *s).unwrap_or(0);
            if size > entry {
                best = Some((size, value));
            }
        }
    }
    best.and_then(|(_, url)| normalize_https_url(Some(url)))
}

fn extract_asin(query: &str) -> Option<String> {
    let mut current = String::new();
    for ch in query.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_uppercase());
        } else {
            if current.len() == 10 {
                return Some(current);
            }
            current.clear();
        }
    }

    if current.len() == 10 {
        Some(current)
    } else {
        None
    }
}

fn extract_region_override(query: &str) -> Option<String> {
    let bytes = query.as_bytes();
    for window in bytes.windows(4) {
        if window[0] == b'[' && window[3] == b']' {
            let region = String::from_utf8_lossy(&window[1..3]).to_ascii_lowercase();
            if AUDNEXUS_ALLOWED_REGIONS.contains(&region.as_str()) {
                return Some(region);
            }
        }
    }
    None
}

fn strip_region_overrides(query: &str) -> String {
    let mut output = String::with_capacity(query.len());
    let bytes = query.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if idx + 3 < bytes.len() && bytes[idx] == b'[' && bytes[idx + 3] == b']' {
            let region = String::from_utf8_lossy(&bytes[idx + 1..idx + 3]).to_ascii_lowercase();
            if AUDNEXUS_ALLOWED_REGIONS.contains(&region.as_str()) {
                idx += 4;
                continue;
            }
        }
        output.push(bytes[idx] as char);
        idx += 1;
    }
    output.trim().to_string()
}

fn region_to_tld(region: &str) -> &str {
    match region {
        "au" => "com.au",
        "ca" => "ca",
        "de" => "de",
        "es" => "es",
        "fr" => "fr",
        "in" => "in",
        "it" => "it",
        "jp" => "co.jp",
        "uk" => "co.uk",
        "us" => "com",
        _ => "com",
    }
}

fn parse_year(value: Option<&str>) -> Option<i32> {
    let raw = value?.trim();
    if raw.len() < 4 {
        return None;
    }

    raw.get(0..4)?.parse().ok()
}

fn normalize_https_url(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let parsed = reqwest::Url::parse(&raw).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    Some(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // EXCEPTION: inline tests for private helper functions.
    #[test]
    fn extract_asin_finds_first_asin() {
        let query = "Project Hail Mary B01234ABCD [uk]";
        assert_eq!(extract_asin(query), Some("B01234ABCD".to_string()));
    }

    #[test]
    fn extract_region_override_accepts_known_regions() {
        let query = "B01234ABCD [uk]";
        assert_eq!(extract_region_override(query), Some("uk".to_string()));
        let query = "[us] The Martian";
        assert_eq!(extract_region_override(query), Some("us".to_string()));
    }

    #[test]
    fn split_primary_series_name_parses_part() {
        let input = "Frontiers Saga, Part 3: Fringe Worlds";
        let (series, subseries) =
            split_primary_series_name(input).expect("expected to parse primary series name");
        assert_eq!(series, "Frontiers Saga");
        assert_eq!(subseries, "Part 3 - Fringe Worlds");
    }

    #[test]
    fn clean_series_part_extracts_number() {
        assert_eq!(
            clean_series_part(Some("Book 12".to_string())),
            Some("12".to_string())
        );
        assert_eq!(
            clean_series_part(Some("Part 3.5".to_string())),
            Some("3.5".to_string())
        );
        assert_eq!(clean_series_part(Some("Nope".to_string())), None);
        assert_eq!(clean_series_part(Some("".to_string())), None);
    }

    #[test]
    fn strip_region_overrides_removes_known_regions() {
        let query = "[uk] The Martian";
        assert_eq!(strip_region_overrides(query), "The Martian");
        let query = "The Martian [us]";
        assert_eq!(strip_region_overrides(query), "The Martian");
    }
}
