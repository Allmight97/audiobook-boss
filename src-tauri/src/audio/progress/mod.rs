//! Progress module: shared types, utilities, and submodule re-exports

mod emitter;
mod state;

use crate::audio::constants::*;
use crate::audio::ProcessingStage;
use serde::Serialize;

// ============================================================================
// Data Contract (UI boundary)
// ============================================================================

/// Stage identifier emitted on `processing-progress` events.
///
/// This enum defines the wire format the frontend consumes. It is distinct
/// from [`ProcessingStage`] (internal orchestration enum that carries data
/// such as `Failed(String)`) because the UI only needs a simple discriminator.
/// Serde `snake_case` serialization keeps the wire values identical to the
/// pre-enum string protocol (`"analyzing"`, `"converting"`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EventStage {
    Analyzing,
    Converting,
    Writing,
    Completed,
    Failed,
    Cancelled,
}

impl From<&ProcessingStage> for EventStage {
    fn from(stage: &ProcessingStage) -> Self {
        match stage {
            ProcessingStage::Analyzing => EventStage::Analyzing,
            ProcessingStage::Converting => EventStage::Converting,
            ProcessingStage::WritingMetadata => EventStage::Writing,
            ProcessingStage::Completed => EventStage::Completed,
            ProcessingStage::Failed(_) => EventStage::Failed,
        }
    }
}

impl From<ProcessingStage> for EventStage {
    fn from(stage: ProcessingStage) -> Self {
        EventStage::from(&stage)
    }
}

/// Progress event structure for frontend communication
#[derive(Clone, Serialize, specta::Type)]
pub struct ProgressEvent {
    /// Current processing stage
    pub stage: EventStage,
    /// Progress percentage (0-100)
    pub percentage: f32,
    /// Human-readable status message
    pub message: String,
    /// Currently processing file (if applicable)
    pub current_file: Option<String>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
    /// Job identifier for parallel batch processing (optional for backward compat)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    /// Original input index for batch processing (optional for backward compat)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_index: Option<usize>,
}

/// Batch queue snapshot for frontend communication
#[derive(Clone, Serialize, specta::Type)]
pub struct QueueEvent {
    pub items: Vec<QueueItem>,
    pub max_concurrent: usize,
}

/// Single queued item in a batch run
#[derive(Clone, Serialize, specta::Type)]
pub struct QueueItem {
    pub input_index: usize,
    pub file_path: String,
}

impl tauri_specta::Event for ProgressEvent {
    const NAME: &'static str = "processing-progress";
}

impl tauri_specta::Event for QueueEvent {
    const NAME: &'static str = "processing-queue";
}

// ============================================================================
// Pure Math Utilities
// ============================================================================

/// Formats estimated time remaining into a human-readable string
pub fn format_eta(seconds: f64) -> String {
    if seconds < SECONDS_PER_MINUTE {
        format!("{seconds:.0}s")
    } else {
        let minutes = (seconds / SECONDS_PER_MINUTE) as u32;
        let remaining_seconds = seconds % SECONDS_PER_MINUTE;
        format!("{minutes}m {remaining_seconds:.0}s")
    }
}

/// Calculates progress percentage within a stage range
pub fn calculate_stage_progress(
    current: f64,
    total: f64,
    start_percentage: f32,
    end_percentage: f32,
) -> f32 {
    if total <= 0.0 {
        return start_percentage;
    }

    let progress_ratio = (current / total) as f32;
    let range = end_percentage - start_percentage;
    start_percentage + (progress_ratio * range)
}

/// Converts seconds to converting-stage UI percentage
pub fn converting_percentage_from_seconds(current_seconds: f64, total_duration: f64) -> f32 {
    if total_duration <= 0.0 {
        return PROGRESS_CONVERTING_START;
    }
    let ratio = (current_seconds / total_duration).clamp(0.0, 1.0);
    let pct = PROGRESS_CONVERTING_START as f64 + ratio * PROGRESS_RANGE_MULTIPLIER;
    pct as f32
}

// ============================================================================
// Re-exports
// ============================================================================

pub use emitter::ProgressEmitter;
pub use state::ProgressReporter;
