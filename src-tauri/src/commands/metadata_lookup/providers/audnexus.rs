use crate::errors::{AppError, Result};
use reqwest::Client;
use serde::Deserialize;

use super::super::mapping::map_audnexus_book;
use super::super::types::OnlineMetadataResult;

const AUDNEXUS_BASE_URL: &str = "https://api.audnex.us";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::commands::metadata_lookup) struct AudnexusBook {
    pub(in crate::commands::metadata_lookup) asin: String,
    pub(in crate::commands::metadata_lookup) title: String,
    pub(in crate::commands::metadata_lookup) subtitle: Option<String>,
    pub(in crate::commands::metadata_lookup) authors: Vec<AudnexusPerson>,
    pub(in crate::commands::metadata_lookup) narrators: Vec<AudnexusPerson>,
    pub(in crate::commands::metadata_lookup) series_primary: Option<AudnexusSeries>,
    pub(in crate::commands::metadata_lookup) series_secondary: Option<AudnexusSeries>,
    pub(in crate::commands::metadata_lookup) description: Option<String>,
    pub(in crate::commands::metadata_lookup) summary: Option<String>,
    pub(in crate::commands::metadata_lookup) release_date: Option<String>,
    pub(in crate::commands::metadata_lookup) runtime_length_min: Option<f64>,
    pub(in crate::commands::metadata_lookup) image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(in crate::commands::metadata_lookup) struct AudnexusPerson {
    pub(in crate::commands::metadata_lookup) name: String,
}

#[derive(Debug, Deserialize)]
pub(in crate::commands::metadata_lookup) struct AudnexusSeries {
    pub(in crate::commands::metadata_lookup) name: String,
    pub(in crate::commands::metadata_lookup) position: Option<String>,
}

pub(in crate::commands::metadata_lookup) async fn fetch_audnexus_book(
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
