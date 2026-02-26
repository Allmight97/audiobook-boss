use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MetadataSource {
    Audnexus,
    Openlibrary,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
    pub published_date: Option<String>,
    pub duration_seconds: Option<u32>,
    pub cover_url: Option<String>,
    pub audible_only: Option<bool>,
}
