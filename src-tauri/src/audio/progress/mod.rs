//! Progress module: shared types, utilities, and submodule re-exports

mod emitter;
mod state;

use crate::audio::constants::*;
use serde::Serialize;

// ============================================================================
// Data Contract (UI boundary)
// ============================================================================

/// Progress event structure for frontend communication
#[derive(Clone, Serialize)]
pub struct ProgressEvent {
    /// Current processing stage name
    pub stage: String,
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
