use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lookup_flow::ChosenResult;

use super::exports::{ExportLink, OutputEdit, OutputEdits};
use super::intent::{SessionIntent, SessionOutcome, SessionReply};
use super::lookup::{QueueStep, RESULT_LIMIT};
use super::plans::PlanTicket;
use super::state::{GateBlock, MetadataStatus, SaveItem, SavePlan, SessionState, SessionUpdate};
use super::submission::{Draft, SubmitRefusal};
use super::tag_cache::ReadTicket;
use super::working_set::{InputNotice, SelectionModifiers, WorkingSet};
use crate::app_settings::{SettingsIntent, SettingsRun, SettingsRuntime};
use crate::audio::{self, AudioFile};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::host::{EngineEvent, Host};
use crate::metadata::AudiobookMetadata;
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource};
use crate::output_artifact::CollisionPolicy;
use crate::processing::title_output::UpdateReply;
use crate::processing::SupplementalProcessingAsset;
use crate::processing::{OutputUpdate, OutputUpdateStatus};
use crate::remote_source::{
    AcquisitionHandoff, AcquisitionJob, Handoff, HandoffRefusal, RemoteRevisions, RemoteUiResult,
};
use crate::work_runtime::OperationSnapshot;
use crate::work_runtime::WorkRuntime;
use crate::ManagedJobRegistry;

const READ_CONCURRENCY: usize = 8;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

pub(crate) struct Network {
    pub(crate) search: Box<SearchFn>,
    pub(crate) covers: crate::cover_service::CoverService,
}

type SearchFn =
    dyn Fn(String, Vec<MetadataSource>) -> BoxFuture<Result<MetadataLookupResponse>> + Send + Sync;

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
            covers: crate::cover_service::CoverService::live(),
        }
    }
}

pub(crate) struct SessionDeps {
    pub(crate) host: Host,
    pub(crate) remote: crate::remote_source::RemoteSourceRuntime,
    pub(crate) work: WorkRuntime,
    pub(crate) jobs: ManagedJobRegistry,
    /// Source files under this root are temporary downloads.
    pub(crate) temporary_root: PathBuf,
    pub(crate) settings: SettingsRuntime,
    pub(crate) tasks: crate::engine::EngineTasks,
    pub(crate) workspace_root: PathBuf,
    pub(crate) remove_staged: RemoveStaged,
}

pub(crate) type RemoveStaged = Arc<dyn Fn(&str) -> Result<()> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Session {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    state: Mutex<SessionState>,
    deps: SessionDeps,
    network: Network,
    imports: super::import_order::ImportOrder,
    resets: AtomicU64,
    /// The revision the last event carried.
    published: AtomicU64,
    deferred_writer_running: AtomicBool,
    sources_freed: tokio::sync::Notify,
    list_unlocked: tokio::sync::Notify,
    sweeping: AtomicBool,
    preview_cancels: Mutex<Vec<Arc<AtomicBool>>>,
    /// Advances with every output change; a delayed record checks it.
    output_edits: Arc<AtomicU64>,
    audio_edits: AtomicU64,
    settings_applied: AtomicU64,
}

struct Bound {
    binding: u64,
    reads: Vec<ReadTicket>,
}

/// A disposable reply to work the engine has already accepted and started.
pub struct SessionRun {
    session: Session,
    since: u64,
    remote_since: RemoteRevisions,
    reply: tokio::sync::oneshot::Receiver<SessionOutcome>,
}

