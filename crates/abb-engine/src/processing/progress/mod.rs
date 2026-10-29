mod emitter;
mod eta;

pub use eta::EtaEstimator;

use crate::processing::lifecycle::OperationKind;
use crate::processing::ProcessingStage;
use serde::Serialize;

pub const PROGRESS_ANALYZING_START: f32 = 0.0;

pub const PROGRESS_CONVERTING_START: f32 = 10.0;
pub const PROGRESS_CONVERTING_MAX: f32 = 79.0;

pub const PROGRESS_METADATA_START: f32 = 90.0;
pub const PROGRESS_FINALIZING: f32 = 95.0;
pub const PROGRESS_COMPLETE: f32 = 100.0;

const PROGRESS_CONVERTING_SPAN: f64 = (PROGRESS_CONVERTING_MAX - PROGRESS_CONVERTING_START) as f64;

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

#[derive(Clone, Serialize, specta::Type)]
pub struct ProgressEvent {
    pub operation_kind: OperationKind,
    pub stage: EventStage,
    /// Progress percentage (0-100)
    #[specta(type = specta_typescript::Number)]
    pub percentage: f32,
    pub message: String,
    pub current_file: Option<String>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
    /// Job identifier when this event is tied to a registered job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    /// Original input index when this event maps to one selected input
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_index: Option<usize>,
}

pub fn converting_percentage_from_seconds(current_seconds: f64, total_duration: f64) -> f32 {
    if total_duration <= 0.0 {
        return PROGRESS_CONVERTING_START;
    }
    let ratio = (current_seconds / total_duration).clamp(0.0, 1.0);
    let pct = PROGRESS_CONVERTING_START as f64 + ratio * PROGRESS_CONVERTING_SPAN;
    pct as f32
}

pub use emitter::{EmitContext, ProgressEmitter};

#[cfg(test)]
mod tests {
    use super::{
        converting_percentage_from_seconds, PROGRESS_CONVERTING_MAX, PROGRESS_CONVERTING_START,
    };

    #[test]
    fn converting_percentage_maps_seconds_into_converting_range() {
        assert!(converting_percentage_from_seconds(0.0, 100.0) >= 10.0);
        assert!(converting_percentage_from_seconds(50.0, 100.0) > 10.0);
        assert!(converting_percentage_from_seconds(100.0, 100.0) <= PROGRESS_CONVERTING_MAX);
    }

    #[test]
    fn converting_percentage_never_exceeds_converting_max() {
        assert_eq!(
            converting_percentage_from_seconds(100.0, 100.0),
            PROGRESS_CONVERTING_MAX
        );
        assert_eq!(
            converting_percentage_from_seconds(500.0, 100.0),
            PROGRESS_CONVERTING_MAX
        );
        assert_eq!(
            converting_percentage_from_seconds(0.0, 100.0),
            PROGRESS_CONVERTING_START
        );
    }

    #[test]
    fn converting_percentage_handles_empty_or_unknown_duration() {
        assert_eq!(converting_percentage_from_seconds(0.0, 0.0), 10.0);
        assert!(converting_percentage_from_seconds(100.0, 0.0) >= 10.0);
    }
}
