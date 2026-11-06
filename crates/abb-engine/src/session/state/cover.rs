use std::path::PathBuf;

use super::{CoverNotice, SessionState};
use crate::audio::AudioFile;
use crate::errors::{AppError, AppErrorEnvelope};
use crate::metadata::{MetadataIntentPatch, PatchOp};
use crate::session::tag_cache::ReadTicket;

#[derive(Debug, Default)]
pub(super) struct Cover {
    pub(super) custom: bool,
    pub(super) removal_requested: bool,
    pub(super) loading: bool,
    pub(super) notice: Option<CoverNotice>,
    pub(super) notice_serial: u64,
    pub(super) displayed: Option<Vec<u8>>,
    pub(super) image_revision: u64,
    pub(super) request: u64,
}

impl SessionState {
    pub(super) fn cover_owner(&self) -> Option<PathBuf> {
        let mut valid = self
            .working_set
            .selected_files()
            .into_iter()
            .filter(|file| file.is_valid);
        match (valid.next(), valid.next()) {
            (Some(file), None) => Some(file.path.clone()),
            _ => None,
        }
    }

    pub(super) fn cover_display_path(&self) -> Option<PathBuf> {
        let valid: Vec<&AudioFile> = self
            .working_set
            .selected_files()
            .into_iter()
            .filter(|file| file.is_valid)
            .collect();
        match valid.as_slice() {
            [] => self
                .working_set
                .files()
                .iter()
                .find(|file| file.is_valid)
                .map(|file| file.path.clone()),
            [only] => Some(only.path.clone()),
            [first, rest @ ..] => {
                let cover = self.tags.effective_cover(&first.path);
                rest.iter()
                    .all(|file| self.tags.effective_cover(&file.path) == cover)
                    .then(|| first.path.clone())
            }
        }
    }

    pub(super) fn refresh_displayed_cover(&mut self) {
        let cover = self
            .cover_display_path()
            .and_then(|path| self.tags.effective_cover(&path));
        if cover != self.cover.displayed {
            self.cover.displayed = cover;
            self.cover.image_revision += 1;
        }
    }

    pub(super) fn cover_read(&mut self) -> Option<ReadTicket> {
        let target = self.cover_read_target()?;
        let answered = self.tags.effective_cover(&target).is_some()
            || self
                .tags
                .pending(&target)
                .is_some_and(|pending| pending.patch.cover_art.is_some())
            || self.tags.has_source_read(&target);
        if answered {
            return None;
        }
        self.tags.begin_read(&target)
    }

    pub(super) fn cover_read_target(&self) -> Option<PathBuf> {
        let selected = self.working_set.selected_files();
        selected
            .into_iter()
            .chain(self.working_set.files())
            .find(|file| file.is_valid)
            .map(|file| file.path.clone())
    }

    pub(crate) fn displayed_cover_at(&self, revision: u64) -> Option<Vec<u8>> {
        (self.cover.image_revision == revision)
            .then(|| self.cover.displayed.clone())
            .flatten()
    }

    pub(crate) fn lookup_offers_cover(&self, url: &str) -> bool {
        self.lookup
            .results
            .iter()
            .any(|result| result.cover_url.as_deref() == Some(url))
    }

    pub(crate) fn lists_source(&self, path: &std::path::Path) -> bool {
        self.working_set.files().iter().any(|file| {
            self.working_set
                .sources_for(file)
                .iter()
                .any(|source| source.path == path)
        })
    }

    pub(super) fn set_cover_notice(&mut self, notice: CoverNotice) {
        self.cover.notice = Some(notice);
        self.cover.notice_serial += 1;
    }

    pub(crate) fn begin_cover_request(&mut self, from_url: bool) -> u64 {
        self.cover.request += 1;
        if from_url {
            self.cover.loading = true;
            self.cover.notice = None;
        }
        self.cover.request
    }

    pub(crate) fn cover_request(&self) -> u64 {
        self.cover.request
    }

    pub(crate) fn cover_url_required(&mut self) {
        self.set_cover_notice(CoverNotice::UrlRequired);
    }

    pub(crate) fn apply_cover(&mut self, bytes: Vec<u8>) -> bool {
        let Some(owner) = self.cover_owner() else {
            return false;
        };
        self.tags.stage(
            &owner,
            &MetadataIntentPatch {
                cover_art: Some(PatchOp::Set(bytes)),
                ..Default::default()
            },
        );
        self.cover.custom = true;
        self.cover.removal_requested = false;
        true
    }

    pub(crate) fn clear_cover(&mut self) {
        if let Some(owner) = self.cover_owner() {
            self.tags.stage(
                &owner,
                &MetadataIntentPatch {
                    cover_art: Some(PatchOp::Clear),
                    ..Default::default()
                },
            );
        }
        self.cover.removal_requested = true;
        self.cover.custom = false;
        self.cover.notice = None;
        self.cover.loading = false;
        self.cover.request += 1;
    }

    pub(crate) fn cover_load_finished(
        &mut self,
        from_url: bool,
        result: Result<Vec<u8>, AppError>,
    ) -> bool {
        self.cover.loading = false;
        match result {
            Ok(bytes) => {
                let applied = self.apply_cover(bytes);
                if from_url {
                    self.set_cover_notice(CoverNotice::LoadedFromUrl);
                }
                applied
            }
            Err(error) => {
                log::error!("Failed to load cover art: {error}");
                self.set_cover_notice(CoverNotice::LoadFailed {
                    error: AppErrorEnvelope::from(&error),
                });
                false
            }
        }
    }
}
