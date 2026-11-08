use std::path::PathBuf;

use super::{blocking, Bound, Rest, Session};
use crate::audio;
use crate::audio::AudioFile;
use crate::errors::{AppErrorEnvelope, Result};
use crate::session::intent::SessionOutcome;
use crate::session::state::SessionState;
use crate::session::working_set::InputNotice;

impl Session {
    fn import_failed(&self, notice: InputNotice) -> SessionOutcome {
        self.transition(|state| state.working_set.set_notice(notice));
        SessionOutcome::Applied
    }

    pub(super) fn begin_import(&self, paths: Vec<String>) -> Rest {
        if self.lock().list_locked() {
            return Rest::Done(self.import_failed(InputNotice::OrderLocked));
        }
        Rest::Import {
            paths,
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
        turn: crate::session::import_order::Turn,
    ) -> SessionOutcome {
        let _in_order = self.inner.imports.wait(turn).await;

        let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let discovery =
            match blocking(move || Ok(audio::discover_audio_import_paths(&inputs))).await {
                Ok(discovery) => discovery,
                Err(error) => {
                    return self.import_failed(InputNotice::AnalysisFailed {
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
            return self.import_failed(if skipped.is_empty() {
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
                    return self.import_failed(InputNotice::AnalysisFailed {
                        error: AppErrorEnvelope::from(&error),
                    })
                }
            };

        let bound = self.list_imported(analyzed.files, skipped).await;
        self.complete_reads(bound).await;
        SessionOutcome::Applied
    }

    /// Analysis can overlap a submission: wait before changing its locked list.
    async fn list_imported(
        &self,
        mut files: Vec<AudioFile>,
        skipped: Vec<audio::SkippedImportPath>,
    ) -> Bound {
        self.when_list_unlocked(|state| {
            let default_audio = state.audio.request();
            state
                .working_set
                .append_analyzed(std::mem::take(&mut files), &default_audio);
            if !skipped.is_empty() {
                state.working_set.set_notice(InputNotice::SkippedPaths {
                    skipped: skipped.clone(),
                });
            }
            Bound::rebind(state)
        })
        .await
    }

    pub(super) async fn when_list_unlocked<T>(
        &self,
        mut apply: impl FnMut(&mut SessionState) -> T,
    ) -> T {
        loop {
            let unlocked = self.inner.list_unlocked.notified();
            tokio::pin!(unlocked);
            unlocked.as_mut().enable();
            let attempt = self.transition(|state| {
                if state.list_locked() {
                    return None;
                }
                Some(apply(state))
            });
            if let Some(result) = attempt {
                return result;
            }
            unlocked.await;
        }
    }
}
