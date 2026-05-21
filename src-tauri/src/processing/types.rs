use super::lifecycle::{OperationKind, OperationResultSummary};
use crate::audio;
use crate::audio::{EncoderSettings, ExternalToolchainPreference};
use crate::errors::AppErrorEnvelope;
pub use crate::output_artifact::{
    CollisionPolicy, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    OutputNamingConfig, OutputReviewRequirement, PlannedOutput, PlannedOutputAction,
};

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

#[derive(serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessPayload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    pub external_toolchain: Option<ExternalToolchainPreference>,
    /// Sample rate from frontend (optional, defaults to Auto)
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>,
    /// Output naming configuration (defaults to ABS-compatible)
    pub output_naming: Option<OutputNamingConfig>,
    /// Explicit collision policy selected by the user after preflight review.
    pub collision_policy: Option<CollisionPolicy>,
    /// Signature returned by preflight so execution can reject stale destination assumptions.
    pub preflight_signature: Option<String>,
}

/// Processes multiple audio files into a single M4B audiobook
/// Merges files with specified settings and optional metadata
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProcessResultStatus {
    Success,
    Skipped,
    Cancelled,
    Failed,
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
        let succeeded = results
            .iter()
            .filter(|result| result.status == ProcessResultStatus::Success)
            .count();
        let skipped = results
            .iter()
            .filter(|result| result.status == ProcessResultStatus::Skipped)
            .count();
        let cancelled = results
            .iter()
            .filter(|result| result.status == ProcessResultStatus::Cancelled)
            .count();
        let failed = results
            .len()
            .saturating_sub(succeeded + skipped + cancelled);
        let summary = OperationResultSummary {
            total: results.len(),
            succeeded,
            skipped,
            cancelled,
            failed,
        };

        Self {
            job_type,
            summary,
            results,
        }
    }
}
