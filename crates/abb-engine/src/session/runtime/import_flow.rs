use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::{blocking, Bound, Rest, Session};
use crate::audio;
use crate::audio::AudioFile;
use crate::errors::{AppErrorEnvelope, Result};
use crate::session::intent::SessionOutcome;
use crate::session::working_set::InputNotice;

impl Session {
    fn import_failed(&self, notice: InputNotice) -> SessionOutcome {
        self.transition(|state| state.working_set.set_notice(notice));
        SessionOutcome::Applied
    }

    pub(super) fn begin_import(&self, paths: Vec<String>) -> Rest {
        if self.lock().working_set.order_locked() {
            return Rest::Done(self.import_failed(InputNotice::OrderLocked));
        }
        Rest::Import {
            paths,
            resets: self.inner.resets.load(Ordering::SeqCst),
            turn: self.inner.imports.take(),
        }
    }

    pub(crate) fn import_opened(&self, paths: Vec<PathBuf>) -> Result<()> {
        let paths = paths
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        self.inner.deps.tasks.admit(|| {
            let rest = Rest::Import {
                paths,
                resets: self.inner.resets.load(Ordering::SeqCst),
                turn: self.inner.imports.take(),
            };
            let session = self.clone();
            self.inner.deps.tasks.spawn(async move {
                session.complete(rest).await;
            });
        })
    }

    pub(super) async fn import(
        &self,
        paths: Vec<String>,
        resets: u64,
        turn: crate::session::import_order::Turn,
    ) -> SessionOutcome {
        let _in_order = self.inner.imports.wait(turn).await;
        let superseded = || self.inner.resets.load(Ordering::SeqCst) != resets;
        let failed = |notice| {
            if superseded() {
                SessionOutcome::Superseded
            } else {
                self.import_failed(notice)
            }
        };

        let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let discovery =
            match blocking(move || Ok(audio::discover_audio_import_paths(&inputs))).await {
                Ok(discovery) => discovery,
                Err(error) => {
                    return failed(InputNotice::AnalysisFailed {
                        error: AppErrorEnvelope::from(&error),
                    })
                }
            };
        let discovered = discovery.paths;
        let skipped: Vec<_> = discovery
            .skipped
            .iter()
            .map(|(path, error)| audio::SkippedImportPath::new(path, error))
            .collect();
        if discovered.is_empty() {
            return failed(if skipped.is_empty() {
                InputNotice::NoSupportedFiles {
                    formats_text: audio::supported_audio_import_formats_text(),
                }
            } else {
                InputNotice::SkippedPaths { skipped }
            });
        }
        let aac_decoder = self.inner.deps.settings.aac_decoder();
        let analyzed =
            match blocking(move || audio::get_file_list_info(&discovered, aac_decoder)).await {
                Ok(analyzed) => analyzed,
                Err(error) => {
                    return failed(InputNotice::AnalysisFailed {
                        error: AppErrorEnvelope::from(&error),
                    })
                }
            };

        if superseded() {
            return SessionOutcome::Superseded;
        }
        let Some(bound) = self.list_imported(analyzed.files, skipped, resets).await else {
            return SessionOutcome::Superseded;
        };
        self.complete_reads(bound).await;
        SessionOutcome::Applied
    }

    /// Analysis can overlap a submission: wait before changing its locked list.
    async fn list_imported(
        &self,
        mut files: Vec<AudioFile>,
        skipped: Vec<audio::SkippedImportPath>,
        resets: u64,
    ) -> Option<Bound> {
        loop {
            let unlocked = self.inner.list_unlocked.notified();
            tokio::pin!(unlocked);
            unlocked.as_mut().enable();
            let attempt = self.transition(|state| {
                if self.inner.resets.load(Ordering::SeqCst) != resets {
                    return Some(None);
                }
                if state.working_set.order_locked() {
                    return None;
                }
                let default_audio = state.audio.request();
                state
                    .working_set
                    .append_analyzed(std::mem::take(&mut files), &default_audio);
                if !skipped.is_empty() {
                    state.working_set.set_notice(InputNotice::SkippedPaths {
                        skipped: skipped.clone(),
                    });
                }
                Some(Some(Bound::rebind(state)))
            });
            if let Some(bound) = attempt {
                return bound;
            }
            unlocked.await;
        }
    }
}
