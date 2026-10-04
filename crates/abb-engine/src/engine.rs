//! The engine's host-facing interface and lifetime.

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use crate::app_settings::{
    SettingsIntent, SettingsOutcome, SettingsReply, SettingsRuntime, SettingsSnapshot,
};
use crate::audio::{self, SupportedAudioImportMetadata};
use crate::errors::{AppError, Result};
use crate::host::{EventSink, Host};
use crate::metadata::AudiobookMetadata;
use crate::opened_audio::OpenedAudioFileQueue;
use crate::power::PowerManager;
use crate::remote_source::{RemoteSourceConfig, RemoteSourceRuntime};
use crate::session::{
    Session, SessionDeps, SessionIntent, SessionReply, SessionRun, SessionUpdate,
};
use crate::work_runtime::{OperationId, OperationListSnapshot, OperationSnapshot, WorkRuntime};
use tokio_util::task::TaskTracker;

/// Admission and shutdown share this lock: visible work is registered before
/// shutdown closes admission and enumerates what it must cancel. Internal
/// cleanup may still spawn while its already-tracked parent is settling.
#[derive(Clone, Default)]
pub(crate) struct EngineTasks {
    tracker: TaskTracker,
    admission: Arc<Mutex<()>>,
    /// Fires when shutdown begins, for work that has nothing to save and is
    /// dropped rather than waited for.
    closing: tokio_util::sync::CancellationToken,
}

impl EngineTasks {
    pub(crate) fn admit<T>(&self, register: impl FnOnce() -> T) -> Result<T> {
        let _turn = self
            .admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if self.is_closed() {
            return Err(AppError::General("ABB is closing.".into()));
        }
        Ok(register())
    }

    pub(crate) fn close(&self) {
        let _turn = self
            .admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.tracker.close();
        self.closing.cancel();
    }

    /// Runs `work` unless shutdown begins first; then it is dropped.
    pub(crate) async fn until_closing<T>(
        &self,
        work: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        tokio::select! {
            result = work => result,
            () = self.closing.cancelled() => Err(AppError::General("ABB is closing.".into())),
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.tracker.is_closed()
    }

    pub(crate) fn spawn<F>(&self, work: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.tracker.spawn(work)
    }

    pub(crate) async fn wait(&self) {
        self.tracker.wait().await;
    }
}

/// What a host supplies to start the engine.
pub struct EngineConfig {
    /// Root for engine-owned working files: processing workspaces and staged
    /// remote downloads. The engine clears abandoned contents at start.
    pub cache_dir: PathBuf,
    /// Root for durable settings.
    pub config_dir: PathBuf,
    /// Scopes stored credentials. Two identifiers never share a vault.
    pub app_identifier: String,
    /// Where the engine publishes events.
    pub events: Arc<dyn EventSink>,
    /// Location of the Audible helper binary. `None` resolves it beside the
    /// running executable.
    pub aaxclean_helper: Option<PathBuf>,
}

/// One running ABB engine. Cloning shares the same engine.
///
/// One engine owns one storage namespace (`cache_dir`, `config_dir`,
/// `app_identifier`): starting it removes working files a previous run left
/// behind, so two engines must not share those roots.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    host: Host,
    settings: SettingsRuntime,
    work: WorkRuntime,
    remote_source: RemoteSourceRuntime,
    opened_audio: Arc<OpenedAudioFileQueue>,
    session: Session,
    /// Every background task the engine starts; shutdown waits for them.
    tasks: EngineTasks,
}

/// Work still running that quitting would stop.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunningWork {
    /// Accepted exports that have not finished.
    pub exports: usize,
    /// Files with a Save waiting for an export to finish reading them.
    pub waiting_writes: usize,
    /// Audible downloads in progress.
    pub acquisitions: usize,
}

