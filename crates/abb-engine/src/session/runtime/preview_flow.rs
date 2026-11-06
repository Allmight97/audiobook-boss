use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError};

use super::submission_flow::{closing, failed};
use super::Session;
use crate::errors::AppError;
use crate::processing::run::{process_inspected_with_options, ProcessingRunOptions};
use crate::session::intent::SessionOutcome;
use crate::session::submission::{Draft, SubmissionStatus};

impl Session {
    pub(super) async fn run_preview(
        &self,
        draft: Draft,
        inspected: crate::processing::plan::InspectedProcessingPlan,
    ) -> SessionOutcome {
        let deps = &self.inner.deps;
        let _active_run = deps.jobs.hold_run();
        let cancels: Vec<Arc<AtomicBool>> = (0..draft.payload.input_files.len())
            .map(|_| Arc::default())
            .collect();
        let id = draft.operation_id.clone();
        *self.preview_cancels() = cancels.clone();
        if let Some(preview) = self.lock().preview.snapshot() {
            for child in &preview.operation.children {
                if child.cancel_requested {
                    if let Some(flag) = child.input_index.and_then(|index| cancels.get(index)) {
                        flag.store(true, Ordering::SeqCst);
                    }
                }
            }
        }
        let artwork = crate::session::preview::PreviewArtwork::from_plan(&inspected);
        if self
            .inner
            .deps
            .tasks
            .admit(|| {
                self.transition(|state| {
                    state.preview.artwork = artwork;
                    state.start_preview(&id);
                });
            })
            .is_err()
        {
            return self.end_submission(&draft, closing());
        }
        self.publish();
        let result = process_inspected_with_options(
            &_active_run,
            deps.host.clone(),
            deps.jobs.clone(),
            deps.workspace_root.clone(),
            draft.payload.clone(),
            inspected,
            ProcessingRunOptions {
                operation_id: Some(id.to_string()),
                title_cancels: cancels,
                progress_listener: Some(Arc::new({
                    let session = self.clone();
                    let id = id.clone();
                    move |event| {
                        session.transition(|state| state.preview.progress(&id, event));
                        session.publish();
                    }
                })),
                ..ProcessingRunOptions::default()
            },
        )
        .await;
        self.preview_cancels().clear();
        let status = match result {
            Ok(result) => SubmissionStatus::PreviewFinished { result },
            Err(error) => failed(&error),
        };
        self.end_submission(&draft, status)
    }

    pub(super) fn preview_cancels(&self) -> std::sync::MutexGuard<'_, Vec<Arc<AtomicBool>>> {
        self.inner
            .preview_cancels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn cancel_preview_run(&self, run_id: &str, child: Option<&str>) -> SessionOutcome {
        let flags = self.preview_cancels();
        let indexes = self.transition(|state| {
            let indexes = state.preview.cancel(run_id, child)?;
            if child.is_none() && state.preview.matches(run_id) {
                state.cancel_preview_review();
            }
            Ok::<_, AppError>(indexes)
        });
        match indexes {
            Ok(indexes) => {
                for index in indexes {
                    if let Some(flag) = flags.get(index) {
                        flag.store(true, Ordering::SeqCst);
                    }
                }
                self.sources_released();
                SessionOutcome::Applied
            }
            Err(error) => SessionOutcome::Rejected {
                error: (&error).into(),
            },
        }
    }

    pub(crate) fn cancel_preview(&self) {
        let id = self
            .lock()
            .preview
            .snapshot()
            .map(|preview| preview.operation.operation_id);
        if let Some(id) = id {
            self.cancel_preview_run(id.as_str(), None);
        }
    }
}
