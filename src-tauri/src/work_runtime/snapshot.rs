use super::types::{
    ChildJobSnapshot, OperationId, OperationSnapshot, ProgressSnapshot, ResourceLane,
    WorkOperationStatus,
};
use crate::processing::OperationKind;
use std::path::Path;

pub(crate) fn new_processing_snapshot(
    operation_id: OperationId,
    sequence: u64,
    kind: OperationKind,
    title: String,
    input_files: &[String],
    input_ids: Option<&[Option<String>]>,
    now_ms: i64,
) -> OperationSnapshot {
    let source_input_ids = input_ids
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.clone())
        .collect();
    let total_items = match kind {
        OperationKind::ProcessingMerge => Some(1),
        _ => Some(input_files.len()),
    };
    let children = match kind {
        OperationKind::ProcessingMerge => vec![new_child(
            &operation_id,
            "merge-output".to_string(),
            merge_label(input_files),
            input_files.first().cloned(),
            None,
            None,
            total_items,
        )],
        _ => input_files
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
                    total_items,
                )
            })
            .collect(),
    };

    OperationSnapshot {
        operation_id,
        sequence,
        kind,
        status: WorkOperationStatus::Accepted,
        title,
        created_at_ms: now_ms,
        started_at_ms: None,
        finished_at_ms: None,
        cancellable: true,
        cancel_requested: false,
        lanes: vec![
            ResourceLane::Analysis,
            ResourceLane::EncodeCpu,
            ResourceLane::OutputCommit,
        ],
        source_input_ids,
        progress: ProgressSnapshot::pending("Accepted for background processing.", total_items),
        children,
        terminal_summary: None,
        warnings: Vec::new(),
        errors: Vec::new(),
    }
}

fn new_child(
    operation_id: &OperationId,
    child_job_id: String,
    label: String,
    source_path: Option<String>,
    input_index: Option<usize>,
    input_id: Option<String>,
    total_items: Option<usize>,
) -> ChildJobSnapshot {
    ChildJobSnapshot {
        child_job_id,
        operation_id: operation_id.clone(),
        label,
        status: super::ChildJobStatus::Queued,
        lane: ResourceLane::EncodeCpu,
        progress: ProgressSnapshot::pending("Queued.", total_items),
        source_path,
        input_index,
        input_id,
        job_id: None,
        cancellable: false,
        cancel_requested: false,
        message: None,
    }
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn merge_label(input_files: &[String]) -> String {
    match input_files.first() {
        Some(first) if input_files.len() > 1 => {
            format!(
                "Merge: {} + {} more",
                basename(first),
                input_files.len() - 1
            )
        }
        Some(first) => format!("Merge: {}", basename(first)),
        None => "Merge output".to_string(),
    }
}
