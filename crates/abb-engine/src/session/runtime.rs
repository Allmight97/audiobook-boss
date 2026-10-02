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

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use super::audio::AudioDefaults;
use super::audio_choice::AudioEdit;
use super::exports::{ExportLink, OutputEdit, OutputEdits};
use super::lookup::{
    self, LookupApplyMode, LookupSource, LookupStatus, QueueStep, QueuedTitle, RESULT_LIMIT,
};
use super::metadata_form::{FieldAction, MetadataField};
use super::output::OutputPlan;
use super::plans::PlanTicket;
use super::state::{GateBlock, MetadataStatus, SaveItem, SavePlan, SessionState, SessionUpdate};
use super::submission::{plan_verdict, Draft, PlanVerdict, SubmissionStatus, SubmitRefusal};
use super::tag_cache::ReadTicket;
use super::working_set::{CueChoice, InputNotice, MoveDirection, SelectionModifiers, WorkingSet};
use crate::app_settings::{PinnedDefaults, SettingsIntent, SettingsRuntime};
use crate::audio::{self, EncoderSettingsCapabilities};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::host::{EngineEvent, Host};
use crate::metadata::AudiobookMetadata;
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource, OnlineMetadataResult};
use crate::metadata_save::{save_metadata_batch, MetadataSaveRequest, MetadataSaveResultStatus};
use crate::opened_audio::OpenedAudioFileQueue;
use crate::output_artifact::CollisionPolicy;
use crate::output_artifact::NamingPreset;
use crate::processing::run::{
    preflight_payload, process_payload_with_options, ProcessingRunOptions,
};
use crate::processing::SupplementalProcessingAsset;
use crate::remote_source::{AcquisitionHandoff, AcquisitionJob, Handoff, HandoffRefusal};
use crate::work_runtime::WorkRuntime;
use crate::work_runtime::{OperationSnapshot, SubmitProcessingOperationRequest};
use crate::ManagedJobRegistry;

/// How many source files are read for tags at once.
const READ_CONCURRENCY: usize = 8;

/// How long template typing pauses before the template is recorded.
const TEMPLATE_PAUSE: std::time::Duration = std::time::Duration::from_millis(400);

fn remember_output(defaults: crate::app_settings::OutputDefaults) -> SettingsIntent {
    SettingsIntent::Remember {
        encoder_defaults: None,
        output_defaults: Some(defaults),
        default_acquisition_lane: None,
    }
}

/// Something the user asked the session to do.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionIntent {
    // ---- Titles ----
    /// Discovers and analyzes audio under `paths` and adds new titles, each
    /// starting from the default audio choice.
    Import {
        paths: Vec<String>,
    },
    /// Imports the files the operating system asked ABB to open.
    ImportOpened,
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
    /// Returns the session to empty.
    Reset,

    // ---- Export ----
    /// Exports every valid title.
    Submit,
    /// Renders the first `seconds` of each valid title, in the foreground.
    Preview {
        seconds: f64,
    },
    /// Continues a submission held for review with the user's choice.
    ChooseCollisionPolicy {
        policy: CollisionPolicy,
    },
    CancelCollisionReview,
    /// Restarts an exported title at the location a Save offered
    /// (`OutputSnapshot::restart_offers`): cancels it, removes its empty
    /// folders, and submits it again through collision review.
    #[serde(rename_all = "camelCase")]
    RestartTitle {
        title_id: String,
        #[specta(type = specta_typescript::Number)]
        revision: u64,
    },
    /// Keeps an exported title where it is; its export continues unchanged.
    #[serde(rename_all = "camelCase")]
    KeepTitleLocation {
        title_id: String,
        #[specta(type = specta_typescript::Number)]
        revision: u64,
    },

    // ---- Output ----
    /// Where exports are written; recorded in the settings.
    SetOutputDirectory {
        directory: String,
    },
    SetNamingPreset {
        preset: NamingPreset,
    },
    #[serde(rename_all = "camelCase")]
    SetIncludeYear {
        include_year: bool,
    },
    /// The custom naming template as typed; recorded once typing pauses.
    SetNamingTemplate {
        template: String,
    },

    // ---- Audio ----
    /// Edits the default audio choice new titles start from, and records it
    /// in the settings.
    SetDefaultAudio {
        edit: AudioEdit,
    },
    /// Edits the audio choice of each named title. Refused while the list is
    /// locked.
    #[serde(rename_all = "camelCase")]
    SetTitleAudio {
        title_ids: Vec<String>,
        edit: AudioEdit,
    },
    /// Gives each named title the default audio choice.
    #[serde(rename_all = "camelCase")]
    ApplyDefaultAudio {
        title_ids: Vec<String>,
    },

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
    /// Where audio and output defaults chosen in the session are recorded.
    pub(crate) settings: SettingsRuntime,
    /// The engine's background tasks; the session's run here.
    pub(crate) tasks: tokio_util::task::TaskTracker,
    /// Where processing keeps its working files.
    pub(crate) workspace_root: PathBuf,
    /// Removes an acquisition's staged download.
    pub(crate) remove_staged: RemoveStaged,
}

