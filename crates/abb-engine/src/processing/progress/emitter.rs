use super::{
    EventStage, ProgressEvent, PROGRESS_ANALYZING_START, PROGRESS_COMPLETE,
    PROGRESS_CONVERTING_START, PROGRESS_FINALIZING, PROGRESS_METADATA_START,
};
use crate::processing::OperationKind;
use crate::processing::ProcessingStage;
use std::sync::Arc;

type ProgressListener = Arc<dyn Fn(&ProgressEvent) + Send + Sync>;

#[derive(Clone)]
pub struct EmitContext {
    pub operation_kind: OperationKind,
    pub job_id: Option<String>,
    pub input_index: Option<usize>,
}

pub struct ProgressEmitter {
    operation_kind: OperationKind,
    job_id: Option<String>,
    input_index: Option<usize>,
    progress_listener: Option<ProgressListener>,
}

impl ProgressEmitter {
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

    pub fn emit_analyzing_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Analyzing,
            PROGRESS_ANALYZING_START,
            message,
            None,
            None,
        );
    }

    pub fn emit_converting_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Converting,
            PROGRESS_CONVERTING_START,
            message,
            None,
            None,
        );
    }

    pub fn emit_converting_progress(
        &self,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        self.emit_event(
            ProcessingStage::Converting,
            percentage,
            message,
            current_file,
            eta_seconds,
        );
    }

    pub fn emit_metadata_start(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_METADATA_START,
            message,
            None,
            None,
        );
    }

    pub fn emit_finalizing(&self, message: &str) {
        self.emit_event(
            ProcessingStage::WritingMetadata,
            PROGRESS_FINALIZING,
            message,
            None,
            None,
        );
    }

    pub fn emit_complete(&self, message: &str) {
        self.emit_event(
            ProcessingStage::Completed,
            PROGRESS_COMPLETE,
            message,
            None,
            None,
        );
    }

    pub fn emit_terminal_failed(&self, message: &str) {
        self.emit_terminal_event(EventStage::Failed, message);
    }

    pub fn emit_terminal_cancelled(&self, message: &str) {
        self.emit_terminal_event(EventStage::Cancelled, message);
    }

    pub fn emit_terminal_skipped(&self, message: &str) {
        self.emit_terminal_event(EventStage::Skipped, message);
    }

    /// Emits cancelled event (special-case stage not represented in ProcessingStage enum)
    pub fn emit_cancelled(&self, message: &str) {
        self.emit_terminal_cancelled(message);
    }
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
#[path = "emitter_tests.rs"]
mod tests;
