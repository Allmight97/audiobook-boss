use std::path::PathBuf;

pub use abb_remote_source_core::AcquisitionProgress;
use abb_remote_source_core::{acquisition_progress, AcquisitionStage};

const PDF_LOOKUP_MESSAGE: &str = "Looking up the title for its Supplemental PDF.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProviderId {
    Audible,
    Indexer,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSourceProviderCapabilities {
    pub provider_id: ProviderId,
    pub label: String,
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
    pub message: Option<String>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAuthStartResponse {
    pub provider_id: ProviderId,
    pub authorization_url: String,
}

impl std::fmt::Debug for RemoteAuthStartResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RemoteAuthStartResponse (authorization redacted)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTitle {
    pub provider_id: ProviderId,
    pub title_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub cover: Option<String>,
    pub supplemental_pdf_available: bool,
    pub availability: RemoteTitleAvailability,
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
pub enum RemoteReleaseProtocol {
    Usenet,
    Torrent,
    Unknown,
}

pub(crate) fn release_key(indexer_id: i64, guid: &str) -> String {
    serde_json::to_string(&(indexer_id, guid)).expect("release identity is serializable")
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteReleaseCategory {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRelease {
    pub provider_id: ProviderId,
    pub key: String,
    pub guid: String,
    pub indexer_id: i64,
    pub title: String,
    pub indexer: String,
    pub detail_url: Option<String>,
    pub size_bytes: u64,
    pub protocol: RemoteReleaseProtocol,
    pub seeders: Option<u32>,
    pub categories: Vec<RemoteReleaseCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteReleaseSearchRequest {
    pub author: Option<String>,
    pub title: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteReleaseSearchResponse {
    pub provider_id: ProviderId,
    pub releases: Vec<RemoteRelease>,
    pub diagnostics: Vec<RemoteSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteReleaseGrabRequest {
    pub release: RemoteRelease,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteReleaseGrabResponse {
    pub provider_id: ProviderId,
    pub accepted: bool,
    pub unconfirmed: bool,
    pub message: String,
    pub diagnostics: Vec<RemoteSourceDiagnostic>,
}

#[derive(Clone)]
pub struct RemoteIndexerConnectionUpdate {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteIndexerConnectionTestResult {
    pub ok: bool,
    pub message: String,
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
    pub selections: Vec<AcquisitionSelection>,
}

/// What one acquisition job does. Every kind shares progress, cancel,
/// staging, and purge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AcquisitionWork {
    /// Download, decrypt, and import these titles.
    Titles(AcquisitionPlan),
    /// Save one title's companion PDF into the output folder, with no audio.
    Pdf { title_id: String },
}

/// What a finished job holds for the session.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AcquiredContent {
    Titles {
        files: Vec<MaterializedSourceFile>,
        assets: Vec<SupplementalAsset>,
    },
    Pdf {
        pdf: StagedPdf,
        title: PdfTitle,
    },
}

/// A downloaded companion PDF in the job's staging folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagedPdf {
    pub(crate) path: PathBuf,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}

/// The provider's facts that name where a title's output goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PdfTitle {
    pub(crate) title: String,
    pub(crate) authors: Vec<String>,
    pub(crate) series: Option<String>,
    pub(crate) series_part: Option<String>,
    pub(crate) release_date: Option<String>,
}

/// A finished job as the session's handoff receives it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AcquiredJob {
    pub(crate) job_id: String,
    pub(crate) content: AcquiredContent,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAcquisitionFailureKind {
    AuthRequired,
    ProviderPrivateProtocolFailed,
    /// Audible answered the license request and refused it; the message says
    /// why (for example, a temporary download limit on the account).
    LicenseDenied,
    ProtectedUnsupported,
    DownloadFailed,
    MaterializationFailed,
    /// Decryption succeeded and the title is import-ready, but the encrypted
    /// staging source could not be purged. Non-blocking: the startup session
    /// sweep removes it on next launch.
    ProtectedSourcePurgeFailed,
    ValidationFailed,
    SupplementalPdfFailed,
    /// The user asked for the title's Supplemental PDF and Audible offers
    /// none. Non-blocking: the audiobook is imported without it.
    SupplementalPdfUnavailable,
    ReleaseSearchFailed,
    ReleaseGrabFailed,
    Cancelled,
}

impl RemoteAcquisitionFailureKind {
    pub fn blocks_title(&self) -> bool {
        match self {
            Self::ProtectedSourcePurgeFailed | Self::SupplementalPdfUnavailable => false,
            Self::AuthRequired
            | Self::ProviderPrivateProtocolFailed
            | Self::LicenseDenied
            | Self::ProtectedUnsupported
            | Self::DownloadFailed
            | Self::MaterializationFailed
            | Self::ValidationFailed
            | Self::SupplementalPdfFailed
            | Self::ReleaseSearchFailed
            | Self::ReleaseGrabFailed
            | Self::Cancelled => true,
        }
    }
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
    #[serde(skip)]
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
    pub progress: AcquisitionProgress,
    pub diagnostics: Vec<RemoteSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionSnapshot {
    #[serde(flatten)]
    pub job: AcquisitionJob,
    pub state: AcquisitionJobState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AcquisitionJobState {
    Running,
    /// The files are acquired and the session is taking them over.
    HandingOver,
    Done {
        ending: JobEnding,
    },
}

impl AcquisitionJobState {
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done { .. })
    }

    pub fn handoff(&self) -> Option<&AcquisitionHandoff> {
        match self {
            Self::Done {
                ending: JobEnding::Handoff { handoff },
            } => Some(handoff),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum JobEnding {
    Handoff { handoff: AcquisitionHandoff },
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct RemoteRevisions {
    pub(super) remote: u64,
    pub(super) library: u64,
    pub(super) acquisition: u64,
}
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAcquisitionSnapshot {
    #[specta(type = specta_typescript::Number)]
    pub revision: u64,
    /// An accepted request whose job is not registered yet.
    #[serde(skip)]
    pub acquiring: bool,
    pub job: Option<Box<AcquisitionSnapshot>>,
}
impl RemoteAcquisitionSnapshot {
    pub(crate) fn busy(&self) -> bool {
        self.acquiring || self.job.as_ref().is_some_and(|job| !job.state.is_done())
    }
}
impl AcquisitionJob {
    pub(crate) fn new(job_id: &str, work: &AcquisitionWork) -> Self {
        let mut progress = acquisition_progress(AcquisitionStage::License, Some(0.0), None, None);
        if let AcquisitionWork::Pdf { title_id } = work {
            progress.message = PDF_LOOKUP_MESSAGE.to_string();
            progress.current_title_id = Some(title_id.clone());
        }
        Self {
            job_id: job_id.to_string(),
            provider_id: ProviderId::Audible,
            progress,
            diagnostics: Vec::new(),
        }
    }
}
impl std::ops::Deref for AcquisitionSnapshot {
    type Target = AcquisitionJob;
    fn deref(&self) -> &Self::Target {
        &self.job
    }
}

/// What the engine did with a finished job's content, by job kind.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "job", content = "outcome", rename_all = "camelCase")]
pub enum AcquisitionHandoff {
    Titles(TitlesHandoff),
    Pdf(PdfHandoff),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TitlesHandoff {
    Imported {
        count: usize,
    },
    /// Nothing was imported, so the engine removes the staged files.
    Removed {
        reason: HandoffRefusal,
    },
}

/// A PDF job's staged copy is removed either way.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PdfHandoff {
    Saved {
        path: PathBuf,
    },
    NotSaved {
        error: crate::errors::AppErrorEnvelope,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HandoffRefusal {
    ImportFailed {
        error: crate::errors::AppErrorEnvelope,
    },
    /// The files were already listed, or a reset dropped the import.
    NothingAdded,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
