use std::collections::BTreeSet;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::app_settings::{
    SettingsIntent, SettingsOutcome, SettingsReply, SettingsRuntime, SettingsSnapshot,
};
use crate::audio;
use crate::engine_tasks::EngineTasks;
use crate::errors::{AppError, Result};
use crate::host::{EventSink, Host};
use crate::metadata::AudiobookMetadata;
use crate::output_artifact::OutputFolderRecords;
use crate::power::PowerManager;
use crate::processing::run::ProcessingRoots;
use crate::quit_press::{QuitPress, QuitPresses, QUIT_PRESS_WINDOW};
use crate::remote_source::{RemoteSourceConfig, RemoteSourceRuntime};
use crate::session::{
    Session, SessionDeps, SessionIntent, SessionReply, SessionRun, SessionUpdate,
};
use crate::work_runtime::{OperationId, OperationSnapshot, WorkOperationsSnapshot, WorkRuntime};

fn lock_cache_dir(cache_dir: &Path) -> Result<Option<File>> {
    let unlocked = |error: std::io::Error| {
        log::warn!("The working folder is not locked against a second ABB: {error}");
        Ok(None)
    };
    let opened = std::fs::create_dir_all(cache_dir).and_then(|()| {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(cache_dir.join("engine.lock"))
    });
    let file = match opened {
        Ok(file) => file,
        Err(error) => return unlocked(error),
    };
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Err(AppError::AlreadyRunning(cache_dir.to_path_buf())),
        Err(TryLockError::Error(error)) => unlocked(error),
    }
}

pub struct EngineConfig {
    pub cache_dir: PathBuf,
    pub config_dir: PathBuf,
    pub app_identifier: String,
    pub events: Arc<dyn EventSink>,
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    host: Host,
    settings: SettingsRuntime,
    work: WorkRuntime,
    remote_source: RemoteSourceRuntime,
    session: Session,
    tasks: EngineTasks,
    quit_presses: QuitPresses,
    _cache_dir_lock: Option<File>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunningWork {
    pub exports: usize,
    pub waiting_writes: usize,
    pub acquisitions: usize,
    pub consent: QuitConsent,
}

#[must_use]
#[derive(Debug, Clone, PartialEq)]
pub enum ShutdownOutcome {
    Settled,
    SettingsUnsaved { error: crate::AppErrorEnvelope },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuitConsent(BTreeSet<StoppedWork>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum StoppedWork {
    Export(String),
    WaitingWrite(PathBuf),
    Acquisition(String),
}

impl Engine {
    pub fn start(config: EngineConfig) -> Result<Self> {
        let cache_dir_lock = lock_cache_dir(&config.cache_dir)?;
        audio::limit_ffmpeg_logging();
        let power = PowerManager::default();
        let host = Host::new(config.events, power.clone());
        let tasks = EngineTasks::default();
        if let Err(error) = OutputFolderRecords::for_cache(&config.cache_dir).remove_abandoned() {
            log::warn!("Abandoned output folders were left in place: {error}");
        }
        if let Err(error) = audio::cleanup_abandoned_processing_workspaces(&config.cache_dir) {
            log::warn!("Abandoned processing files were left in place: {error}");
        }
        let (settings, jobs, startup) = SettingsRuntime::start(config.config_dir, power.clone());
        let remote_source = RemoteSourceRuntime::new(RemoteSourceConfig {
            cache_dir: config.cache_dir.clone(),
            settings: settings.clone(),
            app_identifier: config.app_identifier,
            power: power.clone(),
            host: host.clone(),
            tasks: tasks.clone(),
        })?;
        if let Err(error) = remote_source.cleanup_abandoned_sessions() {
            log::warn!("Abandoned remote downloads were left in place: {error}");
        }

        let work = WorkRuntime::new(tasks.clone());
        work.follow_concurrency_lock(&host, &jobs);
        let session = Session::new(SessionDeps {
            remote: remote_source.clone(),
            host: host.clone(),
            work: work.clone(),
            jobs: Arc::clone(&jobs),
            temporary_root: remote_source.staging_root(),
            settings: settings.clone(),
            tasks: tasks.clone(),
            roots: ProcessingRoots::for_cache(&config.cache_dir),
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
                session,
                tasks,
                quit_presses: QuitPresses::default(),
                _cache_dir_lock: cache_dir_lock,
            }),
        })
    }

