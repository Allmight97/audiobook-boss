//! Progress event emitter

use super::{
    EventStage, ProgressEvent, PROGRESS_ANALYZING_START, PROGRESS_CLEANUP, PROGRESS_COMPLETE,
    PROGRESS_CONVERTING_MAX, PROGRESS_CONVERTING_START, PROGRESS_FINALIZING,
    PROGRESS_METADATA_START,
};
use crate::processing::OperationKind;
use crate::processing::ProcessingStage;
use std::sync::Arc;

type ProgressListener = Arc<dyn Fn(&ProgressEvent) + Send + Sync>;

/// Emit context shared by progress and terminal events: the operation
/// family, and (for batch items) the job id and input index. Bundled so callers
/// pass one value instead of a positional tuple.
#[derive(Clone)]
pub struct EmitContext {
    pub operation_kind: OperationKind,
    pub job_id: Option<String>,
    pub input_index: Option<usize>,
}

/// Centralized progress event emitter
pub struct ProgressEmitter {
    /// Backend operation family this emitter reports for
    operation_kind: OperationKind,
    /// Optional job identifier for parallel batch processing
    job_id: Option<String>,
    /// Optional input index for batch processing
    input_index: Option<usize>,
    progress_listener: Option<ProgressListener>,
}

impl ProgressEmitter {
    /// Progress goes to the owning engine reducer through its listener.
    pub(crate) fn with_context(context: EmitContext) -> Self {
        Self {
            operation_kind: context.operation_kind,
            job_id: context.job_id,
            input_index: context.input_index,
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

    fn terminal_event(&self, stage: EventStage, message: &str) -> ProgressEvent {
        ProgressEvent {
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
    #[cfg(test)]
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
        let percentage = if percentage.is_finite() {
            percentage.clamp(0.0, 100.0)
        } else {
            0.0
        };
        let event = ProgressEvent {
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

    #[test]
    fn emitted_progress_normalizes_non_finite_percentage() {
        let captured = Arc::new(std::sync::Mutex::new(None));
        let listener_capture = Arc::clone(&captured);
        let listener: ProgressListener = Arc::new(move |event| {
            *listener_capture.lock().expect("capture progress event") = Some(event.clone());
        });
        let emitter = ProgressEmitter::with_context(EmitContext {
            operation_kind: OperationKind::ProcessingBatch,
            job_id: None,
            input_index: None,
        })
        .with_progress_listener(Some(listener));

        emitter.emit_custom(
            ProcessingStage::Converting,
            f32::NAN,
            "Encoding",
            None,
            None,
        );

        let event = captured
            .lock()
            .expect("read progress event")
            .clone()
            .expect("progress event was emitted");
        assert_eq!(event.percentage, 0.0);
        assert!(event.percentage.is_finite());
    }
}
