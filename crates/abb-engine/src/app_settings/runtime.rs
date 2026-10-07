//! The settings in effect for this run, and whether they are on disk yet.
//!
//! A preference a panel has accepted stays in effect when writing it fails;
//! the unsaved part is kept, coalesced by field, and written on retry. One
//! lock serializes every change, so an older write can never report a newer
//! choice as saved.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use super::{
    get_app_settings, save_app_settings, AcquisitionLane, AppSettings, AppSettingsPatch,
    ConcurrencyPreference, EncoderDefaults, OutputDefaults, PinnedDefaults, StartupBehavior,
};
use crate::audio::AacDecoder;
use crate::errors::{AppError, AppErrorEnvelope};
use crate::power::PowerManager;
use crate::processing::{JobRegistry, MaxConcurrentJobsCapabilities};
use crate::ManagedJobRegistry;

/// Something the user asked of the settings.
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
    /// Chooses the decoder for AAC sources in imports, previews, and exports
    /// accepted from now on.
    SetAacDecoder {
        decoder: AacDecoder,
    },
    SetStartupBehavior {
        behavior: StartupBehavior,
    },
    /// Captures the current defaults as the ones a later launch starts from.
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
    /// The settings in effect.
    pub settings: AppSettings,
    /// Why accepted changes are not on disk yet. Absent when all are saved.
    pub save_error: Option<AppErrorEnvelope>,
    pub concurrency: ConcurrencySnapshot,
    /// The defaults a host shows at launch: the pinned ones when the user
    /// chose that and has pinned some, otherwise the last used.
    pub startup_defaults: PinnedDefaults,
    pub default_acquisition_lane: AcquisitionLane,
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
    /// Whether the settings in effect have changed since they last reached disk.
    unsaved: bool,
    save_error: Option<AppErrorEnvelope>,
}

/// The defaults a launch starts from.
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

fn rejected(error: &AppError) -> SettingsOutcome {
    SettingsOutcome::Rejected {
        error: AppErrorEnvelope::from(error),
    }
}

impl SettingsRuntime {
    /// Loads the saved settings and builds the job scheduler they ask for.
    /// Also returns the defaults this launch starts from.
    pub(crate) fn start(
        config_dir: PathBuf,
        power: PowerManager,
    ) -> (Self, ManagedJobRegistry, PinnedDefaults) {
        let mut accepted = get_app_settings(&config_dir);
        let startup = startup_defaults(&accepted);
        // The defaults on screen are the ones in effect: after a launch from
        // pinned defaults, a later pin or save starts from those, not from the
        // last-used values the launch set aside.
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
        let state = State {
            accepted,
            ..State::default()
        };
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

    /// The AAC decoder in effect, for an import or a submission accepted now.
    pub(crate) async fn aac_decoder(&self) -> AacDecoder {
        self.inner.state.lock().await.accepted.aac_decoder
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
            default_acquisition_lane: state.accepted.default_acquisition_lane,
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

    /// Template typing owns its settings turn immediately, but writes only
    /// once the latest edit has paused. Reset follows every earlier choice.
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
        // Held for the whole intent: changes apply and write in the order asked.
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

    /// Puts `patch` in effect, then tries to save the settings in effect. A
    /// failed write leaves the change in effect and reported as unsaved.
    async fn accept(&self, state: &mut State, patch: AppSettingsPatch) -> SettingsOutcome {
        match state.accepted.clone().merge(patch) {
            Ok(next) => state.accepted = next,
            Err(error) => return rejected(&error),
        }
        self.apply_to_runtime(state);
        state.unsaved = true;
        self.write(state).await;
        SettingsOutcome::Applied
    }

    /// Writes the settings in effect if any change has not reached disk. The
    /// caller holds the settings turn, so writes stay in order.
    async fn write(&self, state: &mut State) {
        if !state.unsaved {
            return;
        }
        let config_dir = self.inner.config_dir.clone();
        let settings = state.accepted.clone();
        let written =
            tokio::task::spawn_blocking(move || save_app_settings(&config_dir, &settings))
                .await
                .unwrap_or_else(|error| {
                    Err(AppError::General(format!("Settings write failed: {error}")))
                });
        match written {
            Ok(()) => {
                state.unsaved = false;
                state.save_error = None;
            }
            Err(error) => {
                log::warn!("App settings were not saved: {error}");
                state.save_error = Some(AppErrorEnvelope::from(&error));
            }
        }
    }

    /// Writes anything still unsaved, as ABB closes. Returns why accepted
    /// settings are still not on disk, if they are not.
    pub(crate) async fn flush(&self) -> Option<AppErrorEnvelope> {
        let mut state = self.inner.state.lock().await;
        self.write(&mut state).await;
        state.save_error.clone()
    }

    /// The runtime side of the settings in effect.
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

    /// Captures the defaults in effect as the ones a later launch starts from.
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

    /// Returns every setting to its default and concurrency to automatic.
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
        };
        self.apply_to_runtime(state);
        self.write(state).await;
        SettingsOutcome::Applied
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