enum Rest {
    Remote(crate::remote_source::RemoteUiRun),
    Done(SessionOutcome),
    Reads(Bound),
    Import {
        paths: Vec<String>,
        resets: u64,
        turn: super::import_order::Turn,
    },
    Remember(SettingsRun),
    Submit {
        draft: Box<Draft>,
        resets: u64,
    },
    Reviewed {
        draft: Box<Draft>,
        policy: CollisionPolicy,
        resets: u64,
    },
    Restart {
        draft: Box<Draft>,
        link: Box<ExportLink>,
        resets: u64,
    },
    Save {
        epoch: u64,
        plan: SavePlan,
    },
    KeepLocation {
        title_id: String,
        revision: u64,
        link: Box<ExportLink>,
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

impl SessionRun {
    /// Waits for accepted work; dropping this wait does not cancel the work.
    pub async fn finish(self) -> SessionReply {
        let session = self.session;
        let outcome = self
            .reply
            .await
            .unwrap_or_else(|error| SessionOutcome::Rejected {
                error: AppErrorEnvelope::from(&AppError::General(format!(
                    "Session work failed: {error}"
                ))),
            });
        let mut update = session.lock().update_since(Some(self.since));
        session
            .inner
            .deps
            .remote
            .add_ui_parts(&mut update, Some(self.remote_since));
        SessionReply { outcome, update }
    }
}

impl Session {
    async fn complete(&self, rest: Rest) -> SessionOutcome {
        let session = self;
        let outcome = match rest {
            Rest::Remote(run) => match run.finish().await {
                Ok(RemoteUiResult::Applied) => SessionOutcome::Applied,
                Ok(RemoteUiResult::Saved) => SessionOutcome::RemoteSaved,
                Ok(RemoteUiResult::Superseded) => SessionOutcome::Superseded,
                Ok(RemoteUiResult::AuthStarted(authorization)) => {
                    SessionOutcome::RemoteAuthStarted { authorization }
                }
                Err(error) => SessionOutcome::Rejected {
                    error: (&error).into(),
                },
            },
            Rest::Done(outcome) => outcome,
            Rest::Reads(bound) => {
                session.complete_reads(bound).await;
                SessionOutcome::Applied
            }
            Rest::Import {
                paths,
                resets,
                turn,
            } => session.import(paths, resets, turn).await,
            Rest::Remember(run) => {
                let reply = run.finish().await;
                session
                    .inner
                    .deps
                    .host
                    .emit(EngineEvent::Settings(Box::new(reply.snapshot)));
                match reply.outcome {
                    crate::app_settings::SettingsOutcome::Rejected { error } => {
                        SessionOutcome::Rejected { error }
                    }
                    _ => SessionOutcome::Applied,
                }
            }
            Rest::Submit { draft, resets } => session.submit(*draft, resets).await,
            Rest::Reviewed {
                mut draft,
                policy,
                resets,
            } => {
                draft.payload.collision_policy = Some(policy);
                session.submit(*draft, resets).await
            }
            Rest::Save { epoch, plan } => session.save(epoch, plan).await,
            Rest::Restart {
                draft,
                link,
                resets,
            } => session.restart(*draft, *link, resets).await,
            Rest::KeepLocation {
                title_id,
                revision,
                link,
            } => {
                let outcome = session.keep_location(title_id, revision, *link).await;
                session.transition(SessionState::finish_keep_location);
                outcome
            }
            Rest::CoverLoad { source, started } => session.load_cover(source, started).await,
            Rest::LookupSearch { request } => session.lookup_search(request, None).await,
            Rest::LookupApply { request, chosen } => session.lookup_apply(request, *chosen).await,
            Rest::LookupAdvance { request, step } => session.lookup_advance(request, step).await,
        };
        session.publish();
        outcome
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
                imports: super::import_order::ImportOrder::default(),
                resets: AtomicU64::new(0),
                published: AtomicU64::new(0),
                deferred_writer_running: AtomicBool::new(false),
                sources_freed: tokio::sync::Notify::new(),
                list_unlocked: tokio::sync::Notify::new(),
                sweeping: AtomicBool::new(false),
                preview_cancels: Mutex::default(),
                output_edits: Arc::new(AtomicU64::new(0)),
                audio_edits: AtomicU64::new(0),
                settings_applied: AtomicU64::new(0),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

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

    fn sweep_due(&self, state: &SessionState) -> bool {
        !state.staged.is_empty()
            && !self.inner.sweeping.load(Ordering::SeqCst)
            && !state
                .removable_staged(&self.inner.deps.work.sources_held(), now())
                .is_empty()
    }

    fn sweep_staged(&self) {
        if tokio::runtime::Handle::try_current().is_err()
            || self.inner.sweeping.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let session = self.clone();
        self.inner.deps.tasks.spawn(async move {
            loop {
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
                    let bound = session.transition(|state| {
                        let reads = state.finish_staged_removal(&job_id, removed.is_ok(), now());
                        Bound {
                            reads,
                            binding: state.binding,
                        }
                    });
                    session.complete_reads(bound).await;
                }
                session.publish();
            }
        });
    }

    fn export_finished(&self, snapshot: &OperationSnapshot) {
        self.transition(|state| state.staged.finish_export(&snapshot.children));
    }

    fn resolve_plans(&self, tickets: Vec<PlanTicket>) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let session = self.clone();
        self.inner.deps.tasks.spawn(async move {
            for ticket in tickets {
                let resolving = ticket.clone();
                #[expect(clippy::disallowed_methods, reason = "joined by resolve_plans")]
                let result = tokio::task::spawn_blocking(move || resolving.resolve())
                    .await
                    .unwrap_or_else(|error| Err(error.to_string().into()));
                session.transition(|state| state.finish_plan(&ticket, result));
            }
            session.publish();
        });
    }

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

    pub(crate) async fn dispatch(&self, intent: SessionIntent) -> SessionReply {
        self.begin(intent).finish().await
    }

    pub(crate) fn begin(&self, intent: SessionIntent) -> SessionRun {
        let remote_since = self.inner.deps.remote.ui_revisions();
        let since = self.lock().revision();
        let submission = matches!(
            &intent,
            SessionIntent::Submit | SessionIntent::Preview | SessionIntent::RestartTitle { .. }
        );
        let reply = self
            .inner
            .deps
            .tasks
            .admit(|| {
                let (sender, reply) = tokio::sync::oneshot::channel();
                match self.begin_rest(intent) {
                    Rest::Done(outcome) => {
                        let _ = sender.send(outcome);
                    }
                    rest => {
                        let session = self.clone();
                        self.inner.deps.tasks.spawn(async move {
                            let outcome = session.complete(rest).await;
                            let _ = sender.send(outcome);
                        });
                    }
                }
                reply
            })
            .unwrap_or_else(|error| {
                if submission {
                    self.transition(|state| state.refuse_submission(SubmitRefusal::Closing));
                }
                let (sender, reply) = tokio::sync::oneshot::channel();
                let _ = sender.send(SessionOutcome::Rejected {
                    error: AppErrorEnvelope::from(&error),
                });
                reply
            });
        self.publish();
        SessionRun {
            remote_since,
            session: self.clone(),
            since,
            reply,
        }
    }

    pub(crate) fn snapshot(&self) -> SessionUpdate {
        let mut snapshot = self.lock().update_since(None);
        self.inner.deps.remote.add_ui_parts(&mut snapshot, None);
        snapshot
    }

    pub(crate) fn waiting_write_paths(&self) -> Vec<PathBuf> {
        self.lock().waiting_write_paths()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per intent; each arm is a call"
    )]
    fn begin_rest(&self, intent: SessionIntent) -> Rest {
        use SessionIntent as I;
        match intent {
            I::Remote { intent } => Rest::Remote(self.inner.deps.remote.ui_begin(intent)),
            I::Import { paths } => self.begin_import(paths),
            I::SelectFile {
                title_id,
                modifiers,
            } => self.begin_select_file(title_id, modifiers),
            I::SelectAll => self.change_selection(WorkingSet::select_all),
            I::ClearSelection => self.change_selection(WorkingSet::clear_selection),
            I::RemoveFile { input_id } => self.change_selection(|set| {
                if let Some(index) = set.index_of(&input_id) {
                    set.remove_file(index);
                }
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
            I::MoveFile {
                title_id,
                direction,
            } => self.edit_titles(|set| {
                if let Some(index) = set.title_index(&title_id) {
                    set.move_file(index, direction);
                }
            }),
            I::ReorderFiles { title_id, to } => self.edit_titles(|set| {
                if let Some(from) = set.title_index(&title_id) {
                    set.reorder_files(from, to);
                }
            }),
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
                let defaults = self.transition(|state| {
                    state.output.set_template(template);
                    self.inner.output_edits.fetch_add(1, Ordering::SeqCst);
                    state.output.defaults()
                });
                let run = self.inner.deps.settings.remember_output_after_pause(
                    defaults,
                    Arc::clone(&self.inner.output_edits),
                    &self.inner.deps.tasks,
                );
                let host = self.inner.deps.host.clone();
                self.inner.deps.tasks.spawn(async move {
                    let reply = run.finish().await;
                    host.emit(EngineEvent::Settings(Box::new(reply.snapshot)));
                });
                Rest::Done(SessionOutcome::Applied)
            }
            I::SetDefaultAudio { edit } => match self.transition(|state| {
                let defaults = state.audio.edit(edit);
                if defaults.is_some() {
                    self.inner.audio_edits.fetch_add(1, Ordering::SeqCst);
                }
                defaults
            }) {
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
                self.sources_released();
                Rest::Done(SessionOutcome::Applied)
            }

            I::SetField {
                binding,
                field,
                value,
            } => self.edit_form(|state| state.set_field(binding, field, value)),
            I::SetFieldAction {
                binding,
                field,
                action,
            } => self.edit_form(|state| state.set_field_action(binding, field, action)),
            I::LoadCoverFromFile { path } => self.begin_cover_load(CoverSource::File(path)),
            I::LoadCoverFromDrop { paths } => {
                match crate::cover_source::dropped_cover_path(paths) {
                    Some(path) => self.begin_cover_load(CoverSource::File(path)),
                    None => Rest::Done(SessionOutcome::Applied),
                }
            }
            I::LoadCoverFromUrl { url } => self.begin_cover_load(CoverSource::Url(url)),
            I::ClearCover => {
                self.transition(SessionState::clear_cover);
                Rest::Done(SessionOutcome::Applied)
            }
            I::Save => self.begin_save(),
            I::Submit => self.begin_submission(false),
            I::Preview => self.begin_submission(true),
            I::CancelPreview {
                run_id,
                child_job_id,
            } => Rest::Done(self.cancel_preview_run(&run_id, child_job_id.as_deref())),
            I::TakePreviewOutput { run_id } => Rest::Done(SessionOutcome::PreviewOutput {
                path: self.transition(|state| state.preview.take_output(&run_id)),
            }),
            I::ChooseCollisionPolicy { review_id, policy } => {
                match self.transition(|state| state.take_review(review_id)) {
                    Some(draft) => Rest::Reviewed {
                        draft: Box::new(draft),
                        policy,
                        resets: self.inner.resets.load(Ordering::SeqCst),
                    },
                    None => Rest::Done(SessionOutcome::Superseded),
                }
            }
            I::CancelCollisionReview { review_id } => {
                let cancelled = self.transition(|state| state.cancel_review_named(review_id));
                if cancelled {
                    self.sources_released();
                }
                Rest::Done(if cancelled {
                    SessionOutcome::Applied
                } else {
                    SessionOutcome::Superseded
                })
            }
            I::RestartTitle { title_id, revision } => self.begin_restart(&title_id, revision),
            I::KeepTitleLocation { title_id, revision } => {
                match self.transition(|state| state.begin_keep_location(&title_id, revision)) {
                    Some(link) => Rest::KeepLocation {
                        title_id,
                        revision,
                        link: Box::new(link),
                    },
                    None => Rest::Done(SessionOutcome::Superseded),
                }
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
            I::LookupApply { index, revision } => self.begin_lookup_apply(index, revision),
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

    fn edit_titles(&self, change: impl FnOnce(&mut WorkingSet)) -> Rest {
        self.transition(|state| change(&mut state.working_set));
        Rest::Done(SessionOutcome::Applied)
    }

    fn edit_form(&self, change: impl FnOnce(&mut SessionState) -> bool) -> Rest {
        Rest::Done(if self.transition(change) {
            SessionOutcome::Applied
        } else {
            SessionOutcome::Superseded
        })
    }

    fn begin_select_file(&self, title_id: String, modifiers: SelectionModifiers) -> Rest {
        let bound = self.transition(|state| {
            if state.working_set.index_of(&title_id).is_none() {
                return Ok(None);
            }
            state.gate()?;
            state.working_set.select_title(&title_id, modifiers);
            Ok(Some(Bound {
                reads: state.rebind(),
                binding: state.binding,
            }))
        });
        match bound {
            Ok(None) => Rest::Done(SessionOutcome::Applied),
            Ok(Some(bound)) => Rest::Reads(bound),
            Err(GateBlock::SaveInProgress) => {
                Rest::Done(SessionOutcome::DraftRejected { message: None })
            }
            Err(GateBlock::Invalid(message)) => Rest::Done(SessionOutcome::DraftRejected {
                message: Some(message),
            }),
        }
    }

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

    async fn import(
        &self,
        paths: Vec<String>,
        resets: u64,
        turn: super::import_order::Turn,
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
                Some(Some(Bound {
                    reads: state.rebind(),
                    binding: state.binding,
                }))
            });
            if let Some(bound) = attempt {
                return bound;
            }
            unlocked.await;
        }
    }

    async fn import_acquired(&self, job: AcquisitionJob) -> AcquisitionHandoff {
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
        let resets = self.inner.resets.load(Ordering::SeqCst);
        let _in_order = self.inner.imports.wait(self.inner.imports.take()).await;
        let removed = |reason| AcquisitionHandoff::Removed { reason };
        if self.inner.resets.load(Ordering::SeqCst) != resets {
            return removed(HandoffRefusal::NothingAdded);
        }
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
                if self.inner.resets.load(Ordering::SeqCst) != resets {
                    return removed(HandoffRefusal::NothingAdded);
                }
                return removed(HandoffRefusal::ImportFailed {
                    error: AppErrorEnvelope::from(&error),
                });
            }
        };
        let titles = staged_titles(job);
        let listed = loop {
            let unlocked = self.inner.list_unlocked.notified();
            tokio::pin!(unlocked);
            unlocked.as_mut().enable();
            if self.inner.resets.load(Ordering::SeqCst) != resets {
                return removed(HandoffRefusal::NothingAdded);
            }
            let attempt = self.transition(|state| {
                if self.inner.resets.load(Ordering::SeqCst) != resets {
                    return Some(Err(HandoffRefusal::NothingAdded));
                }
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
                if self.inner.resets.load(Ordering::SeqCst) == resets {
                    AcquisitionHandoff::Imported { count }
                } else {
                    removed(HandoffRefusal::NothingAdded)
                }
            }
            Err(reason) => {
                self.publish();
                removed(reason)
            }
        }
    }

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

