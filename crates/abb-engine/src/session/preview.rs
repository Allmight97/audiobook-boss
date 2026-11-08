use super::submission::{Draft, SubmissionStatus};
use crate::processing::{ProcessResultStatus, ProgressEvent};
use crate::work_runtime::{
    new_processing_snapshot, now_ms, OperationId, OperationSnapshot, WorkRuntimeState,
};
use serde::Serialize;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSnapshot {
    pub operation: OperationSnapshot,
    /// A successful single preview may be claimed for opening once, across hosts.
    pub open_ready: bool,
    pub cover: Option<String>,
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

/// One preview run. `cancel` stops every title; each title has its own child
/// token, so one title can stop alone.
#[derive(Default)]
enum Run {
    #[default]
    Accepted,
    Running {
        cancel: CancellationToken,
        titles: Vec<CancellationToken>,
    },
    Finished {
        started: bool,
        open: Option<String>,
    },
}

impl Run {
    fn offers_artwork(&self) -> bool {
        match self {
            Self::Accepted => false,
            Self::Running { .. } => true,
            Self::Finished { started, .. } => *started,
        }
    }

    fn open_path(&mut self) -> Option<&mut Option<String>> {
        match self {
            Self::Finished { open, .. } => Some(open),
            Self::Accepted | Self::Running { .. } => None,
        }
    }
}

#[derive(Default)]
pub(crate) struct Preview {
    state: WorkRuntimeState,
    id: Option<OperationId>,
    run: Run,
    pub(crate) artwork: PreviewArtwork,
}

impl Preview {
    pub(crate) fn begin(&mut self, draft: &Draft) {
        let Some(id) = draft.preview_id() else { return };
        self.state = WorkRuntimeState::default();
        self.id = Some(id.clone());
        self.run = Run::Accepted;
        self.artwork = PreviewArtwork::default();
        self.state.insert_operation(new_processing_snapshot(
            id.clone(),
            0,
            draft.title.clone(),
            &draft.payload.input_files,
            draft.payload.input_ids.as_deref(),
        ));
    }

    pub(crate) fn end_finished(&mut self) {
        let finished = self.id.as_ref().is_some_and(|id| {
            self.state
                .operation(id.as_str())
                .is_some_and(OperationSnapshot::is_finished)
        });
        if finished {
            *self = Self::default();
        }
    }

    pub(crate) fn snapshot(&self) -> Option<PreviewSnapshot> {
        let id = self.id.as_ref()?;
        let operation = self.state.operation(id.as_str())?.clone();
        Some(PreviewSnapshot {
            operation,
            open_ready: matches!(&self.run, Run::Finished { open: Some(_), .. }),
            cover: self
                .run
                .offers_artwork()
                .then(|| super::cover_address::preview(id.as_str())),
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

    /// Opens the run and returns one token per title; a cancel asked for
    /// before now stops its tokens at once.
    pub(crate) fn start(&mut self, id: &OperationId) -> Vec<CancellationToken> {
        if !self.matches(id.as_str()) {
            return Vec::new();
        }
        self.state
            .mark_running(id, now_ms())
            .expect("registered preview");
        let operation = self
            .state
            .operation(id.as_str())
            .expect("registered preview");
        let cancel = CancellationToken::new();
        let titles: Vec<CancellationToken> = operation
            .children
            .iter()
            .map(|_| cancel.child_token())
            .collect();
        if operation.cancel_requested {
            cancel.cancel();
        }
        for child in operation
            .children
            .iter()
            .filter(|child| child.cancel_requested)
        {
            if let Some(token) = child.input_index.and_then(|index| titles.get(index)) {
                token.cancel();
            }
        }
        self.run = Run::Running {
            cancel,
            titles: titles.clone(),
        };
        titles
    }

    pub(crate) fn progress(&mut self, id: &OperationId, event: &ProgressEvent) {
        if self.matches(id.as_str()) {
            self.state
                .apply_progress_event(id, event, now_ms())
                .expect("registered preview");
        }
    }

    pub(crate) fn cancel(&mut self, id: &str, child: Option<&str>) -> crate::Result<()> {
        if !self.matches(id) {
            return Ok(());
        }
        let id = self.id.as_ref().expect("matching preview identity");
        if let Some(open) = self.run.open_path() {
            *open = None;
        }
        let title = match child {
            Some(child) => self.state.request_child_cancel(id, child)?.1,
            None => {
                self.state.request_cancel(id, now_ms())?;
                None
            }
        };
        if let Run::Running { cancel, titles } = &self.run {
            match (child, title) {
                (None, _) => cancel.cancel(),
                (Some(_), Some(index)) => {
                    if let Some(token) = titles.get(index) {
                        token.cancel();
                    }
                }
                (Some(_), None) => {}
            }
        }
        Ok(())
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
                self.end_run(output_to_open(result, cancelled));
            }
            SubmissionStatus::Cancelled => {
                self.state
                    .cancel(id, "Preview cancelled.".into(), now_ms())
                    .expect("registered preview");
                self.end_run(None);
            }
            SubmissionStatus::Failed { error } => {
                self.state
                    .fail(id, error.message.clone(), now_ms())
                    .expect("registered preview");
                self.end_run(None);
            }
            SubmissionStatus::Blocked { message } => {
                self.state
                    .fail(id, message.clone(), now_ms())
                    .expect("registered preview");
                self.end_run(None);
            }
            SubmissionStatus::Refused { .. } => {
                self.state
                    .fail(id, "Preview could not start.".into(), now_ms())
                    .expect("registered preview");
                self.end_run(None);
            }
            _ => {}
        }
    }

    fn end_run(&mut self, open: Option<String>) {
        self.run = Run::Finished {
            started: self.run.offers_artwork(),
            open,
        };
    }

    pub(crate) fn take_output(&mut self, id: &str) -> Option<String> {
        if !self.matches(id) {
            return None;
        }
        self.run.open_path().and_then(Option::take)
    }
}

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
#[path = "preview_tests.rs"]
mod tests;
