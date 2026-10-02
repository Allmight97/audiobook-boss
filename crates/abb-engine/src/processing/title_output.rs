//! One export title's output after acceptance: the tags it carries, the
//! latest edit accepted for it, and when that edit reaches the file.
//!
//! An edit accepted before publication is written to the staged file just
//! before it is published, under the same locks as publication. One accepted
//! after publication is written to the published file once its size and
//! modification time show it is still the file ABB wrote. Writes to one
//! output never overlap, and a failed write never changes the title's export
//! outcome.

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

/// How the latest edit accepted for a title's output is going.
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

/// What `TitleOutput::update` did with an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateReply {
    /// The output already carries it, or will.
    Unchanged,
    /// Accepted. `published`: the file exists, and the caller writes it with
    /// `apply_published`. `elsewhere`: the tags now name a location other
    /// than where the output is.
    Accepted { published: bool, elsewhere: bool },
    /// Not applied: the output is unpublished and the edit names another
    /// location. The title must be restarted to follow it.
    MovesOutput { from: PathBuf, to: PathBuf },
    /// The title ended without an output.
    NoOutput,
}

/// Everything fixed when the export was accepted.
#[derive(Debug, Clone)]
pub(crate) struct TitleOutputPlan {
    pub(crate) anchor: PathBuf,
    /// Every source, for the cover processing would take when the edit leaves
    /// the cover to the sources.
    pub(crate) sources: Vec<PassthroughSource>,
    /// The anchor's own tags as read when the export was planned.
    pub(crate) base: Option<AudiobookMetadata>,
    /// The intent the export was accepted with.
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
        identity: FileIdentity,
    },
    /// Ended without publishing.
    Ended,
}

struct State {
    carried: Tags,
    wanted: Option<(u64, Tags)>,
    phase: Phase,
    update: Option<OutputUpdate>,
    /// The newest edit revision seen; an older one arriving late is ignored.
    newest: u64,
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
            }),
            writing: Mutex::new(()),
            settled: tokio::sync::watch::channel(None).0,
            on_change: OnceLock::new(),
            write,
        }))
    }

    /// Calls `listener` with the update state whenever it changes.
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
        // Saves read their edits in order but may reach here out of order.
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
            Phase::Pending if location != self.plan.requested => UpdateReply::MovesOutput {
                from: self.plan.requested.clone(),
                to: location,
            },
            phase => {
                let published = matches!(phase, Phase::Published { .. });
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

    /// Publishes the title: writes an accepted edit to `staged`, runs
    /// `commit`, which returns the published path, then writes any edit
    /// accepted meanwhile to the published file. Blocking.
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
        match FileIdentity::of(&published) {
            Ok(identity) => {
                self.lock().phase = Phase::Published {
                    path: published.clone(),
                    identity,
                };
                self.write_wanted(&published);
                self.record_identity(&published);
            }
            Err(error) => {
                log::warn!("title_output status=identity_unreadable err={error}");
            }
        }
        self.settled.send_replace(Some(true));
        Ok(value)
    }

    /// Writes an accepted edit to the published output. Blocking.
    pub(crate) fn apply_published(&self) {
        let _title = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
        let (path, identity) = match &self.lock().phase {
            Phase::Published { path, identity } => (path.clone(), *identity),
            _ => return,
        };
        let _file = lock_output_file(&path);
        if FileIdentity::of(&path).ok() != Some(identity) {
            self.fail_wanted("The output changed since ABB wrote it.".to_string());
            return;
        }
        self.write_wanted(&path);
        self.record_identity(&path);
    }

    /// Ends the title. One that did not publish drops any edit still waiting.
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
                *recorded = identity;
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
    /// The tags and location the output gets under `intent`.
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

    /// The tag save that turns an output carrying `from` into one carrying
    /// `to`. A cover left to the sources is the one processing takes: the
    /// first source's own picture.
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