impl RunningWork {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl Engine {
    /// Starts the engine: clears working files abandoned by a previous run,
    /// then puts the saved settings in effect.
    pub fn start(config: EngineConfig) -> Result<Self> {
        let power = PowerManager::default();
        let host = Host::new(config.events, power.clone());
        let tasks = EngineTasks::default();
        audio::cleanup_abandoned_processing_workspaces(&config.cache_dir)?;
        let remote_source = RemoteSourceRuntime::new(RemoteSourceConfig {
            cache_dir: config.cache_dir.clone(),
            config_dir: config.config_dir.clone(),
            app_identifier: config.app_identifier,
            power: power.clone(),
            host: host.clone(),
            aaxclean_helper: config.aaxclean_helper,
            tasks: tasks.clone(),
        })?;
        remote_source.cleanup_abandoned_sessions()?;

        let (settings, jobs, startup) = SettingsRuntime::start(config.config_dir, power.clone());
        let work = WorkRuntime::new(tasks.clone());
        let opened_audio = Arc::new(OpenedAudioFileQueue::default());
        let session = Session::new(SessionDeps {
            remote: remote_source.clone(),
            host: host.clone(),
            work: work.clone(),
            jobs: Arc::clone(&jobs),
            temporary_root: remote_source.staging_root(),
            opened_audio: Arc::clone(&opened_audio),
            settings: settings.clone(),
            tasks: tasks.clone(),
            workspace_root: audio::processing_workspace_root(&config.cache_dir),
            remove_staged: {
                let remote_source = remote_source.clone();
                Arc::new(move |job_id| remote_source.purge_session(job_id))
            },
        });
        remote_source.set_handoff(session.handoff());
        session.start_from_defaults(Some(&startup), Some(audio::encoder_settings_capabilities()));
        Ok(Self {
            inner: Arc::new(EngineInner {
                host,
                settings,
                work,
                remote_source,
                opened_audio,
                session,
                tasks,
            }),
        })
    }

    /// What quitting now would stop.
    pub fn running_work(&self) -> RunningWork {
        RunningWork {
            exports: self.inner.work.unfinished_exports().len(),
            waiting_writes: self.inner.session.waiting_write_paths().len(),
            acquisitions: self.inner.remote_source.running_acquisitions(),
        }
    }

    /// Stops the engine: refuses new exports and acquisitions, cancels the
    /// running ones, and waits for every background task to settle, so the
    /// engine's folders can be reused or removed. Saves that were waiting for
    /// a cancelled export are written once it stops reading their files.
    pub async fn shutdown(&self) {
        self.inner.tasks.close();
        self.inner.session.cancel_review();
        self.inner.session.cancel_preview();
        for operation in self.inner.work.unfinished_exports() {
            if let Err(error) = self
                .inner
                .work
                .cancel_operation(&self.inner.host, operation, None)
            {
                log::warn!("Failed to cancel an export while shutting down: {error}");
            }
        }
        self.inner.remote_source.abort_acquisitions();
        self.inner.tasks.wait().await;
        // A choice whose write failed gets one more attempt before ABB exits.
        self.inner.settings.flush().await;
    }

    // ---- Settings ----

    /// Applies one intent to the settings and returns the settings in effect.
    pub async fn settings_dispatch(&self, intent: SettingsIntent) -> SettingsReply {
        let reset = matches!(intent, SettingsIntent::Reset);
        let accepted = self.inner.tasks.admit(|| {
            let checkpoint = self.inner.session.defaults_checkpoint();
            let run = self.inner.settings.begin(intent, &self.inner.tasks);
            let engine = self.clone();
            self.inner.tasks.spawn(async move {
                let reply = run.finish().await;
                if reset && reply.outcome == SettingsOutcome::Applied {
                    engine.inner.session.replace_defaults(
                        &reply.snapshot.startup_defaults,
                        checkpoint,
                        reply.snapshot.revision,
                    );
                }
                engine
                    .inner
                    .host
                    .emit(crate::EngineEvent::Settings(Box::new(
                        reply.snapshot.clone(),
                    )));
                reply
            })
        });
        match accepted {
            Ok(reply) => match reply.await {
                Ok(reply) => reply,
                Err(error) => {
                    self.settings_rejected(AppError::General(format!(
                        "Settings work failed: {error}"
                    )))
                    .await
                }
            },
            Err(error) => self.settings_rejected(error).await,
        }
    }

