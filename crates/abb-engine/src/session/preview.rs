//! Preview identity and lifecycle, using the same progress reducer as exports.

use super::submission::{Draft, SubmissionStatus};
use crate::processing::{ProcessResultStatus, ProgressEvent};
use crate::work_runtime::{
    new_processing_snapshot, now_ms, OperationId, OperationSnapshot, WorkRuntimeState,
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSnapshot {
    pub operation: OperationSnapshot,
    /// A successful single preview may be claimed for opening once, across hosts.
    pub open_ready: bool,
    pub artwork_ready: bool,
}

#[derive(Clone, Default)]
pub(crate) enum PreviewArtwork {
    #[default]
    None,
    Bytes(Vec<u8>),
    Source(PathBuf),
}

impl PreviewArtwork {
    pub(crate) fn from_plan(inspected: &crate::processing::plan::InspectedProcessingPlan) -> Self {
        let Some(job) = inspected.plan.jobs.first() else {
            return Self::None;
        };
        if let Some(bytes) = job
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.cover_art.clone())
        {
            Self::Bytes(bytes)
        } else if job.cover_art_passthrough == crate::metadata::CoverArtPassthroughPolicy::Preserve
        {
            Self::Source(job.input_path.clone())
        } else {
            Self::None
        }
    }
}

#[derive(Default)]
pub(crate) struct Preview {
    state: WorkRuntimeState,
    id: Option<OperationId>,
    open_path: Option<String>,
    pub(crate) artwork: PreviewArtwork,
    artwork_ready: bool,
}

impl Preview {
    pub(crate) fn begin(&mut self, draft: &Draft) {
        let Some(id) = draft.preview_id() else { return };
        self.state = WorkRuntimeState::default();
        self.id = Some(id.clone());
        self.open_path = None;
        self.artwork = PreviewArtwork::default();
        self.artwork_ready = false;
        self.state.insert_operation(new_processing_snapshot(
            id.clone(),
            0,
            draft.title.clone(),
            &draft.payload.input_files,
            draft.payload.input_ids.as_deref(),
            now_ms(),
        ));
    }

    pub(crate) fn snapshot(&self) -> Option<PreviewSnapshot> {
        let operation = self.state.operation(self.id.as_ref()?.as_str())?.clone();
        Some(PreviewSnapshot {
            operation,
            open_ready: self.open_path.is_some(),
            artwork_ready: self.artwork_ready,
        })
    }

    pub(crate) fn matches(&self, id: &str) -> bool {
        self.id
            .as_ref()
            .is_some_and(|current| current.as_str() == id)
    }

    pub(crate) fn cancelled(&self, id: &OperationId) -> bool {
        self.matches(id.as_str())
            && self
                .state
                .operation(id.as_str())
                .is_some_and(|op| op.cancel_requested)
    }

    pub(crate) fn start(&mut self, id: &OperationId) {
        if self.matches(id.as_str()) {
            self.artwork_ready = true;
            self.state
                .mark_running(id, now_ms())
                .expect("registered preview");
        }
    }

    pub(crate) fn progress(&mut self, id: &OperationId, event: &ProgressEvent) {
        if self.matches(id.as_str()) {
            self.state
                .apply_progress_event(id, event, now_ms())
                .expect("registered preview");
        }
    }

    /// Returns the title indexes whose actual cancellation flags must be set.
    pub(crate) fn cancel(&mut self, id: &str, child: Option<&str>) -> crate::Result<Vec<usize>> {
        if !self.matches(id) {
            return Ok(Vec::new());
        }
        let id = self.id.as_ref().expect("matching preview identity");
        self.open_path = None;
        match child {
            Some(child) => {
                let (_, index) = self.state.request_child_cancel(id, child, now_ms())?;
                Ok(index.into_iter().collect())
            }
            None => {
                let operation = self.state.request_cancel(id, now_ms())?;
                Ok(operation
                    .children
                    .iter()
                    .filter_map(|child| child.input_index)
                    .collect())
            }
        }
    }

    pub(crate) fn finish(&mut self, draft: &Draft, status: &SubmissionStatus) {
        let Some(id) = draft.preview_id() else { return };
        if !self.matches(id.as_str()) {
            return;
        }
        match status {
            SubmissionStatus::PreviewFinished { result } => {
                let operation = self
                    .state
                    .complete_from_process_result(id, result, now_ms())
                    .expect("registered preview");
                let cancelled = operation.cancel_requested
                    || operation
                        .children
                        .iter()
                        .any(|child| child.cancel_requested);
                self.open_path = output_to_open(result, cancelled);
            }
            SubmissionStatus::Cancelled => {
                self.state
                    .cancel(id, "Preview cancelled.".into(), now_ms())
                    .expect("registered preview");
            }
            SubmissionStatus::Failed { error }
                if error.category == crate::errors::AppErrorCategory::Cancellation =>
            {
                self.state
                    .cancel(id, error.message.clone(), now_ms())
                    .expect("registered preview");
            }
            SubmissionStatus::Failed { error } => {
                self.state
                    .fail(id, error.message.clone(), now_ms())
                    .expect("registered preview");
            }
            SubmissionStatus::Blocked { message } => {
                self.state
                    .fail(id, message.clone(), now_ms())
                    .expect("registered preview");
            }
            SubmissionStatus::Refused { .. } => {
                self.state
                    .fail(id, "Preview could not start.".into(), now_ms())
                    .expect("registered preview");
            }
            _ => {}
        }
    }

    pub(crate) fn take_output(&mut self, id: &str) -> Option<String> {
        self.matches(id).then(|| self.open_path.take()).flatten()
    }
}

/// The preview that opens by itself: the one output a run nobody cancelled
/// produced. Several outputs, or none, open nothing.
fn output_to_open(
    result: &crate::processing::ProcessCommandResult,
    cancelled: bool,
) -> Option<String> {
    let mut paths = result
        .results
        .iter()
        .filter(|entry| entry.status == ProcessResultStatus::Success)
        .filter_map(|entry| entry.output_path.clone());
    match (cancelled, paths.next(), paths.next()) {
        (false, Some(path), None) => Some(path),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::output_to_open;
    use crate::processing::{ProcessCommandResult, ProcessResultEntry, ProcessResultStatus};

    fn result(entries: &[(ProcessResultStatus, &str)]) -> ProcessCommandResult {
        ProcessCommandResult::new(
            entries
                .iter()
                .enumerate()
                .map(|(input_index, (status, path))| ProcessResultEntry {
                    input_index,
                    status: *status,
                    message: String::new(),
                    error: None,
                    output_path: (!path.is_empty()).then(|| path.to_string()),
                    preview_actual_seconds: None,
                    supplemental_warning: None,
                    job_id: None,
                })
                .collect(),
        )
    }

    #[test]
    fn only_the_single_output_of_an_uncancelled_preview_opens() {
        use ProcessResultStatus::{Failed, Success};
        let one = result(&[(Success, "/p/a.m4b")]);
        assert_eq!(output_to_open(&one, false).as_deref(), Some("/p/a.m4b"));
        assert_eq!(
            output_to_open(&one, true),
            None,
            "a cancelled run opens nothing"
        );
        let two = result(&[(Success, "/p/a.m4b"), (Success, "/p/b.m4b")]);
        assert_eq!(
            output_to_open(&two, false),
            None,
            "several outputs open nothing"
        );
        let partly = result(&[(Success, "/p/a.m4b"), (Failed, "")]);
        assert_eq!(output_to_open(&partly, false).as_deref(), Some("/p/a.m4b"));
        assert_eq!(output_to_open(&result(&[(Failed, "")]), false), None);
    }
}
