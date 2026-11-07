use std::sync::Arc;

use super::{blocking, Rest, Session};
use crate::errors::{AppError, AppErrorEnvelope};
use crate::output_artifact::CollisionPolicy;
use crate::processing::run::inspect_processing_plan;
use crate::session::exports::ExportLink;
use crate::session::intent::SessionOutcome;
use crate::session::state::SessionState;
use crate::session::submission::{
    plan_verdict, Draft, PlanVerdict, SubmissionStatus, SubmitRefusal,
};
use crate::work_runtime::SubmitProcessingOperationRequest;

impl Session {
    pub(super) fn begin_submission(&self, preview: bool) -> Rest {
        let draft = self.transition(|state| state.begin_submission(preview));
        match draft {
            Some(draft) => Rest::Submit {
                draft: Box::new(self.capture_decoder(draft)),
            },
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    fn capture_decoder(&self, mut draft: Draft) -> Draft {
        draft.payload.aac_decoder = Some(self.inner.deps.settings.aac_decoder());
        draft
    }

    pub(super) async fn submit(&self, draft: Draft) -> SessionOutcome {
        self.publish();
        let checking = draft.clone();
        let plan = blocking(move || {
            inspect_processing_plan(
                &checking.payload,
                checking.metadata.as_ref(),
                checking.preview,
            )
        })
        .await;
        let plan = match plan {
            Ok(plan) => plan,
            Err(error) => return self.end_submission(&draft, failed(&error)),
        };
        if draft
            .preview_id()
            .is_some_and(|id| self.lock().preview.cancelled(id))
        {
            return self.end_submission(&draft, SubmissionStatus::Cancelled);
        }
        let public = plan.plan.to_public();
        let collided: Vec<_> = public
            .outputs
            .iter()
            .filter(|output| output.collision.is_some())
            .cloned()
            .collect();
        let unreviewed = draft.reviewed.as_ref().is_some_and(|reviewed| {
            crate::session::submission::collisions(&collided)
                .iter()
                .any(|collision| !reviewed.contains(collision))
        });
        match plan_verdict(&public) {
            PlanVerdict::Blocked(message) => {
                self.end_submission(&draft, SubmissionStatus::Blocked { message })
            }
            PlanVerdict::Review(outputs) if draft.payload.collision_policy.is_none() => {
                self.hold_for_review(draft, outputs)
            }
            _ if unreviewed => self.hold_for_review(draft, collided),
            PlanVerdict::Review(_) | PlanVerdict::Proceed => {
                self.accept(
                    draft.approved(public.collision_policy, public.plan_signature),
                    plan,
                )
                .await
            }
        }
    }

    fn hold_for_review(
        &self,
        draft: Draft,
        outputs: Vec<crate::output_artifact::PlannedOutput>,
    ) -> SessionOutcome {
        let closing_draft = draft.clone();
        match self
            .inner
            .deps
            .tasks
            .admit(|| self.transition(|state| state.await_review(draft, outputs)))
        {
            Ok(()) => SessionOutcome::Applied,
            Err(_) => {
                self.transition(|state| state.finish_submission(&closing_draft, closing()));
                self.sources_released();
                SessionOutcome::Applied
            }
        }
    }

    async fn accept(
        &self,
        draft: Draft,
        inspected: crate::processing::plan::InspectedProcessingPlan,
    ) -> SessionOutcome {
        let deps = &self.inner.deps;
        if draft.preview {
            return self.run_preview(draft, inspected).await;
        }
        let submitted = deps
            .work
            .submit_processing_operation(
                deps.host.clone(),
                deps.jobs.clone(),
                deps.workspace_root.clone(),
                SubmitProcessingOperationRequest {
                    operation_id: draft.operation_id.clone(),
                    payload: draft.payload.clone(),
                    metadata: draft.metadata.clone(),
                    title: draft.title.clone(),
                },
                Some(Box::new({
                    let session = self.clone();
                    move |snapshot| session.export_finished(&snapshot)
                })),
                inspected,
            )
            .await;
        let status = match submitted {
            Ok(accepted) => {
                self.transition(|state| {
                    state.link_exports(&draft, &accepted.operation_id, &accepted.titles);
                });
                let linked: Vec<String> = draft
                    .payload
                    .input_ids
                    .iter()
                    .flatten()
                    .flatten()
                    .cloned()
                    .collect();
                self.catch_up_outputs(&linked).await;
                SubmissionStatus::Submitted {
                    operation_id: accepted.operation_id,
                    title: draft.title.clone(),
                    left_out: draft.left_out,
                }
            }
            Err(AppError::Closing) => closing(),
            Err(error) => failed(&error),
        };
        self.end_submission(&draft, status)
    }

    /// Ends a submission; its sources stay reserved until WorkRuntime has
    /// registered them.
    pub(super) fn end_submission(&self, draft: &Draft, status: SubmissionStatus) -> SessionOutcome {
        self.transition(|state| state.finish_submission(draft, status));
        self.sources_released();
        SessionOutcome::Applied
    }

    pub(crate) fn cancel_review(&self) {
        self.transition(SessionState::cancel_review);
        self.sources_released();
    }

    pub(super) fn begin_collision_policy(&self, review_id: u64, policy: CollisionPolicy) -> Rest {
        match self.transition(|state| state.take_review(review_id)) {
            Some(draft) => Rest::Reviewed {
                draft: Box::new(draft),
                policy,
            },
            None => Rest::Done(SessionOutcome::Superseded),
        }
    }

    pub(super) async fn submit_reviewed(
        &self,
        mut draft: Draft,
        policy: CollisionPolicy,
    ) -> SessionOutcome {
        draft.payload.collision_policy = Some(policy);
        self.submit(draft).await
    }

    pub(super) fn cancel_collision_review(&self, review_id: u64) -> Rest {
        let cancelled = self.transition(|state| state.cancel_review_named(review_id));
        if cancelled {
            self.sources_released();
        }
        Rest::Done(if cancelled {
            SessionOutcome::Applied
        } else {
            SessionOutcome::Superseded
        })
    }

    pub(super) fn begin_keep_location(&self, title_id: String, revision: u64) -> Rest {
        match self.transition(|state| state.begin_keep_location(&title_id, revision)) {
            Some(link) => Rest::KeepLocation {
                title_id,
                revision,
                link: Box::new(link),
            },
            None => Rest::Done(SessionOutcome::Superseded),
        }
    }

    pub(super) async fn keep_location_finished(
        &self,
        title_id: String,
        revision: u64,
        link: ExportLink,
    ) -> SessionOutcome {
        let outcome = self.keep_location(title_id, revision, link).await;
        self.transition(SessionState::finish_keep_location);
        outcome
    }

    pub(super) fn begin_restart(&self, title_id: &str, revision: u64) -> Rest {
        let started = self.transition(|state| state.begin_restart(title_id, revision));
        match started {
            Some((draft, link)) => Rest::Restart {
                draft: Box::new(self.capture_decoder(draft)),
                link: Box::new(link),
            },
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    pub(super) async fn restart(&self, draft: Draft, link: ExportLink) -> SessionOutcome {
        self.publish();
        let deps = &self.inner.deps;
        let published = deps
            .work
            .stop_title(&deps.host, &link.operation_id, link.index, &link.title)
            .await;
        if !published {
            return self.submit(draft).await;
        }
        let edits = self.transition(|state| {
            let tags = &state.tags;
            state
                .exports
                .edits(|path| tags.pending(path).map(|pending| pending.patch.clone()))
                .into_iter()
                .filter(|edit| edit.title_id == draft_title_id(&draft))
                .collect()
        });
        let outputs = self.update_outputs(edits).await;
        self.end_submission(&draft, SubmissionStatus::FinishedBeforeRestart { outputs })
    }

    async fn keep_location(
        &self,
        title_id: String,
        revision: u64,
        link: ExportLink,
    ) -> SessionOutcome {
        let published = match link.title.keep_location(revision) {
            Ok(published) => published,
            Err(error) => {
                return SessionOutcome::Rejected {
                    error: AppErrorEnvelope::from(&error),
                }
            }
        };
        if published {
            let title = Arc::clone(&link.title);
            if let Err(error) = blocking(move || {
                title.apply_published();
                Ok(())
            })
            .await
            {
                return SessionOutcome::Rejected {
                    error: AppErrorEnvelope::from(&error),
                };
            }
        }
        self.transition(|state| state.kept_location(&title_id, revision));
        match link.title.update_state().map(|update| update.status) {
            Some(crate::processing::OutputUpdateStatus::Failed { message }) => {
                SessionOutcome::Rejected {
                    error: AppErrorEnvelope::from(&AppError::General(message)),
                }
            }
            _ => SessionOutcome::Applied,
        }
    }

    pub(crate) fn export_in_preparation(&self) -> Option<crate::work_runtime::OperationId> {
        self.lock().export_in_preparation().cloned()
    }
}

fn draft_title_id(draft: &Draft) -> String {
    draft
        .payload
        .input_ids
        .as_ref()
        .and_then(|ids| ids.first().cloned().flatten())
        .unwrap_or_default()
}

pub(super) fn closing() -> SubmissionStatus {
    SubmissionStatus::Refused {
        reason: SubmitRefusal::Closing,
    }
}

pub(super) fn failed(error: &AppError) -> SubmissionStatus {
    match error {
        AppError::Cancellation(_) => SubmissionStatus::Cancelled,
        error => SubmissionStatus::Failed {
            error: AppErrorEnvelope::from(error),
        },
    }
}