    async fn settings_rejected(&self, error: AppError) -> SettingsReply {
        SettingsReply {
            outcome: SettingsOutcome::Rejected {
                error: crate::AppErrorEnvelope::from(&error),
            },
            snapshot: self.inner.settings.snapshot().await,
        }
    }

    /// The settings in effect and whether they are saved.
    pub async fn settings_snapshot(&self) -> SettingsSnapshot {
        self.inner.settings.snapshot().await
    }

    // ---- Working session ----

    /// Applies one intent to the working session and returns what changed.
    /// Changes the engine makes on its own arrive as [`crate::EngineEvent::Session`].
    pub async fn session_dispatch(&self, intent: SessionIntent) -> SessionReply {
        self.inner.session.dispatch(intent).await
    }

    /// Applies an intent's immediate effect and returns the rest of its work.
    /// A host whose transport may reorder or overlap requests begins intents
    /// in the order the user made them and finishes each one afterward.
    pub fn session_begin(&self, intent: SessionIntent) -> SessionRun {
        self.inner.session.begin(intent)
    }

    /// The whole working session, for a host that is starting or resyncing.
    pub fn session_snapshot(&self) -> SessionUpdate {
        self.inner.session.snapshot()
    }

    /// The cover image the session currently shows.
    pub fn session_cover_art(&self) -> Option<Vec<u8>> {
        self.inner.session.cover_art()
    }

    // ---- Import ----

    pub fn supported_audio_import_metadata(&self) -> SupportedAudioImportMetadata {
        audio::supported_audio_import_metadata()
    }

    /// Queues files the operating system asked ABB to open. Unsupported paths
    /// are dropped. Returns whether anything was queued. The session's
    /// `ImportOpened` intent imports them.
    pub fn queue_opened_audio_files(&self, paths: Vec<PathBuf>) -> Result<bool> {
        let supported = crate::opened_audio::supported_opened_audio_paths(paths);
        if supported.is_empty() {
            return Ok(false);
        }
        self.inner.opened_audio.push_paths(supported)?;
        Ok(true)
    }

    // ---- Metadata ----

    pub async fn read_audio_metadata(&self, file_path: String) -> Result<AudiobookMetadata> {
        tokio::task::spawn_blocking(move || {
            let validated_path = audio::validate_input_audio_path(&PathBuf::from(&file_path))?;
            crate::metadata::read_metadata(validated_path.to_string_lossy().as_ref())
        })
        .await
        .map_err(|e| AppError::General(format!("Metadata read task failed: {e}")))?
    }

    /// Reads an audio file's embedded cover as a bounded JPEG thumbnail.
    pub async fn read_audio_cover_thumbnail(&self, file_path: String) -> Result<Option<Vec<u8>>> {
        tokio::task::spawn_blocking(move || {
            let validated_path = audio::validate_input_audio_path(&PathBuf::from(&file_path))?;
            crate::metadata::read_audio_cover_thumbnail(&validated_path)
        })
        .await
        .map_err(|error| AppError::General(format!("Cover thumbnail read task failed: {error}")))?
    }

    /// Loads a cover image from an HTTPS URL as write-ready JPEG bytes.
    pub async fn load_cover_art_from_url(&self, url: String) -> Result<Vec<u8>> {
        crate::cover_source::load_cover_art_from_url(url).await
    }

    // ---- Output and processing ----

    pub fn list_work_operations(&self) -> Result<OperationListSnapshot> {
        self.inner.work.list_operations()
    }

    /// Cancels a whole operation, or one of its titles when `child_job_id` is given.
    pub fn cancel_work_operation(
        &self,
        operation_id: OperationId,
        child_job_id: Option<String>,
    ) -> Result<OperationSnapshot> {
        self.inner
            .work
            .cancel_operation(&self.inner.host, operation_id, child_job_id)
    }
}
