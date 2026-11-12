use std::sync::Arc;

use super::submission_flow::{closing, failed};
use super::Session;
use crate::errors::AppError;
use crate::processing::run::{process_inspected_with_options, ProcessingRun, RunMode};
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
        let id = draft.operation_id.clone();
        let artwork = crate::session::preview::PreviewArtwork::from_plan(&inspected);
        let Ok(cancels) = self.inner.deps.tasks.admit(|| {
            self.transition(|state| {
                state.preview.artwork = artwork;
                state.start_preview(&id)
            })
        }) else {
            return self.end_submission(&draft, closing());
        };
        self.publish();
        let result = process_inspected_with_options(
            &_active_run,
            deps.host.clone(),
            deps.jobs.clone(),
            deps.workspace_root.clone(),
            draft.payload.clone(),
            inspected,
            ProcessingRun {
                operation_id: id.to_string(),
                mode: RunMode::Preview(cancels),
                listener: Arc::new({
                    let session = self.clone();
                    let id = id.clone();
                    move |event| {
                        session.transition(|state| state.preview.progress(&id, event));
                        session.publish();
                    }
                }),
            },
        )
        .await;
        let status = match result {
            Ok(result) => SubmissionStatus::PreviewFinished { result },
            Err(error) => failed(&error),
        };
        self.end_submission(&draft, status)
    }

    pub(super) fn cancel_preview_run(&self, run_id: &str, child: Option<&str>) -> SessionOutcome {
        let cancelled = self.transition(|state| {
            state.preview.cancel(run_id, child)?;
            if child.is_none() && state.preview.matches(run_id) {
                state.cancel_preview_review();
            }
            Ok::<_, AppError>(())
        });
        match cancelled {
            Ok(()) => {
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
