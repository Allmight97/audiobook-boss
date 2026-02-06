use crate::errors::{AppError, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::super::parse::region_to_tld;

const AUDIBLE_BASE_DOMAIN: &str = "https://api.audible";
const AUDIBLE_SEARCH_PATH: &str = "1.0/catalog/products";
const AUDIBLE_RESPONSE_GROUPS: &str =
    "contributors,product_desc,product_attrs,product_extended_attrs,media,product_details,series";
const AUDIBLE_IMAGE_SIZES: &str = "500,1024";
const AUDIBLE_SEARCH_SORT: &str = "Relevance";

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub(in crate::commands::metadata_lookup) struct AudibleSearchResponse {
    pub(in crate::commands::metadata_lookup) products: Vec<AudibleSearchItem>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub(in crate::commands::metadata_lookup) struct AudibleSearchItem {
    pub(in crate::commands::metadata_lookup) asin: String,
    pub(in crate::commands::metadata_lookup) title: Option<String>,
    pub(in crate::commands::metadata_lookup) subtitle: Option<String>,
    pub(in crate::commands::metadata_lookup) authors: Option<Vec<AudiblePerson>>,
    pub(in crate::commands::metadata_lookup) narrators: Option<Vec<AudiblePerson>>,
    pub(in crate::commands::metadata_lookup) release_date: Option<String>,
    pub(in crate::commands::metadata_lookup) runtime_length_min: Option<f64>,
    pub(in crate::commands::metadata_lookup) publisher_summary: Option<String>,
    pub(in crate::commands::metadata_lookup) merchandising_summary: Option<String>,
    pub(in crate::commands::metadata_lookup) product_images: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub(in crate::commands::metadata_lookup) struct AudiblePerson {
    pub(in crate::commands::metadata_lookup) name: String,
}

pub(in crate::commands::metadata_lookup) async fn fetch_audible_search(
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
