use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MetadataSource {
    Audnexus,
    Openlibrary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MetadataLookupDiagnosticKind {
    AsinDirectLookupUnavailableTextSearchUsed,
    SourceFailedPartialResults,
    AudnexusDetailUnavailableAudibleOnlyResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataLookupDiagnostic {
    pub kind: MetadataLookupDiagnosticKind,
    pub source: Option<MetadataSource>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataLookupResponse {
    pub results: Vec<OnlineMetadataResult>,
    pub diagnostics: Vec<MetadataLookupDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMetadataResult {
    pub source: MetadataSource,
    #[serde(skip)]
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
    #[serde(skip)]
    pub cover_url: Option<String>,
    /// The address of this result's cover, set by the session that offers it.
    pub cover: Option<String>,
    pub audible_only: Option<bool>,
}
