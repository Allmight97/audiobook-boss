use crate::audio;
pub use crate::audio::output_path::{NamingPreset, OutputNamingConfig};
use crate::audio::settings_encoder::EncoderSettings;

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
    /// Sample rate from frontend (optional, defaults to Auto)
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>,
    /// Output naming configuration (defaults to ABS-compatible)
    pub output_naming: Option<OutputNamingConfig>,
}

/// Processes multiple audio files into a single M4B audiobook
/// Merges files with specified settings and optional metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProcessResultStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResultSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResultEntry {
    pub input_index: Option<usize>,
    pub status: ProcessResultStatus,
    pub message: String,
    pub error: Option<String>,
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
        let failed = results.len().saturating_sub(succeeded);
        let summary = ProcessResultSummary {
            total: results.len(),
            succeeded,
            failed,
        };

        Self {
            job_type,
            summary,
            results,
        }
    }
}
