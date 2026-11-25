mod emitter;

use serde::Serialize;

pub const PROGRESS_ANALYZING_START: f32 = 0.0;

pub const PROGRESS_CONVERTING_START: f32 = 10.0;
pub const PROGRESS_CONVERTING_MAX: f32 = 79.0;

pub const PROGRESS_METADATA_START: f32 = 90.0;
pub const PROGRESS_FINALIZING: f32 = 95.0;
pub const PROGRESS_COMPLETE: f32 = 100.0;

const PROGRESS_CONVERTING_SPAN: f32 = PROGRESS_CONVERTING_MAX - PROGRESS_CONVERTING_START;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EventStage {
    Analyzing,
    Converting,
    Writing,
    Completed,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Clone)]
pub struct ProgressEvent {
    pub stage: EventStage,
    pub percentage: f32,
    pub message: String,
    pub current_file: Option<String>,
    pub eta_seconds: Option<f64>,
    pub job_id: Option<String>,
    pub input_index: Option<usize>,
}

pub fn converting_percentage_from_seconds(current_seconds: f64, total_duration: f64) -> f32 {
    if total_duration <= 0.0 {
        return PROGRESS_CONVERTING_START;
    }
    converting_percentage((current_seconds / total_duration) as f32)
}

pub fn converting_percentage(fraction: f32) -> f32 {
    PROGRESS_CONVERTING_START + fraction.clamp(0.0, 1.0) * PROGRESS_CONVERTING_SPAN
}

pub(crate) use emitter::ProgressEmitter;
pub use emitter::ProgressEventListener;

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
