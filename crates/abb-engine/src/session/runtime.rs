//! Runs the working session: applies intents to [`SessionState`], performs
//! the file and network work a transition asks for, and tells the host what
//! changed.
//!
//! The state lock is never held across an await. Each async workflow reads
//! what it needs under the lock, works without it, and comes back with a
//! completion the state accepts or drops.
//!
//! An intent runs in two steps. [`Session::begin`] applies its immediate
//! effect before returning, so intents begun in order take effect in order.
//! [`SessionRun::finish`] then does the file or network work, which may
//! overlap with later intents.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use super::lookup::{
    self, LookupApplyMode, LookupSource, LookupStatus, QueueStep, QueuedTitle, RESULT_LIMIT,
};
use super::metadata_form::{FieldAction, MetadataField};
use super::state::{
    GateBlock, MetadataStatus, SaveItem, SavePlan, SessionState, SessionUpdate, StageOutcome,
};
use super::tag_cache::ReadTicket;
use super::working_set::{CueChoice, InputNotice, MoveDirection, SelectionModifiers, WorkingSet};
use crate::audio::{self, TitleAudioRequest};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::host::{EngineEvent, Host};
use crate::metadata::{AudiobookMetadata, MetadataIntentPatch};
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource, OnlineMetadataResult};
use crate::metadata_save::{save_metadata_batch, MetadataSaveRequest, MetadataSaveResultStatus};
use crate::opened_audio::OpenedAudioFileQueue;
use crate::work_runtime::WorkRuntime;
use crate::ManagedJobRegistry;

/// How many source files are read for tags at once.
const READ_CONCURRENCY: usize = 8;

/// Something the user asked the session to do.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionIntent {
    // ---- Titles ----
    /// Discovers and analyzes audio under `paths` and adds new titles, each
    /// taking `default_audio` as its audio request.
    #[serde(rename_all = "camelCase")]
    Import {
        paths: Vec<String>,
        default_audio: TitleAudioRequest,
    },
    /// Imports the files the operating system asked ABB to open.
    #[serde(rename_all = "camelCase")]
    ImportOpened {
        default_audio: TitleAudioRequest,
    },
    SelectFile {
        index: usize,
        modifiers: SelectionModifiers,
    },
    SelectAll,
    ClearSelection,
    RemoveFile {
        index: usize,
    },
    ClearAll,
    MoveFile {
        index: usize,
        direction: MoveDirection,
    },
    ReorderFiles {
        from: usize,
        to: usize,
    },
    ToggleSort,
    RestoreImportOrder,
    SetOrderLocked {
        locked: bool,
    },
    GroupSelected,
    #[serde(rename_all = "camelCase")]
    Ungroup {
        title_id: String,
    },
    #[serde(rename_all = "camelCase")]
    ReorderSources {
        title_id: String,
        from: usize,
        to: usize,
    },
    #[serde(rename_all = "camelCase")]
    ChooseCue {
        input_id: String,
        choice: CueChoice,
    },
    #[serde(rename_all = "camelCase")]
    SetAudioRequest {
        title_id: String,
        request: TitleAudioRequest,
    },
    /// Returns the session to empty.
    Reset,

    // ---- Metadata ----
    SetField {
        field: MetadataField,
        value: String,
    },
    SetFieldAction {
        field: MetadataField,
        action: FieldAction,
    },
    LoadCoverFromFile {
        path: String,
    },
    LoadCoverFromUrl {
        url: String,
    },
    ClearCover,
    /// Stages the edits on screen so processing can take them.
    StageSelection,
    /// Writes every pending edit that can be written now.
    Save,

    // ---- Lookup ----
    LookupOpen,
    LookupClose,
    LookupSearch,
    LookupApply {
        index: usize,
    },
    LookupSkip,
    LookupSetTitleQuery {
        value: String,
    },
    LookupSetAuthorQuery {
        value: String,
    },
    LookupSetSource {
        source: LookupSource,
    },
    LookupSetApplyMode {
        mode: LookupApplyMode,
    },
    LookupSetReplaceCover {
        replace: bool,
    },
}

