use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use serde::{Deserialize, Serialize};

use super::edit_stamp::TEMPLATE_PAUSE;
use super::{
    storage, AcquisitionLane, AppSettings, AppSettingsPatch, ConcurrencyPreference, EditStamp,
    EncoderDefaults, IndexerConnection, LatestEdit, OutputDefaults, PinnedDefaults,
    SettingsRecovery, StartupBehavior,
};
use crate::audio::AacDecoder;
use crate::errors::{AppError, AppErrorEnvelope};
use crate::power::PowerManager;
use crate::processing::{JobRegistry, MaxConcurrentJobsCapabilities};
use crate::ManagedJobRegistry;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingsIntent {
    /// Chooses the source an import starts from. It stays in effect even when
    /// the write fails; the snapshot then reports the failure for retry.
    SetDefaultAcquisitionLane {
        lane: AcquisitionLane,
    },
    /// Changes how many titles export at once. Refused while jobs run.
    SetConcurrency {
        preference: ConcurrencyPreference,
    },
    SetKeepAwake {
        enabled: bool,
    },
    /// Chooses whether the Quit shortcut needs a second press.
    SetPressQuitTwice {
        enabled: bool,
    },
    /// Chooses the decoder for AAC sources in imports, previews, and exports
    /// accepted from now on.
    SetAacDecoder {
        decoder: AacDecoder,
    },
    SetStartupBehavior {
        behavior: StartupBehavior,
    },
    PinCurrentDefaults,
    /// Writes accepted changes that an earlier write failed to save.
    Retry,
    /// Returns every setting to its default. Refused while exports run.
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingsOutcome {
    Applied,
    /// Nothing changed.
    Rejected {
        error: AppErrorEnvelope,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConcurrencySnapshot {
    pub preference: ConcurrencyPreference,
    /// How many titles export at once right now.
    pub effective: usize,
    pub capabilities: MaxConcurrentJobsCapabilities,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub revision: u64,
    pub settings: AppSettings,
    /// Why accepted changes are not on disk yet. Absent when all are saved.
    pub save_error: Option<AppErrorEnvelope>,
    pub concurrency: ConcurrencySnapshot,
    /// The defaults a host shows at launch: the pinned ones when the user
    /// chose that and has pinned some, otherwise the last used.
    #[serde(skip)]
    pub startup_defaults: PinnedDefaults,
    #[serde(skip)]
    pub(crate) applied_edits: EditStamp,
    /// What this launch could not keep from the saved settings. Absent after
    /// the next accepted change.
    pub recovery: Option<SettingsRecovery>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsReply {
    pub outcome: SettingsOutcome,
    pub snapshot: SettingsSnapshot,
}

#[derive(Clone)]
pub(crate) struct SettingsRuntime {
    inner: Arc<Inner>,
}

pub(crate) struct SettingsRun {
    settings: SettingsRuntime,
    reply: tokio::task::JoinHandle<SettingsReply>,
}

impl SettingsRun {
    pub(crate) async fn finish(self) -> SettingsReply {
        match self.reply.await {
            Ok(reply) => reply,
            Err(error) => SettingsReply {
                outcome: rejected(&AppError::General(format!("Settings work failed: {error}"))),
                snapshot: self.settings.snapshot().await,
            },
        }
    }
}

struct Inner {
    config_dir: PathBuf,
    jobs: ManagedJobRegistry,
    power: PowerManager,
    current: RwLock<Arc<AppSettings>>,
    state: tokio::sync::Mutex<State>,
    /// Completion of the last accepted intent, independent of host waits.
    turn: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

enum Change {
    Intent(SettingsIntent),
    IndexerConnection(IndexerConnection),
    Defaults {
        encoder_defaults: Option<EncoderDefaults>,
        output_defaults: Option<OutputDefaults>,
        stamp: EditStamp,
    },
}

#[derive(Default)]
enum Persistence {
    #[default]
    Saved,
    Pending {
        error: Option<AppErrorEnvelope>,
        /// The settings file could be neither read nor kept aside; nothing is
        /// saved over it until it is kept or gone.
        set_aside_first: bool,
        /// The unusable file a failed save already kept aside; recovery names
        /// it once a save lands.
        kept: Option<String>,
    },
}

impl Persistence {
    fn unsaved(&self) -> Self {
        match self {
            Self::Saved => Self::Pending {
                error: None,
                set_aside_first: false,
                kept: None,
            },
            Self::Pending {
                error,
                set_aside_first,
                kept,
            } => Self::Pending {
                error: error.clone(),
                set_aside_first: *set_aside_first,
                kept: kept.clone(),
            },
        }
    }

    fn error(&self) -> Option<&AppErrorEnvelope> {
        match self {
            Self::Saved => None,
            Self::Pending { error, .. } => error.as_ref(),
        }
    }

    fn kept(&self) -> Option<String> {
        match self {
            Self::Saved => None,
            Self::Pending { kept, .. } => kept.clone(),
        }
    }

    fn set_aside_first(&self) -> bool {
        matches!(
            self,
            Self::Pending {
                set_aside_first: true,
                ..
            }
        )
    }
}

#[derive(Default)]
struct State {
    revision: u64,
    accepted: AppSettings,
    persistence: Persistence,
    recovery: Option<SettingsRecovery>,
    applied: EditStamp,
}

impl State {
    /// Records how a write ended. A file kept aside stays kept when the save
    /// after it fails, so the retry does not set aside again and recovery
    /// names the file once a save lands.
    fn settle_write(
        &mut self,
        set_aside_first: bool,
        kept: Option<String>,
        saved: crate::errors::Result<()>,
    ) {
        let kept_aside = kept.is_some();
        let kept = kept.or_else(|| self.persistence.kept());
        match saved {
            Ok(()) => {
                self.persistence = Persistence::Saved;
                if let Some(name) = kept {
                    self.recovery.get_or_insert_default().set_aside_as = Some(name);
                }
            }
            Err(error) => {
                log::warn!("App settings were not saved: {error}");
                self.persistence = Persistence::Pending {
                    error: Some(AppErrorEnvelope::from(&error)),
                    set_aside_first: set_aside_first && !kept_aside,
                    kept,
                };
            }
        }
    }
}

fn startup_defaults(settings: &AppSettings) -> PinnedDefaults {
    match (&settings.startup_behavior, &settings.pinned_defaults) {
        (StartupBehavior::PinnedDefaults, Some(pinned)) => pinned.clone(),
        _ => settings.launch(),
    }
}

fn not_kept(error: &std::io::Error) -> AppError {
    AppError::General(format!(
        "ABB couldn't read settings.toml or keep a copy of it, so it won't save over it: {error}"
    ))
}

fn rejected(error: &AppError) -> SettingsOutcome {
    SettingsOutcome::Rejected {
        error: AppErrorEnvelope::from(error),
    }
}

impl SettingsRuntime {
    pub(crate) fn start(
        config_dir: PathBuf,
        power: PowerManager,
    ) -> (Self, ManagedJobRegistry, PinnedDefaults) {
        let loaded = storage::load(&config_dir);
        let mut accepted = loaded.settings;
        let startup = startup_defaults(&accepted);
        accepted.set_launch(startup.clone());
        let jobs: ManagedJobRegistry = Arc::new(match startup.max_concurrent_jobs {
            ConcurrencyPreference::Fixed(value) => JobRegistry::new(value),
            ConcurrencyPreference::Auto => JobRegistry::auto(),
        });
        log::info!(
            "Job registry initialized: max_concurrent = {}",
            jobs.max_concurrent()
        );
        power.set_enabled(accepted.keep_awake_while_working);
        let mut state = State::default();
        let mut reset = loaded.reset;
        if matches!(accepted.startup_behavior, StartupBehavior::PinnedDefaults)
            && accepted.pinned_defaults.is_some()
        {
            reset.retain(|unit| !unit.launch_default());
        }
        let mut recovery = SettingsRecovery {
            reset,
            set_aside_as: None,
        };
        match loaded.set_aside {
            Some(Ok(kept)) => recovery.set_aside_as = kept,
            Some(Err(error)) => {
                state.persistence = Persistence::Pending {
                    error: Some(AppErrorEnvelope::from(&not_kept(&error))),
                    set_aside_first: true,
                    kept: None,
                };
            }
            None => {}
        }
        state.recovery =
            (!recovery.reset.is_empty() || recovery.set_aside_as.is_some()).then_some(recovery);
        let current = RwLock::new(Arc::new(accepted.clone()));
        state.accepted = accepted;
        let runtime = Self {
            inner: Arc::new(Inner {
                config_dir,
                jobs: Arc::clone(&jobs),
                power,
                current,
                state: tokio::sync::Mutex::new(state),
                turn: Mutex::default(),
            }),
        };
        (runtime, jobs, startup)
    }

    fn current(&self) -> Arc<AppSettings> {
        Arc::clone(
            &self
                .inner
                .current
                .read()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    pub(crate) fn aac_decoder(&self) -> AacDecoder {
        self.current().aac_decoder
    }

    pub(crate) fn press_quit_twice(&self) -> bool {
        self.current().press_quit_twice
    }

    pub(crate) fn indexer_connection(&self) -> IndexerConnection {
        self.current().indexer_connection.clone()
    }

    pub(crate) fn output_defaults(&self) -> OutputDefaults {
        self.current().output_defaults.clone()
    }

    pub(crate) async fn snapshot(&self) -> SettingsSnapshot {
        self.snapshot_of(&*self.inner.state.lock().await)
    }

    fn snapshot_of(&self, state: &State) -> SettingsSnapshot {
        SettingsSnapshot {
            revision: state.revision,
            settings: state.accepted.clone(),
            save_error: state.persistence.error().cloned(),
            concurrency: ConcurrencySnapshot {
                preference: state.accepted.max_concurrent_jobs,
                effective: self.inner.jobs.max_concurrent(),
                capabilities: JobRegistry::max_concurrent_jobs_capabilities(),
            },
            startup_defaults: startup_defaults(&state.accepted),
            applied_edits: state.applied,
            recovery: state.recovery.clone(),
        }
    }

    /// Reserves its turn synchronously, so async replies cannot reorder writes.
    pub(crate) fn begin(
        &self,
        intent: SettingsIntent,
        tasks: &crate::engine_tasks::EngineTasks,
    ) -> SettingsRun {
        self.enqueue(Change::Intent(intent), None, tasks)
    }

    /// Reserves the turn for an Indexer Save. It is not a `SettingsIntent`: the
    /// connection changes only with its API key (a Save) or through Reset.
    pub(crate) fn set_indexer_connection(
        &self,
        connection: IndexerConnection,
        tasks: &crate::engine_tasks::EngineTasks,
    ) -> SettingsRun {
        self.enqueue(Change::IndexerConnection(connection), None, tasks)
    }

    pub(crate) fn remember_defaults(
        &self,
        encoder_defaults: Option<EncoderDefaults>,
        output_defaults: Option<OutputDefaults>,
        stamp: EditStamp,
        tasks: &crate::engine_tasks::EngineTasks,
    ) -> SettingsRun {
        self.enqueue(
            Change::Defaults {
                encoder_defaults,
                output_defaults,
                stamp,
            },
            None,
            tasks,
        )
    }

    pub(crate) fn remember_output_after_pause(
        &self,
        output: OutputDefaults,
        stamp: EditStamp,
        latest: LatestEdit,
        tasks: &crate::engine_tasks::EngineTasks,
    ) -> SettingsRun {
        self.enqueue(
            Change::Defaults {
                encoder_defaults: None,
                output_defaults: Some(output),
                stamp,
            },
            Some((latest, tokio::time::Instant::now() + TEMPLATE_PAUSE)),
            tasks,
        )
    }

    fn enqueue(
        &self,
        change: Change,
        pause: Option<(LatestEdit, tokio::time::Instant)>,
        tasks: &crate::engine_tasks::EngineTasks,
    ) -> SettingsRun {
        let (done, next) = tokio::sync::oneshot::channel();
        let previous = self
            .inner
            .turn
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(next);
        let settings = self.clone();
        let reply = tasks.spawn(async move {
            if let Some((_, deadline)) = &pause {
                tokio::time::sleep_until(*deadline).await;
            }
            if let Some(previous) = previous {
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "only the order matters; the previous run replies to its own caller"
                )]
                let _ = previous.await;
            }
            let superseded = pause.is_some_and(|(latest, _)| !latest.is_current());
            let reply = if superseded {
                SettingsReply {
                    outcome: SettingsOutcome::Applied,
                    snapshot: settings.snapshot().await,
                }
            } else {
                settings.apply_ordered(change).await
            };
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the receiver is gone once nobody waits for completion"
            )]
            let _ = done.send(());
            reply
        });
        SettingsRun {
            settings: self.clone(),
            reply,
        }
    }

    async fn apply_ordered(&self, change: Change) -> SettingsReply {
        let mut state = self.inner.state.lock().await;
        let outcome = match change {
            Change::Intent(intent) => self.apply(&mut state, intent).await,
            Change::Defaults {
                encoder_defaults,
                output_defaults,
                stamp,
            } => {
                if encoder_defaults.is_some() {
                    state.applied.audio = state.applied.audio.max(stamp.audio);
                }
                if output_defaults.is_some() {
                    state.applied.output = state.applied.output.max(stamp.output);
                }
                self.accept(
                    &mut state,
                    AppSettingsPatch {
                        encoder_defaults,
                        output_defaults,
                        ..Default::default()
                    },
                )
                .await
            }
            Change::IndexerConnection(connection) => {
                self.accept(
                    &mut state,
                    AppSettingsPatch {
                        indexer_connection: Some(connection),
                        ..Default::default()
                    },
                )
                .await
            }
        };
        state.revision += 1;
        SettingsReply {
            outcome,
            snapshot: self.snapshot_of(&state),
        }
    }

    async fn apply(&self, state: &mut State, intent: SettingsIntent) -> SettingsOutcome {
        match intent {
            SettingsIntent::SetDefaultAcquisitionLane { lane } => {
                self.accept(
                    state,
                    AppSettingsPatch {
                        default_acquisition_lane: Some(lane),
                        ..Default::default()
                    },
                )
                .await
            }
            SettingsIntent::SetConcurrency { preference } => {
                self.set_concurrency(state, preference).await
            }
            SettingsIntent::SetKeepAwake { enabled } => {
                self.accept(
                    state,
                    AppSettingsPatch {
                        keep_awake_while_working: Some(enabled),
                        ..Default::default()
                    },
                )
                .await
            }
            SettingsIntent::SetPressQuitTwice { enabled } => {
                self.accept(
                    state,
                    AppSettingsPatch {
                        press_quit_twice: Some(enabled),
                        ..Default::default()
                    },
                )
                .await
            }
            SettingsIntent::SetAacDecoder { decoder } => {
                self.accept(
                    state,
                    AppSettingsPatch {
                        aac_decoder: Some(decoder),
                        ..Default::default()
                    },
                )
                .await
            }
            SettingsIntent::SetStartupBehavior { behavior } => {
                self.accept(
                    state,
                    AppSettingsPatch {
                        startup_behavior: Some(behavior),
                        ..Default::default()
                    },
                )
                .await
            }
            SettingsIntent::PinCurrentDefaults => self.pin_current_defaults(state).await,
            SettingsIntent::Retry => {
                self.write(state).await;
                SettingsOutcome::Applied
            }
            SettingsIntent::Reset => self.reset(state).await,
        }
    }

    async fn accept(&self, state: &mut State, patch: AppSettingsPatch) -> SettingsOutcome {
        match state.accepted.clone().merge(patch) {
            Ok(next) => state.accepted = next,
            Err(error) => return rejected(&error),
        }
        state.recovery = None;
        self.apply_to_runtime(state);
        state.persistence = state.persistence.unsaved();
        self.write(state).await;
        SettingsOutcome::Applied
    }

    async fn write(&self, state: &mut State) {
        if matches!(state.persistence, Persistence::Saved) {
            return;
        }
        let config_dir = self.inner.config_dir.clone();
        let settings = state.accepted.clone();
        let set_aside_first = state.persistence.set_aside_first();
        #[expect(clippy::disallowed_methods, reason = "joined by write")]
        let (kept, saved) = tokio::task::spawn_blocking(move || {
            let kept = if set_aside_first {
                match storage::set_aside(&config_dir) {
                    Ok(kept) => kept,
                    Err(error) => return (None, Err(not_kept(&error))),
                }
            } else {
                None
            };
            (kept, storage::save(&config_dir, &settings))
        })
        .await
        .unwrap_or_else(|error| {
            (
                None,
                Err(AppError::General(format!("Settings write failed: {error}"))),
            )
        });
        state.settle_write(set_aside_first, kept, saved);
    }

    pub(crate) async fn flush(&self) -> Option<AppErrorEnvelope> {
        let mut state = self.inner.state.lock().await;
        self.write(&mut state).await;
        state.persistence.error().cloned()
    }

    fn apply_to_runtime(&self, state: &State) {
        self.inner
            .power
            .set_enabled(state.accepted.keep_awake_while_working);
        *self
            .inner
            .current
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Arc::new(state.accepted.clone());
    }

    /// Asks the scheduler to accept the change before recording it. A fixed
    /// choice is recorded as the count the scheduler settled on.
    async fn set_concurrency(
        &self,
        state: &mut State,
        preference: ConcurrencyPreference,
    ) -> SettingsOutcome {
        if let Err(error) = preference.validate() {
            return rejected(&error);
        }
        let requested = preference.requested_value(JobRegistry::default_max());
        let effective = match self.inner.jobs.update_max_concurrent(requested) {
            Ok(effective) => effective,
            Err(error) => return rejected(&error),
        };
        let accepted = preference.accepted(effective);
        self.accept(
            state,
            AppSettingsPatch {
                max_concurrent_jobs: Some(accepted),
                ..Default::default()
            },
        )
        .await
    }

    async fn pin_current_defaults(&self, state: &mut State) -> SettingsOutcome {
        let pinned = state.accepted.launch();
        self.accept(
            state,
            AppSettingsPatch {
                pinned_defaults: Some(pinned),
                ..Default::default()
            },
        )
        .await
    }

    async fn reset(&self, state: &mut State) -> SettingsOutcome {
        if self.inner.jobs.reset_to_auto().is_err() {
            return rejected(&AppError::InvalidInput(
                "Settings can't be reset while exports are running. Try again when they finish."
                    .to_string(),
            ));
        }
        let settings = AppSettings::default();
        *state = State {
            revision: state.revision,
            accepted: settings,
            persistence: Persistence::Pending {
                error: None,
                set_aside_first: state.persistence.set_aside_first(),
                kept: state.persistence.kept(),
            },
            recovery: None,
            applied: state.applied,
        };
        self.apply_to_runtime(state);
        self.write(state).await;
        SettingsOutcome::Applied
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