pub(crate) type RemoveStaged = Arc<dyn Fn(&str) -> Result<()> + Send + Sync>;

fn closing() -> SubmissionStatus {
    SubmissionStatus::Refused {
        reason: SubmitRefusal::Closing,
    }
}

fn failed(error: &AppError) -> SubmissionStatus {
    SubmissionStatus::Failed {
        error: AppErrorEnvelope::from(error),
    }
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
    /// Wakes the deferred writer when a submission frees its sources.
    sources_freed: tokio::sync::Notify,
    /// Wakes acquisition handoffs waiting for the list to unlock.
    list_unlocked: tokio::sync::Notify,
    /// A staged-download sweep is scheduled or running; one at a time.
    sweeping: AtomicBool,
    /// Cancels the running preview's titles.
    preview_cancels: Mutex<Vec<Arc<AtomicBool>>>,
    /// Advances with every output change; a delayed record checks it.
    output_edits: AtomicU64,
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
        resets: u64,
    },
    /// Record a choice in the settings, unless they were reset since `resets`.
    Remember {
        intent: SettingsIntent,
        resets: u64,
    },
    /// Preflight, review, then export or preview.
    Submit(Box<Draft>),
    /// Continue a reviewed submission under `policy`.
    Reviewed {
        draft: Box<Draft>,
        policy: CollisionPolicy,
    },
    /// Stop a title's export, then submit `draft` (that title alone).
    Restart {
        draft: Box<Draft>,
        link: Box<ExportLink>,
    },
    Save {
        epoch: u64,
        plan: SavePlan,
    },
    CoverLoad {
        source: CoverSource,
        started: (u64, u64, u64),
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
        let waited = !matches!(self.rest, Rest::Done(_));
        // Work shutdown must wait for: what reads or writes the user's files
        // or settings. Reads and lookups are dropped with the host.
        let tasks = session.inner.deps.tasks.clone();
        let outcome = match self.rest {
            Rest::Done(outcome) => outcome,
            Rest::Reads(bound) => {
                session.complete_reads(bound).await;
                SessionOutcome::Applied
            }
            Rest::Import { paths, resets } => session.import(paths, resets).await,
            Rest::Remember { intent, resets } => {
                tasks.track_future(session.remember(intent, resets)).await;
                SessionOutcome::Applied
            }
            Rest::Submit(draft) => tasks.track_future(session.submit(*draft)).await,
            Rest::Reviewed { mut draft, policy } => {
                draft.payload.collision_policy = Some(policy);
                tasks.track_future(session.submit(*draft)).await
            }
            Rest::Save { epoch, plan } => tasks.track_future(session.save(epoch, plan)).await,
            Rest::Restart { draft, link } => {
                tasks.track_future(session.restart(*draft, *link)).await
            }
            Rest::CoverLoad { source, started } => session.load_cover(source, started).await,
            Rest::LookupSearch { request } => session.lookup_search(request, None).await,
            Rest::LookupApply { request, chosen } => session.lookup_apply(request, *chosen).await,
            Rest::LookupAdvance { request, step } => session.lookup_advance(request, step).await,
        };
        // Other hosts, and a frontend that attached while this ran, learn
        // the result here; the caller also gets it in the reply.
        if waited {
            session.publish();
        }
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
                sources_freed: tokio::sync::Notify::new(),
                list_unlocked: tokio::sync::Notify::new(),
                sweeping: AtomicBool::new(false),
                preview_cancels: Mutex::default(),
                output_edits: AtomicU64::new(0),
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

    /// Runs one atomic transition and re-derives the snapshots. Title plans
    /// the transition made stale are resolved in the background, and a
    /// download the transition left removable is removed.
    fn transition<T>(&self, change: impl FnOnce(&mut SessionState) -> T) -> T {
        let (value, tickets, sweep) = {
            let mut state = self.lock();
            let value = change(&mut state);
            let tickets = state.take_plan_tickets();
            state.settle();
            (value, tickets, self.sweep_due(&state))
        };
        if !tickets.is_empty() {
            self.resolve_plans(tickets);
        }
        if sweep {
            self.sweep_staged();
        }
        value
    }

    /// Whether a download may be removed now and no sweep is on its way.
    /// Every change that frees a download is a transition, so this is the
    /// one place a sweep starts.
    fn sweep_due(&self, state: &SessionState) -> bool {
        !state.staged.is_empty()
            && !self.inner.sweeping.load(Ordering::SeqCst)
            && !state
                .removable_staged(&self.inner.deps.work.sources_held(), now())
                .is_empty()
    }

    /// Removes the downloads no title or export needs any more, until none
    /// is left to remove.
    fn sweep_staged(&self) {
        if tokio::runtime::Handle::try_current().is_err()
            || self.inner.sweeping.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let session = self.clone();
        self.inner.deps.tasks.spawn(async move {
            loop {
                // Read under the session lock, so no submission can reserve a
                // file between this check and its removal. The flag clears
                // under the same lock, so a transition that frees a download
                // afterwards starts the next sweep.
                let jobs = session.transition(|state| {
                    let held = session.inner.deps.work.sources_held();
                    let jobs = state.begin_staged_removal(&held, now());
                    if jobs.is_empty() {
                        session.inner.sweeping.store(false, Ordering::SeqCst);
                    }
                    jobs
                });
                if jobs.is_empty() {
                    return;
                }
                for job_id in jobs {
                    let remove = Arc::clone(&session.inner.deps.remove_staged);
                    let id = job_id.clone();
                    let removed = blocking(move || remove(&id)).await;
                    if let Err(error) = &removed {
                        log::warn!("Failed to remove staged download job_id={job_id}: {error}");
                    }
                    session.transition(|state| {
                        state.finish_staged_removal(&job_id, removed.is_ok(), now());
                    });
                }
                session.publish();
            }
        });
    }

    /// Records which titles an export finished.
    fn export_finished(&self, snapshot: &OperationSnapshot) {
        self.transition(|state| state.staged.finish_export(&snapshot.children));
    }

    fn resolve_plans(&self, tickets: Vec<PlanTicket>) {
        // Without a runtime (engine construction) plans stay pending until
        // the next change.
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let session = self.clone();
        self.inner.deps.tasks.spawn(async move {
            for ticket in tickets {
                let resolving = ticket.clone();
                let result = tokio::task::spawn_blocking(move || resolving.resolve())
                    .await
                    .unwrap_or_else(|error| Err(error.to_string().into()));
                session.transition(|state| state.finish_plan(&ticket, result));
            }
            session.publish();
        });
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

    /// Sets the defaults new titles start from and the output choices, at
    /// engine start.
    pub(crate) fn start_from_defaults(
        &self,
        defaults: Option<&PinnedDefaults>,
        caps: Option<EncoderSettingsCapabilities>,
    ) {
        self.transition(|state| {
            state.audio =
                AudioDefaults::new(defaults.map(|defaults| &defaults.encoder_defaults), caps);
            if let Some(defaults) = defaults {
                state.output = OutputPlan::from_defaults(&defaults.output_defaults);
            }
        });
    }

    /// Replaces the defaults and output choices after the settings changed
    /// them. Loaded titles keep their own audio choices.
    pub(crate) fn replace_defaults(&self, defaults: &PinnedDefaults) {
        self.transition(|state| {
            state.audio.replace(&defaults.encoder_defaults);
            state.output = OutputPlan::from_defaults(&defaults.output_defaults);
        });
        self.publish();
    }

    pub(crate) fn snapshot(&self) -> SessionUpdate {
        self.lock().update_since(None)
    }

    pub(crate) fn cover_art(&self) -> Option<Vec<u8>> {
        self.lock().displayed_cover()
    }

    /// Source files with a Save accepted and not yet written.
    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        self.lock().waiting_write_paths()
    }

    #[allow(clippy::too_many_lines)] // one arm per intent; each arm is a call
    fn begin_rest(&self, intent: SessionIntent) -> Rest {
        use SessionIntent as I;
        match intent {
            I::Import { paths } => self.begin_import(paths),
            // Opened files stay queued while the list is locked, for a retry.
            I::ImportOpened if self.lock().working_set.order_locked() => {
                Rest::Done(self.import_failed(InputNotice::OrderLocked))
            }
            I::ImportOpened => match self.inner.deps.opened_audio.take_paths() {
                Ok(paths) if paths.is_empty() => Rest::Done(SessionOutcome::Applied),
                Ok(paths) => self.begin_import(paths),
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
            I::ReorderSources { title_id, from, to } => {
                self.edit_titles(|set| set.reorder_sources(&title_id, from, to))
            }
            I::ChooseCue { input_id, choice } => {
                self.edit_titles(|set| set.choose_cue(&input_id, choice))
            }
            I::SetOutputDirectory { directory } => {
                self.edit_output(|output| output.set_directory(directory))
            }
            I::SetNamingPreset { preset } => self.edit_output(|output| output.set_preset(preset)),
            I::SetIncludeYear { include_year } => {
                self.edit_output(|output| output.set_include_year(include_year))
            }
            I::SetNamingTemplate { template } => {
                self.transition(|state| state.output.set_template(template));
                self.remember_output_later();
                Rest::Done(SessionOutcome::Applied)
            }
            I::SetDefaultAudio { edit } => match self.transition(|state| state.audio.edit(edit)) {
                Some(defaults) => self.remember_later(SettingsIntent::Remember {
                    encoder_defaults: Some(defaults),
                    output_defaults: None,
                    default_acquisition_lane: None,
                }),
                None => Rest::Done(SessionOutcome::Applied),
            },
            I::SetTitleAudio { title_ids, edit } => {
                self.transition(|state| state.edit_title_audio(&title_ids, edit));
                Rest::Done(SessionOutcome::Applied)
            }
            I::ApplyDefaultAudio { title_ids } => {
                self.transition(|state| state.apply_default_audio(&title_ids));
                Rest::Done(SessionOutcome::Applied)
            }
            I::Reset => {
                self.inner.resets.fetch_add(1, Ordering::SeqCst);
                self.transition(SessionState::reset);
                // A review the reset dropped frees its sources.
                self.sources_released();
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
            I::Save => self.begin_save(),
            I::Submit => self.begin_submission(None),
            I::Preview { seconds } => self.begin_submission(Some(seconds)),
            I::ChooseCollisionPolicy { policy } => match self.transition(SessionState::take_review)
            {
                Some(draft) => Rest::Reviewed {
                    draft: Box::new(draft),
                    policy,
                },
                None => Rest::Done(SessionOutcome::Applied),
            },
            I::CancelCollisionReview => {
                self.cancel_review();
                Rest::Done(SessionOutcome::Applied)
            }
            I::RestartTitle { title_id, revision } => self.begin_restart(&title_id, revision),
            I::KeepTitleLocation { title_id, revision } => {
                self.transition(|state| state.keep_location(&title_id, revision));
                Rest::Done(SessionOutcome::Applied)
            }

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

    fn begin_import(&self, paths: Vec<String>) -> Rest {
        if self.lock().working_set.order_locked() {
            return Rest::Done(self.import_failed(InputNotice::OrderLocked));
        }
        Rest::Import {
            paths,
            resets: self.inner.resets.load(Ordering::SeqCst),
        }
    }

    async fn import(&self, paths: Vec<String>, resets: u64) -> SessionOutcome {
        let _in_order = self.inner.imports.lock().await;
        // A Reset since this import began drops it, success or failure.
        let superseded = || self.inner.resets.load(Ordering::SeqCst) != resets;
        let failed = |notice| {
            if superseded() {
                SessionOutcome::Superseded
            } else {
                self.import_failed(notice)
            }
        };

        let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let discovered = blocking(move || audio::discover_audio_import_paths(&inputs)).await;
        let discovered = match discovered {
            Ok(discovered) => discovered,
            Err(error) => {
                return failed(InputNotice::DiscoveryFailed {
                    error: AppErrorEnvelope::from(&error),
                })
            }
        };
        if discovered.is_empty() {
            return failed(InputNotice::NoSupportedFiles {
                formats_text: audio::supported_audio_import_metadata().formats_text,
            });
        }
        let analyzed = match blocking(move || audio::get_file_list_info(&discovered)).await {
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
        let bound = self.transition(|state| {
            let default_audio = state.audio.request();
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

    /// Imports a finished acquisition's files and records them as staged
    /// downloads, in the transition that lists them, so no Reset or removal
    /// can fall between the two.
    async fn import_acquired(&self, job: AcquisitionJob) -> AcquisitionHandoff {
        let handoff = self.import_acquired_files(&job).await;
        if matches!(handoff, AcquisitionHandoff::Removed { .. }) {
            // Nothing from it is listed; the staged record owns removing the
            // download, and retries if removal fails.
            let paths = job
                .materialized_files
                .iter()
                .map(|file| file.path.clone())
                .chain(
                    job.supplemental_assets
                        .iter()
                        .map(|asset| asset.path.clone()),
                )
                .collect();
            self.transition(|state| state.staged.register_unimported(&job.job_id, paths));
        }
        handoff
    }

    async fn import_acquired_files(&self, job: &AcquisitionJob) -> AcquisitionHandoff {
        let _in_order = self.inner.imports.lock().await;
        let resets = self.inner.resets.load(Ordering::SeqCst);
        let removed = |reason| AcquisitionHandoff::Removed { reason };
        let paths: Vec<PathBuf> = job
            .materialized_files
            .iter()
            .map(|file| file.path.clone())
            .collect();
        let analyzed = blocking(move || {
            let discovered = audio::discover_audio_import_paths(&paths)?;
            audio::get_file_list_info(&discovered)
        })
        .await;
        let analyzed = match analyzed {
            Ok(analyzed) => analyzed.files,
            Err(error) => {
                return removed(HandoffRefusal::ImportFailed {
                    error: AppErrorEnvelope::from(&error),
                })
            }
        };
        let titles = staged_titles(job);
        // A list locked by an export being prepared or reviewed unlocks when
        // it ends; the download waits for that rather than being dropped.
        let listed = loop {
            let unlocked = self.inner.list_unlocked.notified();
            tokio::pin!(unlocked);
            unlocked.as_mut().enable();
            if self.inner.resets.load(Ordering::SeqCst) != resets {
                return removed(HandoffRefusal::NothingAdded);
            }
            let attempt = self.transition(|state| {
                if state.working_set.order_locked() {
                    return None;
                }
                Some(list_acquired(state, job, analyzed.clone(), &titles))
            });
            match attempt {
                Some(listed) => break listed,
                None => unlocked.await,
            }
        };
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

    /// Hands finished acquisitions to this session. Holds the session weakly,
    /// since the remote-source runtime outlives no engine.
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

    // ---- Export ----

    fn begin_submission(&self, preview_seconds: Option<f64>) -> Rest {
        let closing = self.inner.deps.tasks.is_closed();
        let draft = self.transition(|state| {
            if closing {
                state.refuse_submission(SubmitRefusal::Closing);
                return None;
            }
            state.begin_submission(preview_seconds)
        });
        match draft {
            Some(draft) => Rest::Submit(Box::new(draft)),
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    /// Preflights a draft, holds it for review when outputs collide, then
    /// exports or previews it.
    async fn submit(&self, draft: Draft) -> SessionOutcome {
        self.publish();
        let checking = draft.clone();
        let plan = blocking(move || {
            preflight_payload(
                checking.payload,
                checking.metadata,
                checking.preview_seconds,
            )
        })
        .await;
        let plan = match plan {
            Ok(plan) => plan,
            Err(error) => return self.end_submission(&draft, failed(&error)),
        };
        // A policy applies only to collisions the user reviewed; a new one
        // that appeared meanwhile sends the submission back to review.
        let collided: Vec<_> = plan
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
        match plan_verdict(&plan) {
            PlanVerdict::Blocked(message) => {
                self.end_submission(&draft, SubmissionStatus::Blocked { message })
            }
            PlanVerdict::Review(outputs) if draft.payload.collision_policy.is_none() => {
                self.hold_for_review(draft, outputs)
            }
            _ if unreviewed => self.hold_for_review(draft, collided),
            PlanVerdict::Review(_) | PlanVerdict::Proceed => {
                self.accept(draft.approved(plan.collision_policy, plan.plan_signature))
                    .await
            }
        }
    }

    /// Holds a draft for the user's collision choice, unless the engine is
    /// closing: checked under the session lock, so shutdown's cancel of
    /// reviews either finds this one or this sees shutdown.
    fn hold_for_review(
        &self,
        draft: Draft,
        outputs: Vec<crate::output_artifact::PlannedOutput>,
    ) -> SessionOutcome {
        let closing_now = self.transition(|state| {
            if self.inner.deps.tasks.is_closed() {
                state.finish_submission(&draft, closing());
                return true;
            }
            state.await_review(draft, outputs);
            false
        });
        if closing_now {
            self.sources_released();
        }
        SessionOutcome::Applied
    }

    async fn accept(&self, draft: Draft) -> SessionOutcome {
        let deps = &self.inner.deps;
        if let Some(seconds) = draft.preview_seconds {
            let cancels: Vec<Arc<AtomicBool>> = (0..draft.payload.input_files.len())
                .map(|_| Arc::default())
                .collect();
            *self.preview_cancels() = cancels.clone();
            // Checked after the flags are visible, so shutdown either sees
            // this preview to cancel or the preview sees shutdown.
            if deps.tasks.is_closed() {
                return self.end_submission(&draft, closing());
            }
            self.transition(SessionState::start_preview);
            self.publish();
            let result = process_payload_with_options(
                deps.host.clone(),
                deps.jobs.clone(),
                deps.workspace_root.clone(),
                draft.payload.clone(),
                draft.metadata.clone(),
                Some(seconds),
                ProcessingRunOptions {
                    title_cancels: cancels,
                    ..ProcessingRunOptions::default()
                },
            )
            .await;
            self.preview_cancels().clear();
            let status = match result {
                Ok(result) => SubmissionStatus::PreviewFinished { result },
                Err(error) => failed(&error),
            };
            return self.end_submission(&draft, status);
        }
        let submitted = deps
            .work
            .submit_processing_operation(
                deps.host.clone(),
                deps.jobs.clone(),
                deps.workspace_root.clone(),
                SubmitProcessingOperationRequest {
                    payload: draft.payload.clone(),
                    metadata: draft.metadata.clone(),
                    preview_seconds: None,
                    title: draft.title.clone(),
                },
                Some(Box::new({
                    let session = self.clone();
                    move |snapshot| session.export_finished(&snapshot)
                })),
            )
            .await;
        let status = match submitted {
            Ok(accepted) => {
                self.transition(|state| {
                    state.link_exports(&draft, &accepted.operation_id, &accepted.titles);
                });
                SubmissionStatus::Submitted {
                    operation_id: accepted.operation_id,
                    title: draft.title.clone(),
                }
            }
            Err(error) => failed(&error),
        };
        self.end_submission(&draft, status)
    }

    fn preview_cancels(&self) -> std::sync::MutexGuard<'_, Vec<Arc<AtomicBool>>> {
        self.inner
            .preview_cancels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Stops a running preview; shutdown does not wait out a scratch render.
    pub(crate) fn cancel_preview(&self) {
        for cancel in self.preview_cancels().iter() {
            cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Drops a submission held for review and frees its sources, waking a
    /// Save that waits on them. Shutdown calls this too: a review nobody
    /// answers would otherwise hold the files forever.
    pub(crate) fn cancel_review(&self) {
        self.transition(SessionState::cancel_review);
        self.sources_released();
    }

    /// A submission ended and freed its sources and the list: wakes a Save
    /// and an acquisition handoff waiting on them.
    fn sources_released(&self) {
        self.inner.sources_freed.notify_one();
        self.inner.list_unlocked.notify_waiters();
    }

    /// Ends a submission; its sources stay reserved until WorkRuntime has
    /// registered them.
    fn end_submission(&self, draft: &Draft, status: SubmissionStatus) -> SessionOutcome {
        self.transition(|state| state.finish_submission(draft, status));
        self.sources_released();
        SessionOutcome::Applied
    }

    /// Changes the output choices and records them.
    fn edit_output(&self, change: impl FnOnce(&mut OutputPlan)) -> Rest {
        let defaults = self.transition(|state| {
            change(&mut state.output);
            state.output.defaults()
        });
        self.inner.output_edits.fetch_add(1, Ordering::SeqCst);
        self.remember_later(remember_output(defaults))
    }

    /// Records a choice once the intent's immediate effect is done. The
    /// reset count is read now, with the choice, so a settings reset that
    /// lands first wins.
    fn remember_later(&self, intent: SettingsIntent) -> Rest {
        Rest::Remember {
            intent,
            resets: self.inner.deps.settings.resets(),
        }
    }

    /// Records the output choices once template typing has paused, so a
    /// keystroke does not write the settings file.
    fn remember_output_later(&self) {
        let edit = self.inner.output_edits.fetch_add(1, Ordering::SeqCst) + 1;
        let resets = self.inner.deps.settings.resets();
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let session = self.clone();
        self.inner.deps.tasks.spawn(async move {
            tokio::time::sleep(TEMPLATE_PAUSE).await;
            if session.inner.output_edits.load(Ordering::SeqCst) != edit {
                return;
            }
            let defaults = session.lock().output.defaults();
            session.remember(remember_output(defaults), resets).await;
        });
    }

    /// Records a choice in the settings and tells hosts what the settings
    /// are now. A choice made before a reset that already applied is dropped.
    async fn remember(&self, intent: SettingsIntent, resets: u64) {
        let settings = &self.inner.deps.settings;
        let Some(reply) = settings.dispatch_unless_reset(intent, resets).await else {
            return;
        };
        self.inner
            .deps
            .host
            .emit(EngineEvent::Settings(Box::new(reply.snapshot)));
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
            let request = state.begin_cover_request(from_url);
            (state.epoch, state.binding, request)
        });
        Rest::CoverLoad { source, started }
    }

    async fn load_cover(&self, source: CoverSource, started: (u64, u64, u64)) -> SessionOutcome {
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
            // The image was chosen for a selection that is no longer bound,
            // or a later choice or Clear replaced it.
            if (state.epoch, state.binding, state.cover_request()) != started {
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
            // Read under the session lock, so a submission cannot register its
            // sources between this read and the save plan.
            let in_use = self.inner.deps.work.sources_in_use();
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
        let outputs = self.update_outputs(plan.outputs);

        let mut saved = Vec::new();
        let mut status = MetadataStatus::SaveComplete {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            waiting: plan.waiting,
            held: plan.held,
            outputs,
        };
        let nothing = plan.immediate.is_empty()
            && plan.waiting == 0
            && plan.held == 0
            && outputs == OutputEdits::default();
        if nothing {
            status = if plan.grouped {
                MetadataStatus::GroupedEditsKept
            } else {
                MetadataStatus::NoPendingChanges
            };
        }
        let written_paths: Vec<PathBuf> = plan
            .immediate
            .iter()
            .map(|item| item.path.clone())
            .collect();
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
                        outputs,
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
        self.transition(|state| state.finish_save(epoch, &written_paths, &saved, status));
        SessionOutcome::Applied
    }

    /// Sends each exported title's edit to its output, then records restart
    /// offers and ended links.
    fn update_outputs(&self, edits: Vec<OutputEdit>) -> OutputEdits {
        if edits.is_empty() {
            return OutputEdits::default();
        }
        let work = &self.inner.deps.work;
        let replies = edits
            .into_iter()
            .map(|edit| {
                let reply = work
                    .update_title_output(&edit.title, edit.revision, &edit.intent)
                    .map_err(|error| error.to_string());
                (edit, reply)
            })
            .collect();
        self.transition(|state| state.record_output_edits(replies))
    }

    fn begin_restart(&self, title_id: &str, revision: u64) -> Rest {
        let closing = self.inner.deps.tasks.is_closed();
        let started = self.transition(|state| {
            if closing {
                state.refuse_submission(SubmitRefusal::Closing);
                return None;
            }
            state.begin_restart(title_id, revision)
        });
        match started {
            Some((draft, link)) => Rest::Restart {
                draft: Box::new(draft),
                link: Box::new(link),
            },
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    /// Stops the title's export, then submits it again from the session. A
    /// title that published first keeps its output, which takes the edit.
    async fn restart(&self, draft: Draft, link: ExportLink) -> SessionOutcome {
        self.publish();
        let deps = &self.inner.deps;
        let published = deps
            .work
            .stop_title(&deps.host, &link.operation_id, link.index, &link.title)
            .await;
        if !published {
            return self.submit(draft).await;
        }
        let outcome = self.end_submission(&draft, SubmissionStatus::FinishedBeforeRestart);
        let edit = self.transition(|state| {
            let tags = &state.tags;
            state
                .exports
                .edits(|path| tags.pending(path).map(|pending| pending.patch.clone()))
                .into_iter()
                .find(|edit| edit.title_id == draft_title_id(&draft))
        });
        if let Some(edit) = edit {
            self.update_outputs(vec![edit]);
        }
        outcome
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
        self.inner
            .deps
            .tasks
            .spawn(async move { session.write_deferred_when_free().await });
    }

    /// Writes each waiting edit once no accepted export, preview, or
    /// submission being prepared reads its file.
    async fn write_deferred_when_free(&self) {
        let mut changes = self.inner.deps.work.subscribe_changes();
        loop {
            changes.borrow_and_update();
            let ready = {
                let mut state = self.lock();
                let in_use = self.inner.deps.work.sources_in_use();
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
                tokio::select! {
                    changed = changes.changed() => {
                        if changed.is_err() {
                            return;
                        }
                    }
                    () = self.inner.sources_freed.notified() => {}
                }
                continue;
            }
            let written = self.write(&ready).await;
            let results: Vec<(SaveItem, bool)> = ready
                .into_iter()
                .enumerate()
                .map(|(index, item)| {
                    let ok = written
                        .as_ref()
                        .is_ok_and(|written| written.items.get(index) == Some(&true));
                    (item, ok)
                })
                .collect();
            self.transition(|state| state.finish_deferred(&results));
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

/// Lists an acquisition's analyzed files and records each imported title as
/// staged, in the same transition.
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
    Ok((
        Bound {
            reads: state.rebind(),
            binding: state.binding,
        },
        count,
    ))
}

/// Each materialized title's id and canonical path.
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
/// The title a one-title draft submits.
fn draft_title_id(draft: &Draft) -> String {
    draft
        .payload
        .input_ids
        .as_ref()
        .and_then(|ids| ids.first().cloned().flatten())
        .unwrap_or_default()
}

fn now() -> std::time::Instant {
    tokio::time::Instant::now().into_std()
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