    pub fn running_work(&self) -> RunningWork {
        let mut exports: BTreeSet<String> = self
            .inner
            .work
            .unfinished_exports()
            .into_iter()
            .map(|operation| operation.0)
            .collect();
        exports.extend(
            self.inner
                .session
                .export_in_preparation()
                .map(|operation| operation.0),
        );
        let waiting_writes = self.inner.session.waiting_write_paths();
        let acquisitions = self.inner.remote_source.running_acquisitions();
        RunningWork {
            exports: exports.len(),
            waiting_writes: waiting_writes.len(),
            acquisitions: acquisitions.len(),
            consent: QuitConsent(
                exports
                    .into_iter()
                    .map(StoppedWork::Export)
                    .chain(waiting_writes.into_iter().map(StoppedWork::WaitingWrite))
                    .chain(acquisitions.into_iter().map(StoppedWork::Acquisition))
                    .collect(),
            ),
        }
    }

    pub fn quit_pressed(&self) -> QuitPress {
        let second_press_required =
            self.inner.settings.press_quit_twice() && self.running_work().consent.0.is_empty();
        let press = self
            .inner
            .quit_presses
            .press(Instant::now(), second_press_required);
        if press == QuitPress::PressAgain {
            self.inner.host.emit(crate::EngineEvent::QuitPressAgain {
                window_ms: QUIT_PRESS_WINDOW.as_millis() as u32,
            });
        }
        press
    }

    pub fn close_for_quit(&self, consent: &QuitConsent) -> std::result::Result<(), RunningWork> {
        self.inner.tasks.close_if(|| {
            let running = self.running_work();
            if running.consent.0.is_subset(&consent.0) {
                Ok(())
            } else {
                Err(running)
            }
        })
    }

    pub async fn shutdown(&self) -> ShutdownOutcome {
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
        match self.inner.settings.flush().await {
            None => ShutdownOutcome::Settled,
            Some(error) => ShutdownOutcome::SettingsUnsaved { error },
        }
    }

    pub async fn settings_dispatch(&self, intent: SettingsIntent) -> SettingsReply {
        let reset = matches!(intent, SettingsIntent::Reset);
        let reset_lease = if reset {
            match self.inner.remote_source.reset_lease() {
                Ok(lease) => Some(lease),
                Err(error) => return self.settings_rejected(error).await,
            }
        } else {
            None
        };
        let accepted = self.inner.tasks.admit(|| {
            let run = self.inner.settings.begin(intent, &self.inner.tasks);
            let engine = self.clone();
            self.inner.tasks.spawn(async move {
                let reply = run.finish().await;
                if reset && reply.outcome == SettingsOutcome::Applied {
                    engine.inner.session.adopt_reset(&reply.snapshot);
                    engine.inner.remote_source.saved_connection_replaced().await;
                }
                drop(reset_lease);
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

    pub async fn settings_snapshot(&self) -> SettingsSnapshot {
        self.inner.settings.snapshot().await
    }

    pub async fn session_dispatch(&self, intent: SessionIntent) -> SessionReply {
        self.inner.session.dispatch(intent).await
    }

    pub fn session_begin(&self, intent: SessionIntent) -> SessionRun {
        self.inner.session.begin(intent)
    }

    pub fn session_snapshot(&self) -> SessionUpdate {
        self.inner.session.snapshot()
    }

    pub async fn cover(&self, request: &str) -> Result<Option<std::sync::Arc<[u8]>>> {
        self.inner
            .tasks
            .until_closing(self.inner.session.cover(request))
            .await
    }

    pub async fn open_audio_files(&self, paths: Vec<PathBuf>) -> Result<()> {
        self.inner.session.import_opened(paths)
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "joined by Engine::read_audio_metadata"
    )]
    pub async fn read_audio_metadata(&self, file_path: String) -> Result<AudiobookMetadata> {
        tokio::task::spawn_blocking(move || {
            let validated_path = audio::validate_input_audio_path(&PathBuf::from(&file_path))?;
            crate::metadata::read_metadata(validated_path.to_string_lossy().as_ref())
        })
        .await
        .map_err(|e| AppError::General(format!("Metadata read task failed: {e}")))?
    }

    pub fn list_work_operations(&self) -> WorkOperationsSnapshot {
        self.inner.work.list_operations()
    }

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

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
