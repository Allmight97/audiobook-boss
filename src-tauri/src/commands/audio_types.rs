use crate::audio;
pub use crate::audio::output_path::FilenamePattern;
use crate::audio::settings_encoder::EncoderSettings;

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobType {
    Merge,
    Batch,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    /// Sample rate from frontend (optional, defaults to Auto)
    pub sample_rate: Option<audio::SampleRateConfig>,
    pub job_type: Option<JobType>,
    /// Optional toggle for metadata-based subdirectory generation (default true)
    pub use_subdir_pattern: Option<bool>,
    /// Optional filename pattern (default title-year)
    pub filename_pattern: Option<FilenamePattern>,
}

/// Processes multiple audio files into a single M4B audiobook
/// Merges files with specified settings and optional metadata
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCommandResult {
    pub message: String,
    pub preview_file_path: Option<String>,
    pub preview_actual_seconds: Option<f64>,
    pub job_id: String,
}
