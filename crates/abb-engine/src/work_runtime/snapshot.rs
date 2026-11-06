use super::types::{
    ChildJobSnapshot, OperationId, OperationSnapshot, ProgressSnapshot, WorkOperationStatus,
};
use crate::processing::OperationKind;
use std::path::Path;

pub(crate) fn new_processing_snapshot(
    operation_id: OperationId,
    sequence: u64,
    title: String,
    input_files: &[String],
    input_ids: Option<&[Option<String>]>,
) -> OperationSnapshot {
    let children = input_files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            new_child(
                &operation_id,
                format!("input-{index}"),
                basename(path),
                Some(path.clone()),
                Some(index),
                input_ids
                    .and_then(|ids| ids.get(index))
                    .and_then(|value| value.clone()),
            )
        })
        .collect();

    OperationSnapshot {
        operation_id,
        sequence,
        revision: 0,
        kind: OperationKind::ProcessingBatch,
        status: WorkOperationStatus::Accepted,
        title,
        started_at_ms: None,
        finished_at_ms: None,
        cancellable: true,
        cancel_requested: false,
        progress: ProgressSnapshot::pending("Accepted for background processing."),
        children,
        terminal_summary: None,
        log_tail: Vec::new(),
    }
}

/// Builds a metadata-save operation snapshot: one child per file, matched by
/// `input_index`.
pub(crate) fn new_metadata_save_snapshot(
    operation_id: OperationId,
    sequence: u64,
    title: String,
    input_files: &[String],
) -> OperationSnapshot {
    let children = input_files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            new_child(
                &operation_id,
                format!("metadata-{index}"),
                basename(path),
                Some(path.clone()),
                Some(index),
                None,
            )
        })
        .collect();

    OperationSnapshot {
        operation_id,
        sequence,
        revision: 0,
        kind: OperationKind::MetadataSave,
        status: WorkOperationStatus::Accepted,
        title,
        started_at_ms: None,
        finished_at_ms: None,
        cancellable: true,
        cancel_requested: false,
        progress: ProgressSnapshot::pending("Queued for metadata save."),
        children,
        terminal_summary: None,
        log_tail: Vec::new(),
    }
}

fn new_child(
    operation_id: &OperationId,
    child_job_id: String,
    label: String,
    source_path: Option<String>,
    input_index: Option<usize>,
    input_id: Option<String>,
) -> ChildJobSnapshot {
    ChildJobSnapshot {
        child_job_id,
        operation_id: operation_id.clone(),
        label,
        status: super::ChildJobStatus::Queued,
        started_at_ms: None,
        finished_at_ms: None,
        progress: ProgressSnapshot::pending("Queued."),
        source_path,
        input_index,
        source_input_ids: input_id.iter().cloned().collect(),
        input_id,
        job_id: None,
        cancellable: false,
        cancel_requested: false,
        message: None,
        output_path: None,
        supplemental_warning: None,
        warnings: Vec::new(),
        output_update: None,
    }
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string())
}
