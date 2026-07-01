use std::path::PathBuf;

pub use abb_remote_source_core::AcquisitionProgress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProviderId {
    Audible,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountRef {
    pub provider_id: ProviderId,
    pub account_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAuthFlow {
    ExternalBrowserHandoff,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSourceProviderCapabilities {
    pub provider_id: ProviderId,
    pub label: String,
    pub auth_flow: RemoteAuthFlow,
    pub supports_library_scan: bool,
    pub supports_paged_scan: bool,
    pub supports_typeahead_filter: bool,
    pub supports_supplemental_pdf: bool,
    pub supports_materialized_audio: bool,
    pub supports_refresh: bool,
    pub requires_live_session: bool,
    pub known_unsupported_reasons: Vec<RemoteAcquisitionFailureKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAccountStatus {
    Connected,
    NeedsAuth,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSourceAccountState {
    pub provider_id: ProviderId,
    pub status: RemoteAccountStatus,
    pub account: Option<AccountRef>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAuthStartResponse {
    pub provider_id: ProviderId,
    pub authorization_url: String,
    pub handoff_path_hint: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAuthCompletionRequest {
    pub provider_id: ProviderId,
    pub response_url_handoff_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTitle {
    pub provider_id: ProviderId,
    pub title_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub duration_seconds: Option<u32>,
    pub cover_url: Option<String>,
    pub supplemental_pdf_available: bool,
    pub acquired: bool,
    pub availability: RemoteTitleAvailability,
    pub unsupported_reasons: Vec<RemoteAcquisitionFailureKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoteTitleAvailabilityStatus {
    Available,
    CatalogOnly,
    Revoked,
    ProviderUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTitleAvailability {
    pub status: RemoteTitleAvailabilityStatus,
    pub acquirable: bool,
    pub label: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLibraryResponse {
    pub provider_id: ProviderId,
    pub titles: Vec<RemoteTitle>,
    pub diagnostics: Vec<RemoteSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionSelection {
    pub title_id: String,
    pub include_supplemental_pdf: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionPlan {
    pub provider_id: ProviderId,
    pub selections: Vec<AcquisitionSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAcquisitionStatus {
    Planned,
    Acquiring,
    Materialized,
    Validated,
    ImportedToFileList,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAcquisitionFailureKind {
    AuthRequired,
    ProviderPrivateProtocolFailed,
    ProtectedUnsupported,
    DownloadFailed,
    MaterializationFailed,
    /// Decryption succeeded and the title is import-ready, but the encrypted
    /// staging source could not be purged. Non-blocking: the startup session
    /// sweep removes it on next launch.
    ProtectedSourcePurgeFailed,
    ValidationFailed,
    SupplementalPdfFailed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSourceDiagnostic {
    pub kind: RemoteAcquisitionFailureKind,
    pub title_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MaterializedSourceFile {
    pub input_id: String,
    pub title_id: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SupplementalAsset {
    pub asset_id: String,
    pub input_id: String,
    pub title_id: String,
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionJob {
    pub job_id: String,
    pub provider_id: ProviderId,
    pub status: RemoteAcquisitionStatus,
    pub progress: AcquisitionProgress,
    pub materialized_files: Vec<MaterializedSourceFile>,
    pub supplemental_assets: Vec<SupplementalAsset>,
    pub diagnostics: Vec<RemoteSourceDiagnostic>,
}
