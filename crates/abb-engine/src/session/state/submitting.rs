use std::path::PathBuf;
use std::sync::Arc;

use super::super::exports::{ExportLink, OutputEdit, OutputEdits, RestartStale};
use super::super::submission::{
    build_draft, collisions, title_label, CollisionReview, Draft, DraftInputs, PendingReview,
    SubmissionStatus, SubmitRefusal, SubmittedTitle,
};
use super::{SessionState, StageOutcome};
use crate::audio::AudioFile;
use crate::metadata::PatchOp;
use crate::output_artifact::PlannedOutput;
use crate::processing::title_output::UpdateReply;
use crate::processing::TitleOutput;
use crate::work_runtime::OperationId;

pub(super) struct Submitting {
    operation_id: OperationId,
    preview: bool,
}

impl SessionState {
    pub(crate) fn begin_submission(&mut self, preview: bool) -> Option<Draft> {
        self.begin_submission_of(preview, None)
    }

    fn begin_submission_of(&mut self, preview: bool, only: Option<&str>) -> Option<Draft> {
        match self.prepare_submission(preview, only) {
            Ok(draft) => {
                self.preview.begin(&draft);
                self.submitting = Some(Submitting {
                    operation_id: draft.operation_id.clone(),
                    preview: draft.preview,
                });
                self.reserved.extend(draft.sources.iter().cloned());
                self.working_set.set_order_locked(true);
                self.submission = Some(SubmissionStatus::Preparing {
                    preview: draft.preview,
                });
                Some(draft)
            }
            Err(reason) => {
                self.refuse_submission(reason);
                None
            }
        }
    }

    fn prepare_submission(
        &mut self,
        preview: bool,
        only: Option<&str>,
    ) -> Result<Draft, SubmitRefusal> {
        if self.submitting.is_some() {
            return Err(SubmitRefusal::Busy);
        }
        if self.writes.is_writing() {
            return Err(SubmitRefusal::SaveInProgress);
        }
        if !self.working_set.files().is_empty() {
            match self.stage_bound_form() {
                StageOutcome::Staged => {}
                StageOutcome::NoTarget => return Err(SubmitRefusal::NoTarget),
                StageOutcome::Invalid(message) => {
                    return Err(SubmitRefusal::DraftInvalid { message })
                }
            }
        }
        let required = self.working_set.audio_choice_required();
        let titles: Vec<SubmittedTitle<'_>> = self
            .working_set
            .files()
            .iter()
            .filter(|file| only.is_none_or(|title_id| file.input_id == title_id))
            .map(|file| SubmittedTitle {
                anchor: file,
                sources: self.working_set.sources_for(file),
                request: self.working_set.audio_request(&file.input_id).map_or_else(
                    || self.audio.request(),
                    |request| self.audio.title_view(request).request,
                ),
                choice_required: required.contains(&file.input_id),
                has_title: self.has_title(file),
            })
            .collect();
        let supplemental_assets = self.staged.assets_for(
            titles
                .iter()
                .filter(|title| title.anchor.is_valid)
                .flat_map(|title| title.sources)
                .map(|source| source.input_id.as_str()),
        );
        let draft = build_draft(
            &titles,
            DraftInputs {
                output_directory: self.output.naming_directory(),
                naming: self.output.naming(),
                supplemental_assets,
                preview,
            },
            |paths| {
                let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
                self.pending_intents(&paths)
                    .into_iter()
                    .map(|(path, patch)| (path.to_string_lossy().into_owned(), patch))
                    .collect()
            },
            title_label,
        )?;
        if draft
            .sources
            .iter()
            .any(|source| self.removing.contains(source))
        {
            return Err(SubmitRefusal::SourceRemoved);
        }
        Ok(draft)
    }

    fn has_title(&self, file: &AudioFile) -> bool {
        let pending = self
            .tags
            .pending(&file.path)
            .and_then(|pending| pending.patch.title.clone());
        let title = match pending {
            Some(PatchOp::Set(title)) => Some(title),
            Some(PatchOp::Clear) => None,
            None if self.tags.has_source_read(&file.path) => {
                self.tags.effective(&file.path).and_then(|tags| tags.title)
            }
            None => file.tag_title.clone(),
        };
        title.is_some_and(|title| !title.trim().is_empty())
    }

