use crate::audio;
pub use crate::audio::output_path::{
    CollisionPolicy, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
    OutputNamingConfig, PlannedOutput, PlannedOutputAction,
};
use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::toolchain::ExternalToolchainPreference;
use crate::errors::AppErrorEnvelope;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum JobType {
    Merge,
    Batch,
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProcessResultStatus {
    Success,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResultSummary {
    pub total: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
}

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
        let failed = results.len().saturating_sub(succeeded + skipped);
        let summary = ProcessResultSummary {
            total: results.len(),
            succeeded,
            skipped,
            failed,
        };

        Self {
            job_type,
            summary,
            results,
        }
    }
}
