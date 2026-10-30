use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use super::{
    storage, AcquisitionLane, AppSettings, AppSettingsPatch, ConcurrencyPreference,
    EncoderDefaults, OutputDefaults, PinnedDefaults, SettingsRecovery, SettingsUnit,
    StartupBehavior,
};
use crate::errors::{AppError, AppErrorEnvelope};
use crate::power::PowerManager;
use crate::processing::{JobRegistry, MaxConcurrentJobsCapabilities};
use crate::ManagedJobRegistry;

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingsIntent {
    /// Records defaults a panel has accepted. They stay in effect even when
    /// the write fails; the snapshot then reports the failure for retry.
    #[serde(rename_all = "camelCase")]
    Remember {
        encoder_defaults: Option<EncoderDefaults>,
        output_defaults: Option<OutputDefaults>,
        default_acquisition_lane: Option<AcquisitionLane>,
    },
    /// Changes how many titles export at once. Refused while jobs run.
    SetConcurrency {
        preference: ConcurrencyPreference,
    },
    SetKeepAwake {
        enabled: bool,
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
    pub startup_defaults: PinnedDefaults,
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
    state: tokio::sync::Mutex<State>,
    /// Completion of the last accepted intent, independent of host waits.
    turn: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

#[derive(Default)]
struct State {
    revision: u64,
    accepted: AppSettings,
    unsaved: bool,
    save_error: Option<AppErrorEnvelope>,
    recovery: Option<SettingsRecovery>,
    /// The settings file could be neither read nor kept aside; nothing is
    /// saved over it until it is kept or gone.
    unpreserved: bool,
}

fn startup_defaults(settings: &AppSettings) -> PinnedDefaults {
    match (&settings.startup_behavior, &settings.pinned_defaults) {
        (StartupBehavior::PinnedDefaults, Some(pinned)) => pinned.clone(),
        _ => PinnedDefaults {
            max_concurrent_jobs: settings.max_concurrent_jobs,
            encoder_defaults: settings.encoder_defaults.clone(),
            output_defaults: settings.output_defaults.clone(),
        },
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
        accepted.max_concurrent_jobs = startup.max_concurrent_jobs;
        accepted.encoder_defaults = startup.encoder_defaults.clone();
        accepted.output_defaults = startup.output_defaults.clone();
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
            reset.retain(|unit| {
                !matches!(
                    unit,
                    SettingsUnit::JobLimit
                        | SettingsUnit::EncoderDefaults
                        | SettingsUnit::OutputDefaults
                )
            });
        }
        let mut recovery = SettingsRecovery {
            reset,
            set_aside_as: None,
        };
        match loaded.set_aside {
            Some(Ok(kept)) => recovery.set_aside_as = kept,
            Some(Err(error)) => {
                state.unpreserved = true;
                state.unsaved = true;
                state.save_error = Some(AppErrorEnvelope::from(&not_kept(&error)));
            }
            None => {}
        }
        state.recovery =
            (!recovery.reset.is_empty() || recovery.set_aside_as.is_some()).then_some(recovery);
        state.accepted = accepted;
        let runtime = Self {
            inner: Arc::new(Inner {
                config_dir,
                jobs: Arc::clone(&jobs),
                power,
                state: tokio::sync::Mutex::new(state),
                turn: Mutex::default(),
            }),
        };
        (runtime, jobs, startup)
    }

    pub(crate) async fn snapshot(&self) -> SettingsSnapshot {
        self.snapshot_of(&*self.inner.state.lock().await)
    }

    fn snapshot_of(&self, state: &State) -> SettingsSnapshot {
        SettingsSnapshot {
            revision: state.revision,
            settings: state.accepted.clone(),
            save_error: state.save_error.clone(),
            concurrency: ConcurrencySnapshot {
                preference: state.accepted.max_concurrent_jobs,
                effective: self.inner.jobs.max_concurrent(),
                capabilities: JobRegistry::max_concurrent_jobs_capabilities(),
            },
            startup_defaults: startup_defaults(&state.accepted),
            recovery: state.recovery.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) async fn dispatch(&self, intent: SettingsIntent) -> SettingsReply {
        self.begin(intent, &crate::engine::EngineTasks::default())
            .finish()
            .await
    }

    /// Reserves its turn synchronously, so async replies cannot reorder writes.
    pub(crate) fn begin(
        &self,
        intent: SettingsIntent,
        tasks: &crate::engine::EngineTasks,
    ) -> SettingsRun {
        self.enqueue(intent, None, tasks)
    }

    pub(crate) fn remember_output_after_pause(
        &self,
        output: OutputDefaults,
        latest: Arc<std::sync::atomic::AtomicU64>,
        tasks: &crate::engine::EngineTasks,
    ) -> SettingsRun {
        let revision = latest.load(std::sync::atomic::Ordering::SeqCst);
        self.enqueue(
            SettingsIntent::Remember {
                encoder_defaults: None,
                output_defaults: Some(output),
                default_acquisition_lane: None,
            },
            Some((
                latest,
                revision,
                tokio::time::Instant::now() + std::time::Duration::from_millis(400),
            )),
            tasks,
        )
    }

    fn enqueue(
        &self,
        intent: SettingsIntent,
        pause: Option<(Arc<std::sync::atomic::AtomicU64>, u64, tokio::time::Instant)>,
        tasks: &crate::engine::EngineTasks,
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
            if let Some((_, _, deadline)) = &pause {
                tokio::time::sleep_until(*deadline).await;
            }
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            let superseded = pause.is_some_and(|(latest, revision, _)| {
                latest.load(std::sync::atomic::Ordering::SeqCst) != revision
            });
            let reply = if superseded {
                SettingsReply {
                    outcome: SettingsOutcome::Applied,
                    snapshot: settings.snapshot().await,
                }
            } else {
                settings.apply_ordered(intent).await
            };
            let _ = done.send(());
            reply
        });
        SettingsRun {
            settings: self.clone(),
            reply,
        }
    }

    async fn apply_ordered(&self, intent: SettingsIntent) -> SettingsReply {
        let mut state = self.inner.state.lock().await;
        let outcome = self.apply(&mut state, intent).await;
        state.revision += 1;
        SettingsReply {
            outcome,
            snapshot: self.snapshot_of(&state),
        }
    }

    async fn apply(&self, state: &mut State, intent: SettingsIntent) -> SettingsOutcome {
        match intent {
            SettingsIntent::Remember {
                encoder_defaults,
                output_defaults,
                default_acquisition_lane,
            } => {
                self.accept(
                    state,
                    AppSettingsPatch {
                        encoder_defaults,
                        output_defaults,
                        default_acquisition_lane,
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
        state.unsaved = true;
        self.write(state).await;
        SettingsOutcome::Applied
    }

    async fn write(&self, state: &mut State) {
        if !state.unsaved {
            return;
        }
        let config_dir = self.inner.config_dir.clone();
        let settings = state.accepted.clone();
        let unpreserved = state.unpreserved;
        #[expect(clippy::disallowed_methods, reason = "joined by write")]
        let written = tokio::task::spawn_blocking(move || {
            let kept = if unpreserved {
                storage::set_aside(&config_dir).map_err(|error| not_kept(&error))?
            } else {
                None
            };
            storage::save(&config_dir, &settings)?;
            Ok(kept)
        })
        .await
        .unwrap_or_else(|error| Err(AppError::General(format!("Settings write failed: {error}"))));
        match written {
            Ok(kept) => {
                state.unsaved = false;
                state.save_error = None;
                state.unpreserved = false;
                if let Some(name) = kept {
                    state.recovery.get_or_insert_default().set_aside_as = Some(name);
                }
            }
            Err(error) => {
                log::warn!("App settings were not saved: {error}");
                state.save_error = Some(AppErrorEnvelope::from(&error));
            }
        }
    }

    pub(crate) async fn flush(&self) -> Option<AppErrorEnvelope> {
        let mut state = self.inner.state.lock().await;
        self.write(&mut state).await;
        state.save_error.clone()
    }

    fn apply_to_runtime(&self, state: &State) {
        self.inner
            .power
            .set_enabled(state.accepted.keep_awake_while_working);
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
        let effective = match self.inner.jobs.update_max_concurrent(requested).await {
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
        let current = &state.accepted;
        let pinned = PinnedDefaults {
            max_concurrent_jobs: current.max_concurrent_jobs,
            encoder_defaults: current.encoder_defaults.clone(),
            output_defaults: current.output_defaults.clone(),
        };
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
        if self.inner.jobs.reset_to_auto().await.is_err() {
            return rejected(&AppError::InvalidInput(
                "Settings can't be reset while exports are running. Try again when they finish."
                    .to_string(),
            ));
        }
        let settings = AppSettings::default();
        *state = State {
            revision: state.revision,
            accepted: settings,
            unsaved: true,
            save_error: None,
            recovery: None,
            unpreserved: state.unpreserved,
        };
        self.apply_to_runtime(state);
        self.write(state).await;
        SettingsOutcome::Applied
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