    pub(crate) fn link_exports(
        &mut self,
        draft: &Draft,
        operation_id: &OperationId,
        outputs: &[Arc<TitleOutput>],
    ) {
        let ids = draft.payload.input_ids.clone().unwrap_or_default();
        let titles = draft
            .payload
            .input_files
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let id = ids.get(index).cloned().flatten().unwrap_or_default();
                (id, PathBuf::from(path))
            });
        self.exports.link(operation_id, titles, outputs);
    }

    pub(crate) fn record_output_edits(
        &mut self,
        replies: Vec<(OutputEdit, Result<UpdateReply, String>)>,
    ) -> OutputEdits {
        let mut counts = OutputEdits::default();
        let directory = self.output.naming_directory();
        let naming = self.output.naming();
        for (edit, reply) in replies {
            if !self.exports.accepts_reply(&edit) {
                continue;
            }
            match reply {
                Ok(UpdateReply::Unchanged) => self.exports.withdraw(&edit.title_id),
                Ok(UpdateReply::Accepted { elsewhere, .. }) => {
                    self.exports.withdraw(&edit.title_id);
                    counts.updated += 1;
                    counts.elsewhere += usize::from(elsewhere);
                }
                Ok(UpdateReply::MovesOutput { from, to }) => {
                    let offered =
                        self.exports
                            .offer(&edit, &from, &to, directory.clone(), naming.clone());
                    counts.restart_offered += usize::from(offered);
                }
                Ok(UpdateReply::NoOutput) => self.exports.unlink(&edit.title_id),
                Err(error) => {
                    log::warn!("Output update refused: {error}");
                    counts.failed += 1;
                }
            }
        }
        counts
    }

    pub(crate) fn begin_restart(
        &mut self,
        title_id: &str,
        revision: u64,
    ) -> Option<(Draft, ExportLink)> {
        let directory = self.output.naming_directory();
        let naming = self.output.naming();
        let link = match self
            .exports
            .offered(title_id, revision, directory.as_ref(), &naming)
        {
            Ok(link) => link,
            Err(RestartStale::Stale) => {
                self.refuse_submission(SubmitRefusal::RestartStale);
                return None;
            }
        };
        self.exports.acknowledge_prompt(title_id, revision);
        let draft = self.begin_submission_of(false, Some(title_id))?;
        self.exports.consume_offer(title_id);
        Some((draft, link))
    }

    pub(crate) fn kept_location(&mut self, title_id: &str, revision: u64) {
        self.exports.decline(title_id, revision);
    }

    pub(crate) fn begin_keep_location(
        &mut self,
        title_id: &str,
        revision: u64,
    ) -> Option<ExportLink> {
        let link = self
            .exports
            .offered(
                title_id,
                revision,
                self.output.naming_directory().as_ref(),
                &self.output.naming(),
            )
            .ok()?;
        self.exports.acknowledge_prompt(title_id, revision);
        self.keeping_locations += 1;
        Some(link)
    }

    pub(crate) fn finish_keep_location(&mut self) {
        self.keeping_locations -= 1;
    }

    pub(crate) fn await_review(&mut self, mut draft: Draft, outputs: Vec<PlannedOutput>) {
        draft.payload.collision_policy = None;
        draft.reviewed = Some(collisions(&outputs));
        self.review_serial += 1;
        let view = CollisionReview {
            review_id: self.review_serial,
            outputs,
        };
        self.submission = Some(SubmissionStatus::ReviewRequired);
        self.pending_review = Some(PendingReview { view, draft });
    }

    pub(crate) fn refuse_submission(&mut self, reason: SubmitRefusal) {
        log::info!("submit_refused reason={}", reason.kind());
        self.submission = Some(SubmissionStatus::Refused { reason });
    }

    pub(crate) fn cancel_review(&mut self) {
        if let Some(review) = self.pending_review.take() {
            self.finish_submission(&review.draft, SubmissionStatus::Cancelled);
        }
    }

    pub(crate) fn cancel_review_named(&mut self, review_id: u64) -> bool {
        if !self.review_matches(review_id) {
            return false;
        }
        self.cancel_review();
        true
    }

    fn review_matches(&self, review_id: u64) -> bool {
        self.pending_review
            .as_ref()
            .is_some_and(|review| review.view.review_id == review_id)
    }

    pub(crate) fn cancel_preview_review(&mut self) {
        if self
            .pending_review
            .as_ref()
            .is_some_and(|review| review.draft.preview)
        {
            self.cancel_review();
        }
    }

    pub(crate) fn take_review(&mut self, review_id: u64) -> Option<Draft> {
        if !self.review_matches(review_id) {
            return None;
        }
        let draft = self.pending_review.take()?.draft;
        self.submission = Some(SubmissionStatus::Preparing {
            preview: draft.preview,
        });
        Some(draft)
    }

    pub(crate) fn start_preview(&mut self, id: &OperationId) {
        self.preview.start(id);
        self.submission = Some(SubmissionStatus::Previewing);
    }

    pub(crate) fn finish_submission(&mut self, draft: &Draft, status: SubmissionStatus) {
        self.preview.finish(draft, &status);
        for source in &draft.sources {
            if let Some(index) = self.reserved.iter().position(|reserved| reserved == source) {
                self.reserved.swap_remove(index);
            }
        }
        if self.reserved.is_empty() {
            self.working_set.set_order_locked(false);
        }
        self.submitting = None;
        self.submission = Some(status);
    }

    pub(crate) fn export_in_preparation(&self) -> Option<&OperationId> {
        self.submitting
            .as_ref()
            .filter(|submitting| !submitting.preview)
            .map(|submitting| &submitting.operation_id)
    }
}
