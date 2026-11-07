use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use cover_flow::CoverSource;
use lookup_flow::ChosenResult;

use super::exports::ExportLink;
use super::intent::{SessionIntent, SessionOutcome, SessionReply};
use super::lookup::{QueueStep, RESULT_LIMIT};
use super::plans::PlanTicket;
use super::state::{GateBlock, SavePlan, SessionState, SessionUpdate};
use super::submission::{Draft, SubmitRefusal};
use super::tag_cache::ReadTicket;
use super::working_set::{SelectionModifiers, WorkingSet};
use crate::app_settings::{SettingsRun, SettingsRuntime};
use crate::audio::{self};
use crate::errors::{AppError, AppErrorEnvelope, Result};
use crate::host::{EngineEvent, Host};
use crate::metadata::AudiobookMetadata;
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource};
use crate::output_artifact::CollisionPolicy;
use crate::remote_source::{RemoteRevisions, RemoteUiResult};
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
    pub(crate) tasks: crate::engine_tasks::EngineTasks,
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

impl Bound {
    fn rebind(state: &mut SessionState) -> Self {
        let reads = state.rebind();
        Self {
            binding: state.binding,
            reads,
        }
    }
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
        turn: super::import_order::Turn,
    },
    Remember(SettingsRun),
    Submit {
        draft: Box<Draft>,
    },
    Reviewed {
        draft: Box<Draft>,
        policy: CollisionPolicy,
    },
    Restart {
        draft: Box<Draft>,
        link: Box<ExportLink>,
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
            Rest::Import { paths, turn } => session.import(paths, turn).await,
            Rest::Remember(run) => session.finish_remember(run).await,
            Rest::Submit { draft } => session.submit(*draft).await,
            Rest::Reviewed { draft, policy } => session.submit_reviewed(*draft, policy).await,
            Rest::Save { epoch, plan } => session.save(epoch, plan).await,
            Rest::Restart { draft, link } => session.restart(*draft, *link).await,
            Rest::KeepLocation {
                title_id,
                revision,
                link,
            } => {
                session
                    .keep_location_finished(title_id, revision, *link)
                    .await
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

    fn resolve_plans(&self, tickets: Vec<PlanTicket>) {
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
                        #[expect(
                            clippy::let_underscore_must_use,
                            reason = "the host may have dropped its wait; the result also reaches it as an event"
                        )]
                        let _ = sender.send(outcome);
                    }
                    rest => {
                        let session = self.clone();
                        self.inner.deps.tasks.spawn(async move {
                            let outcome = session.complete(rest).await;
                            #[expect(
                                clippy::let_underscore_must_use,
                                reason = "the host may have dropped its wait; the result also reaches it as an event"
                            )]
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
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "the host may have dropped its wait; the result also reaches it as an event"
                )]
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

    #[expect(clippy::too_many_lines, reason = "one arm per intent")]
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
            I::SetNamingTemplate { template } => self.set_naming_template(template),
            I::SetDefaultAudio { edit } => self.set_default_audio(edit),
            I::SetTitleAudio { title_ids, edit } => {
                self.transition(|state| state.edit_title_audio(&title_ids, edit));
                Rest::Done(SessionOutcome::Applied)
            }
            I::ApplyDefaultAudio { title_ids } => {
                self.transition(|state| state.apply_default_audio(&title_ids));
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
            I::LoadCoverFromDrop { paths } => self.begin_cover_drop(paths),
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
                self.begin_collision_policy(review_id, policy)
            }
            I::CancelCollisionReview { review_id } => self.cancel_collision_review(review_id),
            I::RestartTitle { title_id, revision } => self.begin_restart(&title_id, revision),
            I::KeepTitleLocation { title_id, revision } => {
                self.begin_keep_location(title_id, revision)
            }

            I::LookupOpen => self.begin_lookup_open(),
            I::LookupClose => self.begin_lookup_close(),
            I::LookupSearch => Rest::LookupSearch {
                request: self.begin_lookup_action(),
            },
            I::LookupApply { index, revision } => self.begin_lookup_apply(index, revision),
            I::LookupSkip => Rest::LookupAdvance {
                request: self.begin_lookup_action(),
                step: QueueStep::Skipped,
            },
            I::LookupSetTitleQuery { value } => {
                self.edit_lookup_search(|lookup| lookup.title_query = value)
            }
            I::LookupSetAuthorQuery { value } => {
                self.edit_lookup_search(|lookup| lookup.author_query = value)
            }
            I::LookupSetSource { source } => {
                self.edit_lookup_search(|lookup| lookup.source = source)
            }
            I::LookupSetApplyMode { mode } => {
                self.edit_lookup_setting(|lookup| lookup.apply_mode = mode)
            }
            I::LookupSetReplaceCover { replace } => {
                self.edit_lookup_setting(|lookup| lookup.replace_cover = replace)
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
            SessionOutcome::EditLost
        })
    }

    fn begin_select_file(&self, title_id: String, modifiers: SelectionModifiers) -> Rest {
        let bound = self.transition(|state| {
            if state.working_set.index_of(&title_id).is_none() {
                return Ok(None);
            }
            state.gate()?;
            state.working_set.select_title(&title_id, modifiers);
            Ok(Some(Bound::rebind(state)))
        });
        match bound {
            Ok(None) => Rest::Done(SessionOutcome::Applied),
            Ok(Some(bound)) => Rest::Reads(bound),
            Err(block) => Rest::Done(draft_rejected(block)),
        }
    }

    fn change_selection(&self, change: impl FnOnce(&mut WorkingSet)) -> Rest {
        let bound = self.transition(|state| {
            state.gate()?;
            change(&mut state.working_set);
            Ok(Bound::rebind(state))
        });
        match bound {
            Ok(bound) => Rest::Reads(bound),
            Err(block) => Rest::Done(draft_rejected(block)),
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

    fn sources_released(&self) {
        self.inner.sources_freed.notify_one();
        self.inner.list_unlocked.notify_waiters();
    }
}

#[expect(clippy::disallowed_methods, reason = "joined by blocking")]
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::General(format!("Background task failed: {error}")))?
}

fn draft_rejected(block: GateBlock) -> SessionOutcome {
    match block {
        GateBlock::SaveInProgress => SessionOutcome::DraftRejected { message: None },
        GateBlock::Invalid(message) => SessionOutcome::DraftRejected {
            message: Some(message),
        },
    }
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

mod cover_flow;
mod defaults_flow;
mod download_flow;
mod import_flow;
mod lookup_flow;
mod preview_flow;
mod save_flow;
mod submission_flow;

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
