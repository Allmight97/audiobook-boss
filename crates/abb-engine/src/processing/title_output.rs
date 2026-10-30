use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};
use crate::metadata::{
    plan_metadata_outcome_from, AudiobookMetadata, MetadataIntentPatch, MetadataOutcomeRequest,
    PassthroughSource, PatchOp,
};
use crate::output_artifact::{
    build_output_path_preview, lock_output_file, FileIdentity, OutputNamingConfig,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputUpdateStatus {
    /// Accepted; written before publication or as soon as the file is free.
    Waiting,
    Applied,
    /// The tags could not be written; the next Save tries again.
    Failed {
        message: String,
    },
    /// The title ended without an output.
    NotApplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputUpdate {
    /// The session edit revision this update carries.
    #[specta(type = specta_typescript::Number)]
    pub revision: u64,
    pub status: OutputUpdateStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateReply {
    Unchanged,
    /// Accepted. `published`: the file exists, and the caller writes it with
    /// `apply_published`. `elsewhere`: the tags now name a location other
    /// than where the output is.
    Accepted {
        published: bool,
        elsewhere: bool,
    },
    MovesOutput {
        from: PathBuf,
        to: PathBuf,
    },
    NoOutput,
}

#[derive(Debug, Clone)]
pub(crate) struct TitleOutputPlan {
    pub(crate) anchor: PathBuf,
    pub(crate) sources: Vec<PassthroughSource>,
    /// The anchor's own tags as read when the export was planned.
    pub(crate) base: Option<AudiobookMetadata>,
    pub(crate) accepted: Option<MetadataIntentPatch>,
    pub(crate) output_dir: PathBuf,
    pub(crate) naming: OutputNamingConfig,
    pub(crate) extension: String,
    /// Where the accepted naming put the output, before collision handling.
    pub(crate) requested: PathBuf,
}

/// The tags an output carries or should carry: text fields from the planned
/// metadata, and the cover as the edit asks for it (`None`: the sources').
#[derive(Debug, Clone, PartialEq)]
struct Tags {
    text: AudiobookMetadata,
    cover: Option<PatchOp<Vec<u8>>>,
}

#[derive(Debug)]
enum Phase {
    Pending,
    Published {
        path: PathBuf,
        identity: Option<FileIdentity>,
    },
    Ended,
}

struct State {
    carried: Tags,
    wanted: Option<(u64, Tags)>,
    phase: Phase,
    update: Option<OutputUpdate>,
    newest: u64,
    moving: Option<(u64, Tags, PathBuf)>,
    kept_location: Option<PathBuf>,
}

type Writer = dyn Fn(&Path, &MetadataIntentPatch) -> Result<()> + Send + Sync;
type Listener = dyn Fn(Option<OutputUpdate>) + Send + Sync;

pub struct TitleOutput {
    plan: TitleOutputPlan,
    state: Mutex<State>,
    /// Held across every write to this title's output, publication included.
    writing: Mutex<()>,
    settled: tokio::sync::watch::Sender<Option<bool>>,
    on_change: OnceLock<Box<Listener>>,
    write: Box<Writer>,
}

impl std::fmt::Debug for TitleOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TitleOutput")
            .field("requested", &self.plan.requested)
            .finish_non_exhaustive()
    }
}

impl TitleOutput {
    pub(crate) fn new(plan: TitleOutputPlan) -> Result<Arc<Self>> {
        Self::with_writer(plan, Box::new(crate::metadata::save_metadata_intent))
    }

    pub(crate) fn with_writer(plan: TitleOutputPlan, write: Box<Writer>) -> Result<Arc<Self>> {
        let carried = plan.tags(plan.accepted.as_ref())?.0;
        Ok(Arc::new(Self {
            plan,
            state: Mutex::new(State {
                carried,
                wanted: None,
                phase: Phase::Pending,
                update: None,
                newest: 0,
                moving: None,
                kept_location: None,
            }),
            writing: Mutex::new(()),
            settled: tokio::sync::watch::channel(None).0,
            on_change: OnceLock::new(),
            write,
        }))
    }

    pub(crate) fn on_change(&self, listener: Box<Listener>) {
        let _ = self.on_change.set(listener);
    }

    pub(crate) fn update_state(&self) -> Option<OutputUpdate> {
        self.lock().update.clone()
    }

    /// Accepts `intent` (the whole edit relative to the source's tags as read
    /// for this export) as edit `revision`.
    pub(crate) fn update(
        &self,
        revision: u64,
        intent: &MetadataIntentPatch,
    ) -> Result<UpdateReply> {
        let (tags, location) = self.plan.tags(Some(intent))?;
        let mut state = self.lock();
        if revision < state.newest {
            return Ok(UpdateReply::Unchanged);
        }
        state.newest = revision;
        let target = state
            .wanted
            .as_ref()
            .map_or(&state.carried, |(_, tags)| tags);
        let reply = match &state.phase {
            Phase::Ended => UpdateReply::NoOutput,
            _ if *target == tags => UpdateReply::Unchanged,
            Phase::Pending
                if location != self.plan.requested
                    && state.kept_location.as_ref() != Some(&location) =>
            {
                state.moving = Some((revision, tags, location.clone()));
                UpdateReply::MovesOutput {
                    from: self.plan.requested.clone(),
                    to: location,
                }
            }
            phase => {
                let published = matches!(phase, Phase::Published { .. });
                state.moving = None;
                state.wanted = Some((revision, tags));
                state.update = Some(OutputUpdate {
                    revision,
                    status: OutputUpdateStatus::Waiting,
                });
                UpdateReply::Accepted {
                    published,
                    elsewhere: location != self.plan.requested,
                }
            }
        };
        let changed = matches!(reply, UpdateReply::Accepted { .. });
        drop(state);
        if changed {
            self.notify();
        }
        Ok(reply)
    }

    pub(crate) fn keep_location(&self, revision: u64) -> Result<bool> {
        let mut state = self.lock();
        if matches!(state.phase, Phase::Ended) {
            return Err(AppError::InvalidInput(
                "This export ended without an output.".into(),
            ));
        }
        let Some((latest, tags, location)) = state.moving.take() else {
            return Err(AppError::InvalidInput(
                "That location offer is no longer pending.".into(),
            ));
        };
        if latest != revision || state.newest != revision {
            state.moving = Some((latest, tags, location));
            return Err(AppError::InvalidInput(
                "That location offer was replaced by a newer Save.".into(),
            ));
        }
        state.kept_location = Some(location);
        let published = matches!(state.phase, Phase::Published { .. });
        state.wanted = Some((revision, tags));
        state.update = Some(OutputUpdate {
            revision,
            status: OutputUpdateStatus::Waiting,
        });
        drop(state);
        self.notify();
        Ok(published)
    }

    pub(crate) fn publish<T>(
        &self,
        staged: &Path,
        final_path: &Path,
        commit: impl FnOnce() -> Result<(T, PathBuf)>,
    ) -> Result<T> {
        let _title = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
        let _file = lock_output_file(final_path);
        self.write_wanted(staged);
        let (value, published) = commit()?;
        let identity = FileIdentity::of(&published)
            .map_err(|error| {
                log::warn!("title_output status=identity_unreadable err={error}");
            })
            .ok();
        self.lock().phase = Phase::Published {
            path: published.clone(),
            identity,
        };
        if identity.is_some() {
            self.write_wanted(&published);
            self.record_identity(&published);
        } else {
            self.fail_wanted("The output was published, but ABB could not capture its identity. Its tags were not updated after publication.".into());
        }
        self.settled.send_replace(Some(true));
        Ok(value)
    }

    pub(crate) fn apply_published(&self) {
        let _title = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
        let (path, identity) = match &self.lock().phase {
            Phase::Published { path, identity } => (path.clone(), *identity),
            _ => return,
        };
        let _file = lock_output_file(&path);
        let Some(identity) = identity else {
            self.fail_wanted("ABB could not capture this published output's identity; its tags cannot be safely updated.".into());
            return;
        };
        if FileIdentity::of(&path).ok() != Some(identity) {
            self.fail_wanted("The output changed since ABB wrote it.".to_string());
            return;
        }
        self.write_wanted(&path);
        self.record_identity(&path);
    }

    pub(crate) fn end(&self) {
        let mut state = self.lock();
        let unpublished = matches!(state.phase, Phase::Pending);
        if unpublished {
            state.phase = Phase::Ended;
            if state.wanted.take().is_some() {
                if let Some(update) = &mut state.update {
                    update.status = OutputUpdateStatus::NotApplied;
                }
            }
        }
        drop(state);
        if unpublished {
            self.settled.send_replace(Some(false));
            self.notify();
        }
    }

    /// Waits until the title published (`true`) or ended without an output.
    pub(crate) async fn settled(&self) -> bool {
        let mut settled = self.settled.subscribe();
        let published = match settled.wait_for(Option::is_some).await {
            Ok(published) => published.unwrap_or(false),
            Err(_) => false,
        };
        published
    }

    fn write_wanted(&self, path: &Path) {
        loop {
            let (revision, tags, carried) = {
                let state = self.lock();
                let Some((revision, tags)) = state.wanted.clone() else {
                    return;
                };
                (revision, tags, state.carried.clone())
            };
            let written = self.plan.patch_between(&carried, &tags).and_then(|patch| {
                match patch.is_actionable() {
                    true => (self.write)(path, &patch),
                    false => Ok(()),
                }
            });
            let mut state = self.lock();
            let current = state.wanted.as_ref().map(|(newest, _)| *newest) == Some(revision);
            match written {
                Ok(()) => {
                    state.carried = tags;
                    if current {
                        state.wanted = None;
                        state.update = Some(OutputUpdate {
                            revision,
                            status: OutputUpdateStatus::Applied,
                        });
                    }
                }
                Err(error) => {
                    log::warn!("title_output status=write_failed err={error}");
                    if current {
                        state.wanted = None;
                        state.update = Some(OutputUpdate {
                            revision,
                            status: OutputUpdateStatus::Failed {
                                message: error.to_string(),
                            },
                        });
                    }
                }
            }
            drop(state);
            self.notify();
        }
    }

    fn fail_wanted(&self, message: String) {
        let mut state = self.lock();
        if let Some((revision, _)) = state.wanted.take() {
            state.update = Some(OutputUpdate {
                revision,
                status: OutputUpdateStatus::Failed { message },
            });
        }
        drop(state);
        self.notify();
    }

    fn record_identity(&self, path: &Path) {
        if let Ok(identity) = FileIdentity::of(path) {
            if let Phase::Published {
                identity: recorded, ..
            } = &mut self.lock().phase
            {
                *recorded = Some(identity);
            }
        }
    }

    fn notify(&self) {
        if let Some(listener) = self.on_change.get() {
            listener(self.update_state());
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl TitleOutputPlan {
    fn tags(&self, intent: Option<&MetadataIntentPatch>) -> Result<(Tags, PathBuf)> {
        let outcome = plan_metadata_outcome_from(
            self.base.clone(),
            MetadataOutcomeRequest {
                input_path: Some(&self.anchor),
                intent_patch: intent,
            },
        )?;
        let mut text = outcome.effective_metadata.unwrap_or_default();
        text.cover_art = None;
        let location = build_output_path_preview(
            &self.output_dir,
            outcome.naming_metadata.as_ref(),
            self.naming.clone(),
            Some(&self.anchor),
        )?
        .with_extension(&self.extension);
        let tags = Tags {
            text,
            cover: intent.and_then(|intent| intent.cover_art.clone()),
        };
        Ok((tags, location))
    }

    fn patch_between(&self, from: &Tags, to: &Tags) -> Result<MetadataIntentPatch> {
        let mut patch = MetadataIntentPatch::between(&from.text, &to.text);
        if from.cover != to.cover {
            patch.cover_art = Some(match &to.cover {
                Some(PatchOp::Set(bytes)) => {
                    PatchOp::Set(crate::metadata::prepare_cover_art_for_write(bytes)?)
                }
                Some(PatchOp::Clear) => PatchOp::Clear,
                None => match self.source_cover()? {
                    Some(bytes) => PatchOp::Set(bytes),
                    None => PatchOp::Clear,
                },
            });
        }
        Ok(patch)
    }

    fn source_cover(&self) -> Result<Option<Vec<u8>>> {
        let own = self.base.as_ref().and_then(|base| base.cover_art.clone());
        let cover =
            own.or_else(|| crate::metadata::extract_passthrough_metadata(&self.sources).cover_art);
        cover
            .map(|bytes| crate::metadata::prepare_cover_art_for_write(&bytes))
            .transpose()
            .map_err(|error| AppError::General(format!("Cannot prepare the source cover: {error}")))
    }
}

#[cfg(test)]
#[path = "title_output_tests.rs"]
mod tests;
