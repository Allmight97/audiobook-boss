//! Progress module: shared types, utilities, and submodule re-exports

mod emitter;

use crate::processing::lifecycle::OperationKind;
use crate::processing::ProcessingStage;
use serde::Serialize;
use tauri::Emitter;

// ============================================================================
// Event Names And Progress Math
// ============================================================================

/// Progress event name for frontend communication.
pub const PROGRESS_EVENT_NAME: &str = "processing-progress";
/// Queue event name for batch/operation queue snapshots.
pub const QUEUE_EVENT_NAME: &str = "processing-queue";

/// Progress percentage at the start of the analyzing stage.
pub const PROGRESS_ANALYZING_START: f32 = 0.0;
/// Progress percentage at the end of the analyzing stage.
pub const PROGRESS_ANALYZING_END: f32 = 10.0;

/// Progress percentage range for the converting stage.
pub const PROGRESS_CONVERTING_START: f32 = 10.0;
/// Max converting percentage to avoid reaching finalization too early.
pub const PROGRESS_CONVERTING_MAX: f32 = 79.0;
/// Range from converting start to nominal converting end.
pub const PROGRESS_CONVERTING_RANGE: f32 = 70.0;

/// Progress percentage for metadata writing.
pub const PROGRESS_METADATA_START: f32 = 90.0;
/// Progress percentage for final artifact finalization.
pub const PROGRESS_FINALIZING: f32 = 95.0;
/// Progress percentage for cleanup.
pub const PROGRESS_CLEANUP: f32 = 98.0;
/// Progress percentage for complete.
pub const PROGRESS_COMPLETE: f32 = 100.0;

/// Progress percentage calculation range (maps file progress to UI progress).
pub const PROGRESS_RANGE_MULTIPLIER: f64 = 70.0;
/// Seconds per minute for ETA formatting.
pub const SECONDS_PER_MINUTE: f64 = 60.0;
/// Weight for metadata writing in progress calculations.
pub const PROGRESS_METADATA_WEIGHT: f32 = 5.0;

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

/// Progress event structure for frontend communication
#[derive(Clone, Serialize, specta::Type)]
pub struct ProgressEvent {
    /// Backend operation family that emitted this event
    pub operation_kind: OperationKind,
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
    /// Job identifier when this event is tied to a registered job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    /// Original input index when this event maps to one selected input
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_index: Option<usize>,
}

/// Batch queue snapshot for frontend communication
#[derive(Clone, Serialize, specta::Type)]
pub struct QueueEvent {
    pub operation_kind: OperationKind,
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
    const NAME: &'static str = PROGRESS_EVENT_NAME;
}

impl tauri_specta::Event for QueueEvent {
    const NAME: &'static str = QUEUE_EVENT_NAME;
}

impl QueueEvent {
    pub fn new(
        operation_kind: OperationKind,
        items: Vec<QueueItem>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            operation_kind,
            items,
            max_concurrent,
        }
    }
}

/// Emits a progress event through the lifecycle-owned event name.
pub fn emit_progress_event(window: &tauri::Window, event: &ProgressEvent) {
    let _ = window.emit(PROGRESS_EVENT_NAME, event);
}

/// Emits a queue event through the lifecycle-owned event name.
pub fn emit_queue_event(window: &tauri::Window, event: &QueueEvent) {
    let _ = window.emit(QUEUE_EVENT_NAME, event);
}

// ============================================================================
// Pure Math Utilities
// ============================================================================

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

pub use emitter::{EmitContext, ProgressEmitter};