/// Whether an intent took effect. Details a user needs are in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionOutcome {
    Applied,
    /// The edits on screen were not accepted, so nothing changed. `message`
    /// is absent when a save in progress is what blocked the change.
    DraftRejected {
        message: Option<String>,
    },
    /// There are edits and no valid title to carry them.
    NoTarget,
    CoverLoadFailed,
    /// A newer request or a reset replaced this one before it finished.
    Superseded,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionReply {
    pub outcome: SessionOutcome,
    /// What changed since the intent was received.
    pub update: SessionUpdate,
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The session's network calls, replaceable so lookup rules can be proven
/// without a network.
pub(crate) struct Network {
    pub(crate) search: Box<SearchFn>,
    pub(crate) cover_from_url: Box<CoverFromUrlFn>,
}

type SearchFn =
    dyn Fn(String, Vec<MetadataSource>) -> BoxFuture<Result<MetadataLookupResponse>> + Send + Sync;
type CoverFromUrlFn = dyn Fn(String) -> BoxFuture<Result<Vec<u8>>> + Send + Sync;

impl Network {
    fn live() -> Self {
        Self {
            search: Box::new(|query, sources| {
                Box::pin(crate::metadata_lookup::search_online_metadata(
                    query,
                    Some(sources),
                    Some(RESULT_LIMIT),
                ))
            }),
            cover_from_url: Box::new(|url| {
                Box::pin(crate::cover_source::load_cover_art_from_url(url))
            }),
        }
    }
}

pub(crate) struct SessionDeps {
    pub(crate) host: Host,
    pub(crate) work: WorkRuntime,
    pub(crate) jobs: ManagedJobRegistry,
    /// Source files under this root are temporary downloads.
    pub(crate) temporary_root: PathBuf,
    pub(crate) opened_audio: Arc<OpenedAudioFileQueue>,
    /// Preview runs in flight. Save waits for them because they read sources.
    pub(crate) previews: Arc<AtomicUsize>,
}

/// One working session. Cloning shares it.
#[derive(Clone)]
pub(crate) struct Session {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    state: Mutex<SessionState>,
    deps: SessionDeps,
    network: Network,
    /// One import runs at a time, in the order requested.
    imports: tokio::sync::Mutex<()>,
    /// Advances on reset; an import that started earlier is dropped.
    resets: AtomicU64,
    /// The revision the last event carried.
    published: AtomicU64,
    deferred_writer_running: AtomicBool,
}

/// A selection change the state accepted and the reads it asked for.
struct Bound {
    binding: u64,
    reads: Vec<ReadTicket>,
}

/// An intent whose immediate effect has been applied. Await
/// [`SessionRun::finish`] for the rest and the reply; a run that is dropped
/// instead leaves its work undone.
#[must_use = "an intent that began must be finished"]
pub struct SessionRun {
    session: Session,
    /// The session revision before the intent began.
    since: u64,
    rest: Rest,
}

/// What an intent still has to do after its immediate effect.
enum Rest {
    Done(SessionOutcome),
    Reads(Bound),
    Import {
        paths: Vec<String>,
        default_audio: TitleAudioRequest,
        resets: u64,
    },
    Save {
        epoch: u64,
        plan: SavePlan,
    },
    CoverLoad {
        source: CoverSource,
        started: (u64, u64),
    },
    LookupSearch {
        request: u64,
    },
    LookupApply {
        request: u64,
        chosen: Box<ChosenResult>,
    },
    LookupAdvance {
        request: u64,
        step: QueueStep,
    },
}

/// A lookup result picked for a queued title.
struct ChosenResult {
    result: OnlineMetadataResult,
    title: QueuedTitle,
    replace_cover: bool,
    mode: LookupApplyMode,
}

impl SessionRun {
    /// Does the intent's file or network work and returns what changed since
    /// it began.
    pub async fn finish(self) -> SessionReply {
        let session = self.session;
        let outcome = match self.rest {
            Rest::Done(outcome) => outcome,
            Rest::Reads(bound) => {
                session.complete_reads(bound).await;
                SessionOutcome::Applied
            }
            Rest::Import {
                paths,
                default_audio,
                resets,
            } => session.import(paths, default_audio, resets).await,
            Rest::Save { epoch, plan } => session.save(epoch, plan).await,
            Rest::CoverLoad { source, started } => session.load_cover(source, started).await,
            Rest::LookupSearch { request } => {
                session.publish();
                session.lookup_search(request, None).await
            }
            Rest::LookupApply { request, chosen } => session.lookup_apply(request, *chosen).await,
            Rest::LookupAdvance { request, step } => session.lookup_advance(request, step).await,
        };
        let update = session.lock().update_since(Some(self.since));
        SessionReply { outcome, update }
    }
}

impl Session {
    pub(crate) fn new(deps: SessionDeps) -> Self {
        Self::with_network(deps, Network::live())
    }

    pub(crate) fn with_network(deps: SessionDeps, network: Network) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                state: Mutex::new(SessionState::default()),
                deps,
                network,
                imports: tokio::sync::Mutex::new(()),
                resets: AtomicU64::new(0),
                published: AtomicU64::new(0),
                deferred_writer_running: AtomicBool::new(false),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, SessionState> {
        // A panic mid-transition leaves state that is still structurally
        // valid; the session keeps serving rather than failing every intent.
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs one atomic transition and re-derives the snapshots.
    fn transition<T>(&self, change: impl FnOnce(&mut SessionState) -> T) -> T {
        let mut state = self.lock();
        let value = change(&mut state);
        state.settle();
        value
    }

    /// Tells the host what changed since the last event. Used before a wait
    /// the user should not sit through blind, and by background work.
    fn publish(&self) {
        let update = {
            let state = self.lock();
            let since = self
                .inner
                .published
                .swap(state.revision(), Ordering::SeqCst);
            if since == state.revision() {
                return;
            }
            state.update_since(Some(since))
        };
        self.inner.deps.host.emit(EngineEvent::Session(update));
    }

    // ---- Host-facing ----

    pub(crate) async fn dispatch(&self, intent: SessionIntent) -> SessionReply {
        self.begin(intent).finish().await
    }

    /// Applies the intent's immediate effect. Intents begun in order take
    /// effect in order, whatever work each still has to finish.
    pub(crate) fn begin(&self, intent: SessionIntent) -> SessionRun {
        let since = self.lock().revision();
        let rest = self.begin_rest(intent);
        SessionRun {
            session: self.clone(),
            since,
            rest,
        }
    }

    pub(crate) fn snapshot(&self) -> SessionUpdate {
        self.lock().update_since(None)
    }

    pub(crate) fn cover_art(&self) -> Option<Vec<u8>> {
        self.lock().displayed_cover()
    }

    pub(crate) fn pending_intents(
        &self,
        paths: &[PathBuf],
    ) -> HashMap<String, MetadataIntentPatch> {
        self.lock()
            .pending_intents(paths)
            .into_iter()
            .map(|(path, patch)| (path.to_string_lossy().into_owned(), patch))
            .collect()
    }

    /// Source files with a Save accepted and not yet written.
    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        self.lock().waiting_write_paths()
    }

    #[allow(clippy::too_many_lines)] // one arm per intent; each arm is a call
    fn begin_rest(&self, intent: SessionIntent) -> Rest {
        use SessionIntent as I;
        match intent {
            I::Import {
                paths,
                default_audio,
            } => self.begin_import(paths, default_audio),
            I::ImportOpened { default_audio } => match self.inner.deps.opened_audio.take_paths() {
                Ok(paths) if paths.is_empty() => Rest::Done(SessionOutcome::Applied),
                Ok(paths) => self.begin_import(paths, default_audio),
                Err(error) => Rest::Done(self.import_failed(InputNotice::DiscoveryFailed {
                    error: AppErrorEnvelope::from(&error),
                })),
            },
            I::SelectFile { index, modifiers } => {
                self.change_selection(|set| set.select_file(index, modifiers))
            }
            I::SelectAll => self.change_selection(WorkingSet::select_all),
            I::ClearSelection => self.change_selection(WorkingSet::clear_selection),
            I::RemoveFile { index } => self.change_selection(|set| {
                set.remove_file(index);
            }),
            I::ClearAll => self.change_selection(|set| {
                set.clear_all();
            }),
            I::GroupSelected => self.change_selection(|set| {
                set.group_selected();
            }),
            I::Ungroup { title_id } => self.change_selection(|set| {
                set.ungroup(&title_id);
            }),
            I::MoveFile { index, direction } => {
                self.edit_titles(|set| set.move_file(index, direction))
            }
            I::ReorderFiles { from, to } => self.edit_titles(|set| set.reorder_files(from, to)),
            I::ToggleSort => self.edit_titles(WorkingSet::toggle_sort),
            I::RestoreImportOrder => self.edit_titles(WorkingSet::restore_import_order),
            I::SetOrderLocked { locked } => self.edit_titles(|set| set.set_order_locked(locked)),
            I::ReorderSources { title_id, from, to } => {
                self.edit_titles(|set| set.reorder_sources(&title_id, from, to))
            }
            I::ChooseCue { input_id, choice } => {
                self.edit_titles(|set| set.choose_cue(&input_id, choice))
            }
            I::SetAudioRequest { title_id, request } => {
                self.edit_titles(|set| set.set_audio_request(&title_id, request))
            }
            I::Reset => {
                self.inner.resets.fetch_add(1, Ordering::SeqCst);
                self.transition(SessionState::reset);
                Rest::Done(SessionOutcome::Applied)
            }

            I::SetField { field, value } => {
                self.transition(|state| state.set_field(field, value));
                Rest::Done(SessionOutcome::Applied)
            }
            I::SetFieldAction { field, action } => {
                self.transition(|state| state.set_field_action(field, action));
                Rest::Done(SessionOutcome::Applied)
            }
            I::LoadCoverFromFile { path } => self.begin_cover_load(CoverSource::File(path)),
            I::LoadCoverFromUrl { url } => self.begin_cover_load(CoverSource::Url(url)),
            I::ClearCover => {
                self.transition(SessionState::clear_cover);
                Rest::Done(SessionOutcome::Applied)
            }
            I::StageSelection => {
                Rest::Done(self.transition(|state| match state.stage_bound_form() {
                    StageOutcome::Staged => SessionOutcome::Applied,
                    StageOutcome::NoTarget => SessionOutcome::NoTarget,
                    StageOutcome::Invalid(message) => {
                        state.set_status(MetadataStatus::DraftInvalid {
                            message: message.clone(),
                        });
                        SessionOutcome::DraftRejected {
                            message: Some(message),
                        }
                    }
                }))
            }
            I::Save => self.begin_save(),

            I::LookupOpen => self.begin_lookup_open(),
            I::LookupClose => {
                self.transition(|state| {
                    state.lookup.request += 1;
                    state.lookup.open = false;
                });
                Rest::Done(SessionOutcome::Applied)
            }
            I::LookupSearch => Rest::LookupSearch {
                request: self.begin_lookup_action(),
            },
            I::LookupApply { index } => self.begin_lookup_apply(index),
            I::LookupSkip => Rest::LookupAdvance {
                request: self.begin_lookup_action(),
                step: QueueStep::Skipped,
            },
            I::LookupSetTitleQuery { value } => {
                self.edit_lookup(|lookup| lookup.title_query = value)
            }
            I::LookupSetAuthorQuery { value } => {
                self.edit_lookup(|lookup| lookup.author_query = value)
            }
            I::LookupSetSource { source } => self.edit_lookup(|lookup| lookup.source = source),
            I::LookupSetApplyMode { mode } => self.edit_lookup(|lookup| lookup.apply_mode = mode),
            I::LookupSetReplaceCover { replace } => {
                self.edit_lookup(|lookup| lookup.replace_cover = replace)
            }
        }
    }

    // ---- Titles ----

    /// A change to the titles that leaves the selection on the same titles.
    fn edit_titles(&self, change: impl FnOnce(&mut WorkingSet)) -> Rest {
        self.transition(|state| change(&mut state.working_set));
        Rest::Done(SessionOutcome::Applied)
    }

    /// A change that may move the selection: the draft gate accepts the edits
    /// on screen first, then the form binds to the new selection.
    fn change_selection(&self, change: impl FnOnce(&mut WorkingSet)) -> Rest {
        let bound = self.transition(|state| {
            state.gate()?;
            change(&mut state.working_set);
            Ok(Bound {
                reads: state.rebind(),
                binding: state.binding,
            })
        });
        match bound {
            Ok(bound) => Rest::Reads(bound),
            Err(GateBlock::SaveInProgress) => {
                Rest::Done(SessionOutcome::DraftRejected { message: None })
            }
            Err(GateBlock::Invalid(message)) => Rest::Done(SessionOutcome::DraftRejected {
                message: Some(message),
            }),
        }
    }

    /// Reads the tags a new selection asked for. The selection is published
    /// first so it shows while the reads run.
    async fn complete_reads(&self, bound: Bound) {
        if bound.reads.is_empty() {
            return;
        }
        self.publish();
        let reads = read_all(bound.reads).await;
        self.transition(|state| state.finish_reads(bound.binding, reads));
    }

    fn import_failed(&self, notice: InputNotice) -> SessionOutcome {
        self.transition(|state| state.working_set.set_notice(notice));
        SessionOutcome::Applied
    }

    fn begin_import(&self, paths: Vec<String>, default_audio: TitleAudioRequest) -> Rest {
        if self.lock().working_set.order_locked() {
            return Rest::Done(self.import_failed(InputNotice::OrderLocked));
        }
        Rest::Import {
            paths,
            default_audio,
            resets: self.inner.resets.load(Ordering::SeqCst),
        }
    }

    async fn import(
        &self,
        paths: Vec<String>,
        default_audio: TitleAudioRequest,
        resets: u64,
    ) -> SessionOutcome {
        let _in_order = self.inner.imports.lock().await;

        let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let discovered = blocking(move || audio::discover_audio_import_paths(&inputs)).await;
        let discovered = match discovered {
            Ok(discovered) => discovered,
            Err(error) => {
                return self.import_failed(InputNotice::DiscoveryFailed {
                    error: AppErrorEnvelope::from(&error),
                })
            }
        };
        if discovered.is_empty() {
            return self.import_failed(InputNotice::NoSupportedFiles {
                formats_text: audio::supported_audio_import_metadata().formats_text,
            });
        }
        let analyzed = match blocking(move || audio::get_file_list_info(&discovered)).await {
            Ok(analyzed) => analyzed,
            Err(error) => {
                return self.import_failed(InputNotice::AnalysisFailed {
                    error: AppErrorEnvelope::from(&error),
                })
            }
        };

        if self.inner.resets.load(Ordering::SeqCst) != resets {
            return SessionOutcome::Superseded;
        }
        let bound = self.transition(|state| {
            state
                .working_set
                .append_analyzed(analyzed.files, &default_audio);
            Bound {
                reads: state.rebind(),
                binding: state.binding,
            }
        });
        self.complete_reads(bound).await;
        SessionOutcome::Applied
    }

    // ---- Cover ----

    fn begin_cover_load(&self, source: CoverSource) -> Rest {
        let from_url = matches!(source, CoverSource::Url(_));
        if let CoverSource::Url(url) = &source {
            if url.trim().is_empty() {
                self.transition(SessionState::cover_url_required);
                return Rest::Done(SessionOutcome::CoverLoadFailed);
            }
        }
        let started = self.transition(|state| {
            if from_url {
                state.cover_load_started();
            }
            (state.epoch, state.binding)
        });
        Rest::CoverLoad { source, started }
    }

    async fn load_cover(&self, source: CoverSource, started: (u64, u64)) -> SessionOutcome {
        let from_url = matches!(source, CoverSource::Url(_));
        if from_url {
            self.publish();
        }
        let result = match source {
            CoverSource::File(path) => crate::cover_source::load_cover_art_file(path).await,
            CoverSource::Url(url) => {
                (self.inner.network.cover_from_url)(url.trim().to_string()).await
            }
        };
        self.transition(|state| {
            // The image was chosen for a selection that is no longer bound.
            if (state.epoch, state.binding) != started {
                return SessionOutcome::Superseded;
            }
            let loaded = result.is_ok();
            state.cover_load_finished(from_url, result);
            if loaded {
                SessionOutcome::Applied
            } else {
                SessionOutcome::CoverLoadFailed
            }
        })
    }

    // ---- Save ----

    fn begin_save(&self) -> Rest {
        let previewing = self.inner.deps.previews.load(Ordering::SeqCst) > 0;
        let in_use = self.inner.deps.work.sources_in_use();
        // Source paths are canonical, so match either spelling of the root.
        let staged = &self.inner.deps.temporary_root;
        let staged_canonical = std::fs::canonicalize(staged).ok();
        let is_temporary = |path: &Path| {
            path.starts_with(staged)
                || staged_canonical
                    .as_ref()
                    .is_some_and(|root| path.starts_with(root))
        };
        let plan = self.transition(|state| {
            if state.working_set.files().is_empty() {
                return None;
            }
            if previewing {
                state.set_status(MetadataStatus::SaveBlockedByPreview);
                return None;
            }
            let plan = state.begin_save(&in_use, is_temporary)?;
            Some((state.epoch, plan))
        });
        match plan {
            Some((epoch, plan)) => Rest::Save { epoch, plan },
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    async fn save(&self, epoch: u64, plan: SavePlan) -> SessionOutcome {
        if plan.waiting > 0 {
            self.ensure_deferred_writer();
        }

        let mut saved = Vec::new();
        let mut status = MetadataStatus::SaveComplete {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            waiting: plan.waiting,
            held: plan.held,
        };
        if !plan.immediate.is_empty() {
            self.publish();
            match self.write(&plan.immediate).await {
                Ok(written) => {
                    status = MetadataStatus::SaveComplete {
                        succeeded: written.succeeded,
                        failed: written.failed,
                        cancelled: written.cancelled,
                        waiting: plan.waiting,
                        held: plan.held,
                    };
                    saved = plan
                        .immediate
                        .into_iter()
                        .zip(written.items)
                        .filter_map(|(item, written)| written.then_some(item))
                        .collect();
                }
                Err(AppError::Cancellation(_)) => status = MetadataStatus::SaveCancelled,
                Err(error) => {
                    log::error!("Failed to save metadata: {error}");
                    status = MetadataStatus::SaveFailed {
                        error: AppErrorEnvelope::from(&error),
                    };
                }
            }
        }
        self.transition(|state| state.finish_save(epoch, &saved, status));
        SessionOutcome::Applied
    }

    /// Writes `items` as one accepted operation. The result says, per item,
    /// whether the file now carries the edit.
    async fn write(&self, items: &[SaveItem]) -> Result<Written> {
        let deps = &self.inner.deps;
        let requests = items
            .iter()
            .map(|item| MetadataSaveRequest {
                file_path: item.path.to_string_lossy().into_owned(),
                metadata_patch: item.patch.clone(),
            })
            .collect();
        let result = save_metadata_batch(&deps.host, &deps.work, &deps.jobs, requests).await?;
        let mut written = Written {
            items: vec![false; items.len()],
            succeeded: result.summary.succeeded,
            failed: result.summary.failed,
            cancelled: result.summary.cancelled,
        };
        for entry in result.results {
            if let Some(slot) = written.items.get_mut(entry.input_index) {
                *slot = entry.status == MetadataSaveResultStatus::Success;
            }
        }
        Ok(written)
    }

    fn ensure_deferred_writer(&self) {
        if self
            .inner
            .deferred_writer_running
            .swap(true, Ordering::SeqCst)
        {
            return;
        }
        let session = self.clone();
        tokio::spawn(async move { session.write_deferred_when_free().await });
    }

    /// Writes each waiting edit once no accepted export reads its file.
    async fn write_deferred_when_free(&self) {
        let mut changes = self.inner.deps.work.subscribe_changes();
        loop {
            changes.borrow_and_update();
            let in_use = self.inner.deps.work.sources_in_use();
            let ready = {
                let mut state = self.lock();
                if !state.has_waiting_writes() {
                    // Cleared under the state lock, where a Save adds waiting
                    // writes, so a new one always finds a writer or starts one.
                    self.inner
                        .deferred_writer_running
                        .store(false, Ordering::SeqCst);
                    return;
                }
                state.take_ready_deferred(&in_use)
            };
            if ready.is_empty() {
                if changes.changed().await.is_err() {
                    return;
                }
                continue;
            }
            let written = self.write(&ready).await;
            self.transition(|state| {
                for (index, item) in ready.iter().enumerate() {
                    let written = written
                        .as_ref()
                        .is_ok_and(|written| written.items.get(index) == Some(&true));
                    state.finish_deferred(item, written);
                }
            });
            self.publish();
        }
    }

    // ---- Lookup ----

    fn edit_lookup(&self, change: impl FnOnce(&mut lookup::LookupState)) -> Rest {
        self.transition(|state| change(&mut state.lookup));
        Rest::Done(SessionOutcome::Applied)
    }

    /// Starts a lookup action; any earlier one still running is superseded.
    fn begin_lookup_action(&self) -> u64 {
        let mut state = self.lock();
        state.lookup.request += 1;
        state.lookup.request
    }

    /// Runs `change` if `request` is still the current lookup action.
    fn lookup_step<T>(
        &self,
        request: u64,
        change: impl FnOnce(&mut SessionState) -> T,
    ) -> Option<T> {
        self.transition(|state| (state.lookup.request == request).then(|| change(state)))
    }

    fn begin_lookup_open(&self) -> Rest {
        let request = self.begin_lookup_action();
        let has_titles = self.transition(|state| {
            let queue: Vec<QueuedTitle> = state
                .working_set
                .selected_files()
                .into_iter()
                .filter(|file| file.is_valid)
                .map(|file| QueuedTitle {
                    title_id: file.input_id.clone(),
                    path: file.path.clone(),
                })
                .collect();
            let first = queue
                .first()
                .and_then(|title| state.known_tags(&title.path));
            let has_titles = !queue.is_empty();
            state.lookup.open(queue, first.as_ref());
            has_titles
        });
        if has_titles {
            Rest::LookupSearch { request }
        } else {
            Rest::Done(SessionOutcome::Applied)
        }
    }

    async fn lookup_search(&self, request: u64, after: Option<QueueStep>) -> SessionOutcome {
        let search = self.lookup_step(request, |state| {
            let query = state.lookup.search_query();
            if query.is_empty() {
                state.lookup.status = Some(LookupStatus::QueryRequired);
                return None;
            }
            state.lookup.status = Some(LookupStatus::Searching);
            Some((query, state.lookup.source.sources()))
        });
        let Some(search) = search else {
            return SessionOutcome::Superseded;
        };
        let Some((query, sources)) = search else {
            return SessionOutcome::Applied;
        };
        self.publish();
        let response = (self.inner.network.search)(query, sources).await;
        let applied = self.lookup_step(request, |state| match response {
            Ok(response) => {
                state.lookup.status = Some(LookupStatus::Found {
                    count: response.results.len(),
                    partial: !response.diagnostics.is_empty(),
                    after,
                });
                state.lookup.results = response.results;
                state.lookup.has_searched = true;
            }
            Err(error) => {
                log::error!("Metadata lookup failed: {error}");
                state.lookup.reset_results();
                state.lookup.status = Some(LookupStatus::SearchFailed { after });
            }
        });
        outcome_of(applied)
    }

    /// Selects a queued title through the draft gate and waits for its tags.
    /// Returns whether the form is bound to it.
    async fn select_title(&self, title: &QueuedTitle) -> bool {
        let bound = self.transition(|state| {
            let index = state.working_set.index_of(&title.title_id)?;
            if state.working_set.files()[index].path != title.path {
                return None;
            }
            state.gate().ok()?;
            state
                .working_set
                .select_file(index, SelectionModifiers::default());
            Some(Bound {
                reads: state.rebind(),
                binding: state.binding,
            })
        });
        let Some(bound) = bound else {
            return false;
        };
        let binding = bound.binding;
        self.complete_reads(bound).await;
        self.lock().binding == binding
    }

    fn begin_lookup_apply(&self, index: usize) -> Rest {
        let request = self.begin_lookup_action();
        let chosen = self.transition(|state| {
            let result = state.lookup.results.get(index)?.clone();
            let Some(title) = state.lookup.current().cloned() else {
                state.lookup.status = Some(LookupStatus::NoTitleQueued);
                return None;
            };
            Some(ChosenResult {
                result,
                title,
                replace_cover: state.lookup.replace_cover,
                mode: state.lookup.apply_mode,
            })
        });
        match chosen {
            Some(chosen) => Rest::LookupApply {
                request,
                chosen: Box::new(chosen),
            },
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    async fn lookup_apply(&self, request: u64, chosen: ChosenResult) -> SessionOutcome {
        let ChosenResult {
            result,
            title,
            replace_cover,
            mode,
        } = chosen;

        let mut cover = None;
        let mut cover_failed = false;
        if let Some(url) = result.cover_url.clone().filter(|_| replace_cover) {
            match (self.inner.network.cover_from_url)(url).await {
                Ok(bytes) => cover = Some(bytes),
                Err(error) => {
                    log::warn!("Failed to load cover art from lookup: {error}");
                    cover_failed = true;
                }
            }
        }
        if self.lock().lookup.request != request {
            return SessionOutcome::Superseded;
        }
        let selected = self.select_title(&title).await;
        let metadata = lookup::result_metadata(&result);
        let applied = self.lookup_step(request, |state| {
            let applied = selected && state.apply_lookup(&title, &metadata, cover);
            state.lookup.status = Some(if applied {
                LookupStatus::Applied { cover_failed }
            } else {
                LookupStatus::ApplyRejected
            });
            applied
        });
        match applied {
            None => SessionOutcome::Superseded,
            Some(true) if mode == LookupApplyMode::Queue => {
                // Applied values are form edits; moving on passes the draft
                // gate, which validates and stages them like any other edit.
                let step = if cover_failed {
                    QueueStep::AppliedWithoutCover
                } else {
                    QueueStep::Applied
                };
                self.lookup_advance(request, step).await
            }
            Some(_) => SessionOutcome::Applied,
        }
    }

    async fn lookup_advance(&self, request: u64, step: QueueStep) -> SessionOutcome {
        let next = self.lookup_step(request, |state| {
            let lookup = &mut state.lookup;
            if lookup.queue.is_empty() {
                return None;
            }
            let next = lookup.index + 1;
            let Some(title) = lookup.queue.get(next).cloned() else {
                lookup.status = Some(LookupStatus::QueueComplete {
                    cover_failed: step == QueueStep::AppliedWithoutCover,
                });
                return None;
            };
            Some((next, title))
        });
        let Some(next) = next else {
            return SessionOutcome::Superseded;
        };
        let Some((next, title)) = next else {
            return SessionOutcome::Applied;
        };

        let selected = self.select_title(&title).await;
        let moved = self.lookup_step(request, |state| {
            if !selected {
                state.lookup.status = Some(LookupStatus::NextTitleRejected);
                return false;
            }
            let known = state.known_tags(&title.path);
            state.lookup.index = next;
            state.lookup.title_query = lookup::title_query(known.as_ref(), &title.path);
            state.lookup.author_query = lookup::author_query(known.as_ref());
            state.lookup.reset_results();
            true
        });
        match moved {
            None => SessionOutcome::Superseded,
            Some(false) => SessionOutcome::Applied,
            Some(true) => self.lookup_search(request, Some(step)).await,
        }
    }
}

fn outcome_of(applied: Option<()>) -> SessionOutcome {
    match applied {
        Some(()) => SessionOutcome::Applied,
        None => SessionOutcome::Superseded,
    }
}

enum CoverSource {
    File(String),
    Url(String),
}

/// Per submitted item, whether the file now carries the edit.
struct Written {
    items: Vec<bool>,
    succeeded: usize,
    failed: usize,
    cancelled: usize,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::General(format!("Background task failed: {error}")))?
}

fn read_tags(path: &Path) -> Result<AudiobookMetadata> {
    let validated = audio::validate_input_audio_path(path)?;
    crate::metadata::read_metadata(validated.to_string_lossy().as_ref())
}

async fn read_all(tickets: Vec<ReadTicket>) -> Vec<(ReadTicket, Result<AudiobookMetadata>)> {
    let limit = Arc::new(tokio::sync::Semaphore::new(READ_CONCURRENCY));
    let reads: Vec<_> = tickets
        .into_iter()
        .map(|ticket| {
            let limit = Arc::clone(&limit);
            tokio::spawn(async move {
                let _permit = limit.acquire_owned().await;
                let path = ticket.path.clone();
                let result = blocking(move || read_tags(&path)).await;
                (ticket, result)
            })
        })
        .collect();
    let mut finished = Vec::with_capacity(reads.len());
    for read in reads {
        match read.await {
            Ok(read) => finished.push(read),
            Err(error) => log::warn!("Metadata read task failed: {error}"),
        }
    }
    finished
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