    fn sources_released(&self) {
        self.inner.sources_freed.notify_one();
        self.inner.list_unlocked.notify_waiters();
    }

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
            CoverSource::Url(url) => self
                .inner
                .network
                .covers
                .remote_full(url.trim())
                .await
                .map(|cover| cover.to_vec()),
        };
        self.transition(|state| {
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

    fn begin_save(&self) -> Rest {
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
        let outputs = self.update_outputs(plan.outputs).await;

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
        self.sources_released();
        SessionOutcome::Applied
    }

    async fn update_outputs(&self, edits: Vec<OutputEdit>) -> OutputEdits {
        if edits.is_empty() {
            return OutputEdits::default();
        }
        let mut replies = Vec::with_capacity(edits.len());
        for edit in edits {
            let mut reply = edit
                .title
                .update(edit.revision, &edit.intent)
                .map_err(|error| error.to_string());
            if matches!(
                reply,
                Ok(UpdateReply::Accepted {
                    published: true,
                    ..
                })
            ) {
                let title = Arc::clone(&edit.title);
                let _ = blocking(move || {
                    title.apply_published();
                    Ok(())
                })
                .await;
                if let Some(OutputUpdate {
                    status: OutputUpdateStatus::Failed { message },
                    ..
                }) = edit.title.update_state()
                {
                    reply = Err(message);
                }
            }
            replies.push((edit, reply));
        }
        self.transition(|state| state.record_output_edits(replies))
    }

    /// Brings outputs linked by a just-accepted export up to the session's
    /// edits: a Save made while the submission was being prepared did not
    /// reach the export.
    async fn catch_up_outputs(&self, title_ids: &[String]) {
        let edits = self.transition(|state| {
            let tags = &state.tags;
            state
                .exports
                .edits(|path| tags.pending(path).map(|pending| pending.patch.clone()))
                .into_iter()
                .filter(|edit| title_ids.contains(&edit.title_id))
                .collect::<Vec<_>>()
        });
        self.update_outputs(edits).await;
    }
}

enum CoverSource {
    File(String),
    Url(String),
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
    Ok((
        Bound {
            reads: state.rebind(),
            binding: state.binding,
        },
        count,
    ))
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

#[expect(clippy::disallowed_methods, reason = "joined by blocking")]
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
            #[expect(clippy::disallowed_methods, reason = "joined by read_all")]
            let read = tokio::spawn(async move {
                let _permit = limit.acquire_owned().await;
                let path = ticket.path.clone();
                let result = blocking(move || read_tags(&path)).await;
                (ticket, result)
            });
            read
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

#[path = "cover_request.rs"]
mod cover_request;

mod defaults_flow;
mod lookup_flow;
mod preview_flow;
mod save_flow;
mod submission_flow;

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
