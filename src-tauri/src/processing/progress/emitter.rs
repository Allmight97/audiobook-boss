//! Tauri progress event emitter

use super::{
    emit_progress_event, EventStage, ProgressEvent, PROGRESS_ANALYZING_END,
    PROGRESS_ANALYZING_START, PROGRESS_CLEANUP, PROGRESS_COMPLETE, PROGRESS_CONVERTING_MAX,
    PROGRESS_CONVERTING_START, PROGRESS_FINALIZING, PROGRESS_METADATA_START,
};
use crate::processing::OperationKind;
use crate::processing::ProcessingStage;
use std::sync::Arc;
use tauri::Window;

type ProgressListener = Arc<dyn Fn(&ProgressEvent) + Send + Sync>;

/// Centralized progress event emitter
pub struct ProgressEmitter {
    /// Backend operation family this emitter reports for
    operation_kind: OperationKind,
    /// Optional Tauri window for event emission (None in headless/test runs)
    window: Option<Window>,
    /// Optional WorkRuntime operation identifier
    operation_id: Option<String>,
    /// Optional job identifier for parallel batch processing
    job_id: Option<String>,
    /// Optional input index for batch processing
    input_index: Option<usize>,
    progress_listener: Option<ProgressListener>,
}

impl ProgressEmitter {
    /// Creates a new progress emitter without job tracking (single-job mode)
    pub fn new(window: Window) -> Self {
        Self::for_operation(window, OperationKind::ProcessingBatch)
    }

    /// Creates a new progress emitter scoped to an operation kind.
    pub fn for_operation(window: Window, operation_kind: OperationKind) -> Self {
        Self {
            operation_kind,
            window: Some(window),
            operation_id: None,
            job_id: None,
            input_index: None,
            progress_listener: None,
        }
    }

    /// Creates a progress emitter with job tracking for parallel processing
    pub fn with_job_id(window: Window, job_id: String) -> Self {
        Self {
            operation_kind: OperationKind::ProcessingBatch,
            window: Some(window),
            operation_id: None,
            job_id: Some(job_id),
            input_index: None,
            progress_listener: None,
        }
    }

    /// Creates a progress emitter with job tracking and input index context.
    ///
    /// `window` is optional: background (WorkRuntime) operations pass `None` and
    /// report exclusively through the progress listener (snapshots), so they do not
    /// also emit `processing-progress` to the window.
    pub fn with_context(
        window: Option<Window>,
        operation_kind: OperationKind,
        operation_id: Option<String>,
        job_id: Option<String>,
        input_index: Option<usize>,
    ) -> Self {
        Self {
            operation_kind,
            window,
            operation_id,
            job_id,
            input_index,
            progress_listener: None,
        }
    }

    /// Creates a headless emitter (no UI events emitted).
    pub fn headless() -> Self {
        Self::headless_for(OperationKind::ProcessingBatch)
    }

    /// Creates a headless emitter for a specific operation kind.
    pub fn headless_for(operation_kind: OperationKind) -> Self {
        Self {
            operation_kind,
            window: None,
            operation_id: None,
            job_id: None,
            input_index: None,
            progress_listener: None,
        }
    }

    pub(crate) fn with_progress_listener(
        mut self,
        progress_listener: Option<ProgressListener>,
    ) -> Self {
        self.progress_listener = progress_listener;
        self
    }

    /// Returns the job ID if set
    pub fn job_id(&self) -> Option<&str> {
        self.job_id.as_deref()
    }

    fn terminal_event(&self, stage: EventStage, message: &str) -> ProgressEvent {
        ProgressEvent {
            operation_id: self.operation_id.clone(),
            operation_kind: self.operation_kind,
            stage,
            percentage: if stage == EventStage::Skipped {
                100.0
            } else {
                0.0
            },
            message: message.to_string(),
            current_file: None,
            eta_seconds: None,
            job_id: self.job_id.clone(),
            input_index: self.input_index,
        }
    }

    fn emit_terminal_event(&self, stage: EventStage, message: &str) {
        let event = self.terminal_event(stage, message);
        self.notify_listener(&event);
        if let Some(window) = &self.window {
            emit_progress_event(window, &event);
        }
    }

