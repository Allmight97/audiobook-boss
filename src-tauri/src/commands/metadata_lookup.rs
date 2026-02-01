use crate::errors::{AppError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataSource {
    OpenLibrary,
    Itunes,
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
    pub description: Option<String>,
    pub published_year: Option<i32>,
    pub duration_seconds: Option<u32>,
    pub cover_url: Option<String>,
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
    let sources =
        sources.unwrap_or_else(|| vec![MetadataSource::OpenLibrary, MetadataSource::Itunes]);

    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("audiobook-boss/metadata-lookup")
        .build()
        .map_err(|e| {
            log::error!("Failed to configure metadata lookup client: {}", e);
            AppError::General("Failed to configure metadata lookup client".to_string())
        })?;

    let mut combined = Vec::new();
    for source in sources {
        let mut results = match source {
            MetadataSource::OpenLibrary => fetch_open_library(&client, trimmed, limit).await?,
            MetadataSource::Itunes => fetch_itunes(&client, trimmed, limit).await?,
        };
        combined.append(&mut results);
    }

    Ok(combined)
}

#[derive(Debug, Deserialize)]
struct OpenLibraryResponse {
    docs: Vec<OpenLibraryDoc>,
}

#[derive(Debug, Deserialize)]
struct OpenLibraryDoc {
    key: Option<String>,
    title: Option<String>,
    subtitle: Option<String>,
    author_name: Option<Vec<String>>,
    first_publish_year: Option<i32>,
    cover_i: Option<i64>,
}

async fn fetch_open_library(
    client: &Client,
    query: &str,
    limit: u8,
) -> Result<Vec<OnlineMetadataResult>> {
    let mut url = reqwest::Url::parse("https://openlibrary.org/search.json")
        .map_err(|_| AppError::General("Failed to build Open Library URL".to_string()))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("limit", &limit.to_string());

    let response = client.get(url).send().await.map_err(|e| {
        log::warn!("Open Library request failed: {}", e);
        AppError::General("Open Library request failed".to_string())
    })?;

    if !response.status().is_success() {
        return Err(AppError::General("Open Library request failed".to_string()));
    }

    let payload: OpenLibraryResponse = response.json().await.map_err(|e| {
        log::warn!("Open Library response parse failed: {}", e);
        AppError::General("Open Library response parse failed".to_string())
    })?;

    let results = payload
        .docs
        .into_iter()
        .enumerate()
        .filter_map(|(idx, doc)| {
            let title = doc.title?.trim().to_string();
            if title.is_empty() {
                return None;
            }
            let full_title = doc
                .subtitle
                .as_ref()
                .map(|subtitle| format!("{}: {}", title, subtitle.trim()))
                .unwrap_or(title);

            let source_id = doc.key.unwrap_or_else(|| format!("openlibrary-{}", idx));
            let authors = doc.author_name.unwrap_or_default();
            let cover_url = doc
                .cover_i
                .map(|id| format!("https://covers.openlibrary.org/b/id/{}-L.jpg", id));

            Some(OnlineMetadataResult {
                source: MetadataSource::OpenLibrary,
                source_id,
                title: full_title,
                authors,
                narrators: Vec::new(),
                series: None,
                series_part: None,
                description: None,
                published_year: doc.first_publish_year,
                duration_seconds: None,
                cover_url,
            })
        })
        .collect();

    Ok(results)
}

#[derive(Debug, Deserialize)]
struct ItunesResponse {
    results: Vec<ItunesItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesItem {
    track_id: Option<u64>,
    track_name: Option<String>,
    collection_name: Option<String>,
    artist_name: Option<String>,
    description: Option<String>,
    long_description: Option<String>,
    release_date: Option<String>,
    artwork_url_100: Option<String>,
    artwork_url_600: Option<String>,
    track_time_millis: Option<u64>,
}

async fn fetch_itunes(
    client: &Client,
    query: &str,
    limit: u8,
) -> Result<Vec<OnlineMetadataResult>> {
    let mut url = reqwest::Url::parse("https://itunes.apple.com/search")
        .map_err(|_| AppError::General("Failed to build Apple Books URL".to_string()))?;
    url.query_pairs_mut()
        .append_pair("media", "audiobook")
        .append_pair("term", query)
        .append_pair("limit", &limit.to_string());

    let response = client.get(url).send().await.map_err(|e| {
        log::warn!("Apple Books request failed: {}", e);
        AppError::General("Apple Books request failed".to_string())
    })?;

    if !response.status().is_success() {
        return Err(AppError::General("Apple Books request failed".to_string()));
    }

    let payload: ItunesResponse = response.json().await.map_err(|e| {
        log::warn!("Apple Books response parse failed: {}", e);
        AppError::General("Apple Books response parse failed".to_string())
    })?;

    let results = payload
        .results
        .into_iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let title = item.track_name.or(item.collection_name).unwrap_or_default();
            let title = title.trim().to_string();
            if title.is_empty() {
                return None;
            }

            let source_id = item
                .track_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| format!("itunes-{}", idx));
            let authors = item.artist_name.map(|name| vec![name]).unwrap_or_default();
            let description = item.long_description.or(item.description);
            let published_year = parse_year(item.release_date.as_deref());
            let duration_seconds = item.track_time_millis.map(|millis| (millis / 1000) as u32);

            let cover_url = normalize_cover_url(item.artwork_url_600)
                .or_else(|| normalize_cover_url(item.artwork_url_100));

            Some(OnlineMetadataResult {
                source: MetadataSource::Itunes,
                source_id,
                title,
                authors,
                narrators: Vec::new(),
                series: None,
                series_part: None,
                description,
                published_year,
                duration_seconds,
                cover_url,
            })
        })
        .collect();

    Ok(results)
}

fn parse_year(value: Option<&str>) -> Option<i32> {
    let raw = value?.trim();
    if raw.len() < 4 {
        return None;
    }

    raw.get(0..4)?.parse().ok()
}

fn normalize_cover_url(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let parsed = reqwest::Url::parse(&raw).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    Some(parsed.to_string())
}
