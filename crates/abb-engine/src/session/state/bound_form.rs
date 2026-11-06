use std::path::PathBuf;

use super::super::lookup::QueuedTitle;
use super::super::metadata_form::{FieldAction, MetadataField, MetadataForm};
use super::super::tag_cache::ReadTicket;
use super::{Cover, CoverNotice, MetadataStatus, SessionState};
use crate::errors::{AppError, AppErrorEnvelope};
use crate::metadata::{validate_metadata_intent_patch, AudiobookMetadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GateBlock {
    SaveInProgress,
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StageOutcome {
    Staged,
    NoTarget,
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoundTitle {
    pub(super) path: PathBuf,
    input_id: String,
    editable: bool,
}

impl SessionState {
    fn selected_titles(&self) -> Vec<BoundTitle> {
        self.working_set
            .selected_files()
            .into_iter()
            .map(|file| BoundTitle {
                path: file.path.clone(),
                input_id: file.input_id.clone(),
                editable: file.takes_edits(),
            })
            .collect()
    }

    pub(crate) fn stage_bound_form(&mut self) -> StageOutcome {
        let patch = self.form.compose_intent();
        if !patch.is_actionable() {
            return StageOutcome::Staged;
        }
        let targets: Vec<&BoundTitle> = self.bound.iter().filter(|title| title.editable).collect();
        if targets.is_empty() {
            return StageOutcome::NoTarget;
        }
        let validation = validate_metadata_intent_patch(&patch);
        if let Some(error) = validation.field_errors.first() {
            return StageOutcome::Invalid(error.message.clone());
        }
        for target in targets {
            self.tags.stage(&target.path, &validation.metadata_patch);
        }
        self.form.reset_dirty();
        StageOutcome::Staged
    }

    pub(crate) fn gate(&mut self) -> Result<(), GateBlock> {
        if self.writes.save_running() {
            return Err(GateBlock::SaveInProgress);
        }
        if self.bound.is_empty() || !self.form.has_dirty_fields() {
            return Ok(());
        }
        match self.stage_bound_form() {
            StageOutcome::Staged => {
                self.cover.custom = false;
                self.cover.removal_requested = false;
                Ok(())
            }
            StageOutcome::NoTarget => Ok(()),
            StageOutcome::Invalid(message) => {
                self.status = Some(MetadataStatus::DraftInvalid {
                    message: message.clone(),
                });
                Err(GateBlock::Invalid(message))
            }
        }
    }

    pub(crate) fn rebind(&mut self) -> Vec<ReadTicket> {
        if self.working_set.files().is_empty() {
            self.reset_metadata();
            return Vec::new();
        }
        self.tags.retain_paths(&self.working_set.source_paths());
        self.restore_failed_writes();

        let selected = self.selected_titles();
        let mut key: Vec<PathBuf> = selected.iter().map(|title| title.path.clone()).collect();
        key.sort();
        let mut tickets = Vec::new();
        if key != self.selection_key || selected != self.bound {
            self.binding += 1;
            self.selection_key = key;
            self.bound = selected;
            self.status = None;
            self.cover = Cover {
                displayed: self.cover.displayed.take(),
                image_revision: self.cover.image_revision,
                notice_serial: self.cover.notice_serial,
                ..Cover::default()
            };
            self.form = self.form_from_known_tags();
            let paths: Vec<PathBuf> = self
                .bound
                .iter()
                .filter(|title| title.editable)
                .map(|title| title.path.clone())
                .collect();
            tickets.extend(paths.iter().filter_map(|path| self.tags.begin_read(path)));
        }
        if let Some(cover) = self.cover_read() {
            if !tickets.iter().any(|ticket| ticket.path == cover.path) {
                tickets.push(cover);
            }
        }
        tickets
    }

    pub(super) fn form_from_known_tags(&self) -> MetadataForm {
        let known = |title: &BoundTitle| {
            title
                .editable
                .then(|| self.tags.effective(&title.path))
                .flatten()
                .unwrap_or_default()
        };
        match self.bound.as_slice() {
            [] => MetadataForm::single(&AudiobookMetadata::default()),
            [title] => MetadataForm::single(&known(title)),
            titles => {
                let metadata: Vec<AudiobookMetadata> = titles.iter().map(known).collect();
                MetadataForm::multi(&metadata, titles.len())
            }
        }
    }

    pub(crate) fn finish_reads(
        &mut self,
        binding: u64,
        reads: Vec<(ReadTicket, Result<AudiobookMetadata, AppError>)>,
    ) {
        let cover_path = self.cover_read_target();
        for (ticket, result) in reads {
            match result {
                Ok(metadata) => {
                    self.tags.complete_read(&ticket, metadata);
                }
                Err(error) => {
                    log::warn!("Failed to read metadata: {error}");
                    if binding == self.binding && cover_path.as_ref() == Some(&ticket.path) {
                        self.set_cover_notice(CoverNotice::LoadFailed {
                            error: AppErrorEnvelope::from(&error),
                        });
                    }
                }
            }
        }
        if binding == self.binding {
            let fresh = self.form_from_known_tags();
            self.form.rehydrate(fresh);
        }
    }

    pub(super) fn reset_metadata(&mut self) {
        self.epoch += 1;
        self.binding += 1;
        self.tags.clear();
        self.form = MetadataForm::single(&AudiobookMetadata::default());
        self.bound.clear();
        self.selection_key.clear();
        self.cover = Cover {
            displayed: self.cover.displayed.take(),
            image_revision: self.cover.image_revision,
            notice_serial: self.cover.notice_serial,
            ..Cover::default()
        };
        self.writes.end_save();
        self.status = None;
    }

    pub(crate) fn set_field(&mut self, binding: u64, field: MetadataField, value: String) -> bool {
        if binding != self.binding {
            return false;
        }
        self.form.set_value(field, value);
        self.status = None;
        true
    }

    pub(crate) fn set_field_action(
        &mut self,
        binding: u64,
        field: MetadataField,
        action: FieldAction,
    ) -> bool {
        if binding != self.binding {
            return false;
        }
        self.form.set_action(field, action);
        self.status = None;
        true
    }

    pub(crate) fn apply_lookup(
        &mut self,
        title: &QueuedTitle,
        metadata: &AudiobookMetadata,
        cover: Option<Vec<u8>>,
    ) -> bool {
        let is_title = |titles: &[BoundTitle]| matches!(titles, [only] if only.path == title.path && only.input_id == title.title_id);
        if self.writes.save_running()
            || !is_title(&self.bound)
            || !is_title(&self.selected_titles())
        {
            return false;
        }
        self.form.apply_lookup(metadata);
        if let Some(cover) = cover.filter(|cover| !cover.is_empty()) {
            self.apply_cover(cover);
        }
        true
    }
}
