use std::path::PathBuf;
use std::sync::Arc;

use super::{blocking, Bound, Session};
use crate::audio;
use crate::errors::AppErrorEnvelope;
use crate::processing::SupplementalProcessingAsset;
use crate::remote_source::{AcquisitionHandoff, AcquisitionJob, Handoff, HandoffRefusal};
use crate::session::state::SessionState;
use crate::work_runtime::OperationSnapshot;

impl Session {
    pub(crate) fn handoff(&self) -> Handoff {
        let session = Arc::downgrade(&self.inner);
        Arc::new(move |job| {
            let session = session.upgrade().map(|inner| Session { inner });
            Box::pin(async move {
                match session {
                    Some(session) => session.import_acquired(job).await,
                    None => AcquisitionHandoff::Removed {
                        reason: HandoffRefusal::NothingAdded,
                    },
                }
            })
        })
    }

    pub(super) async fn import_acquired(&self, job: AcquisitionJob) -> AcquisitionHandoff {
        let handoff = self.import_acquired_files(&job).await;
        if matches!(handoff, AcquisitionHandoff::Removed { .. }) {
            let paths = job
                .materialized_files
                .iter()
                .map(|file| &file.path)
                .chain(job.supplemental_assets.iter().map(|asset| &asset.path))
                .map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| path.clone()))
                .collect();
            self.transition(|state| state.staged.register_unimported(&job.job_id, paths));
        }
        handoff
    }

    async fn import_acquired_files(&self, job: &AcquisitionJob) -> AcquisitionHandoff {
        let _in_order = self.inner.imports.wait(self.inner.imports.take()).await;
        let removed = |reason| AcquisitionHandoff::Removed { reason };
        let paths: Vec<PathBuf> = job
            .materialized_files
            .iter()
            .map(|file| file.path.clone())
            .collect();
        let aac_decoder = self.inner.deps.settings.aac_decoder();
        let analyzed = blocking(move || {
            let discovery = audio::discover_audio_import_paths(&paths);
            if let Some((_, error)) = discovery.skipped.into_iter().next() {
                return Err(error);
            }
            audio::get_file_list_info(&discovery.paths, aac_decoder)
        })
        .await;
        let analyzed = match analyzed {
            Ok(analyzed) => analyzed.files,
            Err(error) => {
                return removed(HandoffRefusal::ImportFailed {
                    error: AppErrorEnvelope::from(&error),
                });
            }
        };
        let titles = staged_titles(job);
        let listed = self
            .when_list_unlocked(|state| list_acquired(state, job, analyzed.clone(), &titles))
            .await;
        match listed {
            Ok((bound, count)) => {
                self.complete_reads(bound).await;
                self.publish();
                AcquisitionHandoff::Imported { count }
            }
            Err(reason) => {
                self.publish();
                removed(reason)
            }
        }
    }

    pub(super) fn claim_sweep(&self, state: &mut SessionState) -> bool {
        !state.staged.is_empty()
            && !state.sweeping.held()
            && !state
                .removable_staged(&self.inner.deps.work.sources_held(), now())
                .is_empty()
            && state.sweeping.take()
    }

    pub(super) fn sweep_staged(&self) {
        let session = self.clone();
        let mut claim = Some(self.sweep_claim());
        self.inner.deps.tasks.spawn(async move {
            loop {
                let (jobs, retry_at) = session.transition(|state| {
                    let held = session.inner.deps.work.sources_held();
                    let jobs = state.begin_staged_removal(&held, now());
                    if jobs.is_empty() {
                        if let Some(claim) = claim.take() {
                            claim.end_in(state);
                        }
                    }
                    (jobs, state.staged.next_retry())
                });
                if jobs.is_empty() {
                    if let Some(retry_at) = retry_at {
                        session.retry_sweep_at(retry_at).await;
                    }
                    return;
                }
                for job_id in jobs {
                    let remove = Arc::clone(&session.inner.deps.remove_staged);
                    let id = job_id.clone();
                    let removed = blocking(move || remove(&id)).await;
                    let (bound, new_retry_delay) = session.transition(|state| {
                        let finished = state.finish_staged_removal(&job_id, removed.is_ok(), now());
                        let bound = Bound {
                            reads: finished.reads,
                            binding: state.binding,
                        };
                        (bound, finished.new_retry_delay)
                    });
                    if let Err(error) = &removed {
                        match new_retry_delay {
                            Some(delay) => log::warn!(
                                "Failed to remove staged download job_id={job_id}, retrying in {delay:?}: {error}"
                            ),
                            None => log::debug!(
                                "Failed again to remove staged download job_id={job_id}: {error}"
                            ),
                        }
                    }
                    session.complete_reads(bound).await;
                }
                session.publish();
            }
        });
    }

    async fn retry_sweep_at(&self, retry_at: std::time::Instant) {
        let wait = async {
            tokio::time::sleep_until(tokio::time::Instant::from_std(retry_at)).await;
            Ok(())
        };
        if self.inner.deps.tasks.until_closing(wait).await.is_ok() {
            self.transition(|_| ());
        }
    }

    pub(super) fn export_finished(&self, snapshot: &OperationSnapshot) {
        self.transition(|state| state.staged.finish_export(&snapshot.children));
    }
}

fn list_acquired(
    state: &mut SessionState,
    job: &AcquisitionJob,
    analyzed: Vec<crate::audio::AudioFile>,
    titles: &[(String, PathBuf)],
) -> std::result::Result<(Bound, usize), HandoffRefusal> {
    let before = state.working_set.source_paths();
    let default_audio = state.audio.request();
    state.working_set.append_analyzed(analyzed, &default_audio);
    let mut count = 0;
    for file in state.working_set.files().to_vec() {
        if before.contains(&file.path) {
            continue;
        }
        let Some((title_id, _)) = titles.iter().find(|(_, path)| *path == file.path) else {
            continue;
        };
        let assets = job
            .supplemental_assets
            .iter()
            .filter(|asset| &asset.title_id == title_id)
            .map(|asset| SupplementalProcessingAsset {
                asset_id: asset.asset_id.clone(),
                input_id: file.input_id.clone(),
                title_id: asset.title_id.clone(),
                path: asset.path.clone(),
                file_name: asset.file_name.clone(),
                size_bytes: asset.size_bytes,
                sha256: asset.sha256.clone(),
            })
            .collect();
        state
            .staged
            .register(&job.job_id, &file.input_id, file.path.clone(), assets);
        count += 1;
    }
    if count == 0 {
        return Err(HandoffRefusal::NothingAdded);
    }
    Ok((Bound::rebind(state), count))
}

fn staged_titles(job: &AcquisitionJob) -> Vec<(String, PathBuf)> {
    job.materialized_files
        .iter()
        .map(|file| {
            let path = std::fs::canonicalize(&file.path).unwrap_or_else(|_| file.path.clone());
            (file.title_id.clone(), path)
        })
        .collect()
}

/// The time staged-download retries are measured in: tokio's clock, so a
/// paused test runtime can move it.
fn now() -> std::time::Instant {
    tokio::time::Instant::now().into_std()
}
