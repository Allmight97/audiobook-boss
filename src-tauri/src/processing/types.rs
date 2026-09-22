use super::lifecycle::{OperationKind, OperationResultSummary};
use crate::audio;
use crate::audio::EncoderSettings;
use crate::errors::AppErrorEnvelope;
use crate::output_artifact::{CollisionPolicy, OutputNamingConfig, PlannedOutput};
pub use abb_processing_core::ProcessResultStatus;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum JobType {
    Merge,
    Batch,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AudioHandling {
    Encode,
    Preserve,
}

impl From<JobType> for OperationKind {
    fn from(value: JobType) -> Self {
        match value {
            JobType::Merge => OperationKind::ProcessingMerge,
            JobType::Batch => OperationKind::ProcessingBatch,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleSource {
    pub path: String,
    pub input_id: Option<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessPayload {
    /// One metadata anchor per output title. Source order never changes this identity.
    pub input_files: Vec<String>,
    /// Ordered sources for multi-file titles, keyed by their metadata anchor.
    pub title_sources: Option<HashMap<String, Vec<TitleSource>>>,
    pub chapter_plans: Option<HashMap<String, crate::metadata::ChapterPlan>>,
    /// Session/workbench identities aligned to `input_files`; used for acquired
    /// source sidecars without replacing path as the filesystem source label.
    pub input_ids: Option<Vec<Option<String>>>,
    pub output_dir: String,
    pub settings: Option<EncoderSettings>,
    /// Per-input audio handling aligned with `input_files`. Absent means encode
    /// every input, preserving the existing request shape.
    pub audio_handling: Option<Vec<AudioHandling>>,
    /// Sample rate from frontend (optional, defaults to Auto)
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>,
    /// Output naming configuration (defaults to ABS-compatible)
    pub output_naming: Option<OutputNamingConfig>,
    /// Explicit collision policy selected by the user after preflight review.
    pub collision_policy: Option<CollisionPolicy>,
    /// Signature returned by preflight so execution can reject stale destination assumptions.
    pub preflight_signature: Option<String>,
    /// Supplemental assets keyed by input id. These are committed only after a
    /// matching final batch audiobook succeeds.
    pub supplemental_assets_by_input_id: Option<HashMap<String, Vec<SupplementalProcessingAsset>>>,
}

impl ProcessPayload {
    pub(crate) fn sources_for(&self, index: usize) -> Vec<TitleSource> {
        let anchor = &self.input_files[index];
        self.title_sources
            .as_ref()
            .and_then(|groups| groups.get(anchor))
            .cloned()
            .unwrap_or_else(|| {
                vec![TitleSource {
                    path: anchor.clone(),
                    input_id: self
                        .input_ids
                        .as_ref()
                        .and_then(|ids| ids.get(index))
                        .cloned()
                        .flatten(),
                }]
            })
    }

    pub(crate) fn validate_title_sources(&self) -> crate::errors::Result<()> {
        use crate::errors::AppError;
        if self.title_sources.as_ref().is_some_and(|groups| {
            groups
                .keys()
                .any(|anchor| !self.input_files.contains(anchor))
        }) {
            return Err(AppError::InvalidInput(
                "Title sources must belong to a requested output title.".into(),
            ));
        }
        if self.job_type == Some(JobType::Merge)
            && self
                .title_sources
                .as_ref()
                .is_some_and(|groups| !groups.is_empty())
        {
            return Err(AppError::InvalidInput(
                "Title groups cannot be combined with a global merge request.".into(),
            ));
        }
        let mut paths = std::collections::HashSet::new();
        for (index, anchor) in self.input_files.iter().enumerate() {
            let sources = self.sources_for(index);
            if sources.is_empty() || !sources.iter().any(|source| &source.path == anchor) {
                return Err(AppError::InvalidInput(
                    "Every title needs its metadata source among its ordered audio sources.".into(),
                ));
            }
            for source in sources {
                if !paths.insert(source.path) {
                    return Err(AppError::InvalidInput(
                        "An audio source can belong to only one output title.".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn resolved_audio_handling(&self) -> crate::errors::Result<Vec<AudioHandling>> {
        let handling = self
            .audio_handling
            .clone()
            .unwrap_or_else(|| vec![AudioHandling::Encode; self.input_files.len()]);
        if handling.len() != self.input_files.len() {
            return Err(crate::errors::AppError::InvalidInput(
                "Audio handling must align with the processing input files.".into(),
            ));
        }
        if self.job_type == Some(JobType::Merge) && handling.contains(&AudioHandling::Preserve) {
            return Err(crate::errors::AppError::InvalidInput(
                "Preserving grouped audio requires a title-source request.".into(),
            ));
        }
        Ok(handling)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SupplementalProcessingAsset {
    pub asset_id: String,
    pub input_id: String,
    pub title_id: String,
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub type ProcessResultSummary = OperationResultSummary;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingPreflightPlan {
    pub job_type: JobType,
    pub preview_seconds: Option<f64>,
    pub collision_policy: CollisionPolicy,
    pub plan_signature: String,
    pub outputs: Vec<PlannedOutput>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResultEntry {
    pub input_index: Option<usize>,
    pub status: ProcessResultStatus,
    pub message: String,
    pub error: Option<AppErrorEnvelope>,
    pub preview_file_path: Option<String>,
    pub preview_actual_seconds: Option<f64>,
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCommandResult {
    pub job_type: JobType,
    pub summary: ProcessResultSummary,
    /// Backend-owned terminal classification of the run. The UI renders this
    /// instead of re-deriving success/mixed/failed/skipped/cancelled precedence.
    pub terminal_class: abb_processing_core::RunTerminalClass,
    pub results: Vec<ProcessResultEntry>,
}

impl ProcessCommandResult {
    pub fn new(job_type: JobType, results: Vec<ProcessResultEntry>) -> Self {
        let summary = abb_processing_core::summarize_result_statuses(
            results.iter().map(|result| result.status),
        );
        let terminal_class = abb_processing_core::classify_run_terminal(&summary);

        Self {
            job_type,
            summary,
            terminal_class,
            results,
        }
    }
}