    /// Emits analyzing start event
    pub fn emit_analyzing_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Analyzing,
            PROGRESS_ANALYZING_START,
            message,
            None,
            None,
        );
    }

    /// Emits analyzing end event
    pub fn emit_analyzing_end(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Analyzing,
            PROGRESS_ANALYZING_END,
            message,
            None,
            None,
        );
    }

    /// Emits converting start event
    pub fn emit_converting_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Converting,
            PROGRESS_CONVERTING_START,
            message,
            None,
            None,
        );
    }

    /// Emits converting progress with file info
    pub fn emit_converting_progress(
        &self,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        let clamped_percentage = percentage.min(PROGRESS_CONVERTING_MAX);
        self.emit_event(
            ProcessingStage::Converting,
            clamped_percentage,
            message,
            current_file,
            eta_seconds,
        );
    }

    /// Emits metadata writing start event
    pub fn emit_metadata_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_METADATA_START,
            message,
            None,
            None,
        );
    }

    /// Emits finalizing event
    pub fn emit_finalizing(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_FINALIZING,
            message,
            None,
            None,
        );
    }

    /// Emits cleanup event
    pub fn emit_cleanup(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Completed,
            PROGRESS_CLEANUP,
            message,
            None,
            None,
        );
    }

    /// Emits completion event
    pub fn emit_complete(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Completed,
            PROGRESS_COMPLETE,
            message,
            None,
            None,
        );
    }

    /// Emits terminal failed event.
    pub fn emit_terminal_failed(&self, message: &str) {
        self.emit_terminal_event(EventStage::Failed, message);
    }

    /// Emits terminal cancelled event.
    pub fn emit_terminal_cancelled(&self, message: &str) {
        self.emit_terminal_event(EventStage::Cancelled, message);
    }

    /// Emits terminal skipped event.
    pub fn emit_terminal_skipped(&self, message: &str) {
        self.emit_terminal_event(EventStage::Skipped, message);
    }

    /// Emits cancelled event (special-case stage not represented in ProcessingStage enum)
    pub fn emit_cancelled(&self, message: &str) {
        self.emit_terminal_cancelled(message);
    }
    /// Emits custom progress event with all parameters
    pub fn emit_custom(
        &self,
        stage: ProcessingStage,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        self.emit_event(stage, percentage, message, current_file, eta_seconds);
    }

    /// Internal method to emit progress events
    fn emit_event(
        &self,
        stage: ProcessingStage,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        let event = ProgressEvent {
            operation_id: self.operation_id.clone(),
            operation_kind: self.operation_kind,
            stage: EventStage::from(&stage),
            percentage,
            message: message.to_string(),
            current_file,
            eta_seconds,
            job_id: self.job_id.clone(),
            input_index: self.input_index,
        };

        self.notify_listener(&event);

        if let Some(window) = &self.window {
            emit_progress_event(window, &event);
        }
    }

    fn notify_listener(&self, event: &ProgressEvent) {
        if let Some(listener) = &self.progress_listener {
            listener(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_events_share_context_and_reset_progress() {
        let emitter = ProgressEmitter {
            operation_kind: OperationKind::ProcessingBatch,
            window: None,
            operation_id: Some("op-123".to_string()),
            job_id: Some("job-123".to_string()),
            input_index: Some(7),
            progress_listener: None,
        };

        let failed = emitter.terminal_event(EventStage::Failed, "failed");
        let cancelled = emitter.terminal_event(EventStage::Cancelled, "cancelled");
        let skipped = emitter.terminal_event(EventStage::Skipped, "skipped");

        assert_eq!(failed.stage, EventStage::Failed);
        assert_eq!(cancelled.stage, EventStage::Cancelled);
        assert_eq!(skipped.stage, EventStage::Skipped);
        assert_eq!(failed.operation_kind, OperationKind::ProcessingBatch);
        assert_eq!(cancelled.operation_kind, OperationKind::ProcessingBatch);
        assert_eq!(skipped.operation_kind, OperationKind::ProcessingBatch);
        assert_eq!(failed.operation_id, Some("op-123".to_string()));
        assert_eq!(cancelled.operation_id, Some("op-123".to_string()));
        assert_eq!(skipped.operation_id, Some("op-123".to_string()));
        assert_eq!(failed.percentage, 0.0);
        assert_eq!(cancelled.percentage, 0.0);
        assert_eq!(skipped.percentage, 100.0);
        assert_eq!(failed.job_id, Some("job-123".to_string()));
        assert_eq!(cancelled.job_id, Some("job-123".to_string()));
        assert_eq!(skipped.job_id, Some("job-123".to_string()));
        assert_eq!(failed.input_index, Some(7));
        assert_eq!(cancelled.input_index, Some(7));
        assert_eq!(skipped.input_index, Some(7));
        assert_eq!(failed.current_file, None);
        assert_eq!(cancelled.current_file, None);
        assert_eq!(skipped.current_file, None);
        assert_eq!(failed.eta_seconds, None);
        assert_eq!(cancelled.eta_seconds, None);
        assert_eq!(skipped.eta_seconds, None);
    }
}
