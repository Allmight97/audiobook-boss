use crate::processing::{OperationKind, ProcessPayload};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const WORK_OPERATION_SNAPSHOT_EVENT_NAME: &str = "work-operation-snapshot";
pub const WORK_OPERATION_LIST_SNAPSHOT_EVENT_NAME: &str = "work-operation-list-snapshot";

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
pub enum ChildJobStatus {
    Queued,
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
    Downloading,
    Decrypting,
    Committing,
    Cleaning,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ResourceLane {
    EncodeCpu,
    NetworkDownload,
    HelperMaterializer,
    MetadataWrite,
    OutputCommit,
    Analysis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub stage: WorkProgressStage,
    pub percentage: f32,
    pub message: String,
    pub current_item_index: Option<usize>,
    pub total_items: Option<usize>,
    pub bytes_downloaded: Option<u64>,
    pub bytes_total: Option<u64>,
    pub eta_seconds: Option<f64>,
}

impl ProgressSnapshot {
    pub fn pending(message: impl Into<String>, total_items: Option<usize>) -> Self {
        Self {
            stage: WorkProgressStage::Pending,
            percentage: 0.0,
            message: message.into(),
            current_item_index: None,
            total_items,
            bytes_downloaded: None,
            bytes_total: None,
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
    pub lane: ResourceLane,
    pub progress: ProgressSnapshot,
    pub source_path: Option<String>,
    pub input_index: Option<usize>,
    pub input_id: Option<String>,
    pub job_id: Option<String>,
    pub cancellable: bool,
    pub cancel_requested: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationTerminalSummary {
    pub total: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub cancelled: usize,
    pub failed: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationSnapshot {
    pub operation_id: OperationId,
    pub sequence: u64,
    pub kind: OperationKind,
    pub status: WorkOperationStatus,
    pub title: String,
    pub created_at_ms: i64,
    pub started_at_ms: Option<i64>,
    pub finished_at_ms: Option<i64>,
    pub cancellable: bool,
    pub cancel_requested: bool,
    pub lanes: Vec<ResourceLane>,
    pub source_input_ids: Vec<String>,
    pub progress: ProgressSnapshot,
    pub children: Vec<ChildJobSnapshot>,
    pub terminal_summary: Option<OperationTerminalSummary>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationListSnapshot {
    pub operations: Vec<OperationSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkSubmissionAccepted {
    pub operation_id: OperationId,
    pub snapshot: OperationSnapshot,
}

#[derive(Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubmitProcessingOperationRequest {
    pub payload: ProcessPayload,
    pub metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    pub preview_seconds: Option<f64>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkOperationSnapshotEvent {
    pub snapshot: OperationSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkOperationListSnapshotEvent {
    pub operations: Vec<OperationSnapshot>,
}

impl tauri_specta::Event for WorkOperationSnapshotEvent {
    const NAME: &'static str = WORK_OPERATION_SNAPSHOT_EVENT_NAME;
}

impl tauri_specta::Event for WorkOperationListSnapshotEvent {
    const NAME: &'static str = WORK_OPERATION_LIST_SNAPSHOT_EVENT_NAME;
}
