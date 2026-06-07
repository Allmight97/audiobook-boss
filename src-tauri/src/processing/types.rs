use super::lifecycle::{OperationKind, OperationResultSummary};
use crate::audio;
use crate::audio::EncoderSettings;
use crate::errors::AppErrorEnvelope;
pub use crate::output_artifact::{
    CollisionPolicy, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    OutputNamingConfig, OutputReviewRequirement, PlannedOutput, PlannedOutputAction,
};
pub use abb_processing_core::ProcessResultStatus;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum JobType {
    Merge,
    Batch,
}

impl From<JobType> for OperationKind {
    fn from(value: JobType) -> Self {
        match value {
            JobType::Merge => OperationKind::ProcessingMerge,
            JobType::Batch => OperationKind::ProcessingBatch,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessPayload {
    pub input_files: Vec<String>,
    /// Session/workbench identities aligned to `input_files`; used for acquired
    /// source sidecars without replacing path as the filesystem source label.
    pub input_ids: Option<Vec<Option<String>>>,
    pub output_dir: String,
    pub settings: EncoderSettings,
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
    pub results: Vec<ProcessResultEntry>,
}

impl ProcessCommandResult {
    pub fn new(job_type: JobType, results: Vec<ProcessResultEntry>) -> Self {
        let summary = abb_processing_core::summarize_result_statuses(
            results.iter().map(|result| result.status),
        );

        Self {
            job_type,
            summary,
            results,
        }
    }
}
