use abb_media_pipeline::processing::{OperationKind, ProcessPayload};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
pub struct OperationId(pub String);

impl OperationId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for OperationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OperationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum WorkOperationStatus {
    Accepted,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationPhase {
    Accepted,
    Active,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChildJobStatus {
    Queued { position: u32, of: u32 },
    Running,
    Completed,
    Skipped,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum WorkProgressStage {
    Pending,
    Analyzing,
    Converting,
    Writing,
    Cleaning,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub stage: WorkProgressStage,
    #[specta(type = specta_typescript::Number)]
    pub percentage: f32,
    pub message: String,
    pub eta_seconds: Option<f64>,
}

impl ProgressSnapshot {
    pub fn pending(message: impl Into<String>) -> Self {
        Self {
            stage: WorkProgressStage::Pending,
            percentage: 0.0,
            message: message.into(),
            eta_seconds: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChildJobSnapshot {
    pub child_job_id: String,
    pub operation_id: OperationId,
    pub label: String,
    pub status: ChildJobStatus,
    pub started_at_ms: Option<i64>,
    pub finished_at_ms: Option<i64>,
    pub progress: ProgressSnapshot,
    pub source_path: Option<String>,
    pub input_index: Option<usize>,
    pub input_id: Option<String>,
    #[serde(skip)]
    pub source_input_ids: Vec<String>,
    pub job_id: Option<String>,
    pub cancellable: bool,
    pub cancel_requested: bool,
    pub message: Option<String>,
    pub output_path: Option<String>,
    pub supplemental_warning: Option<String>,
    pub warnings: Vec<String>,
    pub output_update: Option<abb_media_pipeline::processing::OutputUpdate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationLogEntry {
    pub message: String,
    pub stage: Option<WorkProgressStage>,
    pub child_job_id: Option<String>,
}

pub const OPERATION_LOG_TAIL_CAP: usize = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChildCounts {
    pub queued: u32,
    pub running: u32,
    pub completed: u32,
    pub skipped: u32,
    pub cancelled: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationSnapshot {
    pub operation_id: OperationId,
    pub sequence: u64,
    pub revision: u64,
    pub kind: OperationKind,
    pub status: WorkOperationStatus,
    pub phase: OperationPhase,
    pub title: String,
    pub started_at_ms: Option<i64>,
    pub finished_at_ms: Option<i64>,
    pub cancellable: bool,
    pub cancel_requested: bool,
    pub progress: ProgressSnapshot,
    pub children: Vec<ChildJobSnapshot>,
    pub counts: ChildCounts,
    pub log_tail: Vec<OperationLogEntry>,
}

impl OperationSnapshot {
    pub(crate) fn set_status(&mut self, status: WorkOperationStatus) {
        self.status = status;
        self.phase = super::terminal::phase_of(status);
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.phase == OperationPhase::Finished
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkOperationsSnapshot {
    pub revision: u64,
    pub order: Vec<OperationId>,
    pub operations: Vec<OperationSnapshot>,
    pub concurrency_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkOperationsUpdate {
    pub revision: u64,
    pub order: Vec<OperationId>,
    pub changed: Option<OperationSnapshot>,
    pub concurrency_locked: bool,
}

#[derive(Debug, Clone)]
pub struct WorkSubmissionAccepted {
    pub operation_id: OperationId,
    pub snapshot: OperationSnapshot,
    pub(crate) titles: Vec<std::sync::Arc<abb_media_pipeline::processing::TitleOutput>>,
}

#[derive(Clone)]
pub struct SubmitProcessingOperationRequest {
    pub operation_id: OperationId,
    pub payload: ProcessPayload,
    pub metadata: Option<HashMap<String, abb_media_pipeline::metadata::MetadataIntentPatch>>,
    pub title: String,
}
