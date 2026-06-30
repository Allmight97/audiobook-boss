use crate::errors::{AppError, Result};
use reqwest::Client;
use serde::Deserialize;

use super::super::types::{MetadataSource, OnlineMetadataResult};

const OPENLIBRARY_BASE_URL: &str = "https://openlibrary.org";
const OPENLIBRARY_COVER_URL: &str = "https://covers.openlibrary.org/b/id";

#[derive(Debug, Deserialize)]
struct OpenLibrarySearchResponse {
    #[serde(default)]
    docs: Vec<OpenLibraryDoc>,
}

#[derive(Debug, Deserialize)]
struct OpenLibraryDoc {
    key: String,
    title: String,
    #[serde(default)]
    author_name: Vec<String>,
    cover_i: Option<i64>,
    first_publish_year: Option<i32>,
    #[serde(default)]
    description: Option<String>,
}

fn build_cover_url(cover_id: i64, size: char) -> String {
    format!("{}/{}/{}.jpg", OPENLIBRARY_COVER_URL, cover_id, size)
}

fn map_openlibrary_doc(doc: OpenLibraryDoc) -> OnlineMetadataResult {
    let cover_url = doc.cover_i.map(|id| build_cover_url(id, 'L'));

    OnlineMetadataResult {
        source: MetadataSource::Openlibrary,
        source_id: doc.key.clone(),
        title: doc.title,
        authors: doc.author_name,
        narrators: Vec::new(), // OpenLibrary doesn't have narrator data
        series: None,
        series_part: None,
        subseries: None,
        subseries_part: None,
        description: doc.description,
        published_date: doc.first_publish_year.map(|year| year.to_string()),
        duration_seconds: None, // OpenLibrary doesn't have duration data
        cover_url,
        audible_only: Some(false),
    }
}

pub(in crate::commands::metadata_lookup) async fn fetch_openlibrary_search(
    client: &Client,
    query: &str,
    limit: u8,
) -> Result<Vec<OnlineMetadataResult>> {
    let fields = "key,title,author_name,cover_i,first_publish_year,description";
    let limit = limit.to_string();
    let response = client
        .get(format!("{OPENLIBRARY_BASE_URL}/search.json"))
        .query(&[("q", query), ("fields", fields), ("limit", limit.as_str())])
        .send()
        .await
        .map_err(|e| {
            log::warn!("OpenLibrary request failed: {}", e);
            AppError::General("OpenLibrary request failed".to_string())
        })?;

    if !response.status().is_success() {
        log::warn!("OpenLibrary returned status: {}", response.status());
        return Err(AppError::General("OpenLibrary request failed".to_string()));
    }

    let search_result: OpenLibrarySearchResponse = response.json().await.map_err(|e| {
        log::warn!("OpenLibrary response parse failed: {}", e);
        AppError::General("OpenLibrary response parse failed".to_string())
    })?;

    let results: Vec<OnlineMetadataResult> = search_result
        .docs
        .into_iter()
        .map(map_openlibrary_doc)
        .collect();

    Ok(results)
}
