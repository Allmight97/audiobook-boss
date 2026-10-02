//! The settings in effect for this run, and whether they are on disk yet.
//!
//! A preference a panel has accepted stays in effect when writing it fails;
//! the unsaved part is kept, coalesced by field, and written on retry. One
//! lock serializes every change, so an older write can never report a newer
//! choice as saved.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use super::{
    get_app_settings, get_app_settings_recovery, recover_app_settings, reset_app_settings,
    update_app_settings, AcquisitionLane, AppSettings, AppSettingsPatch, AppSettingsRecoveryPlan,
    ConcurrencyPreference, EncoderDefaults, OutputDefaults, PinnedDefaults, StartupBehavior,
};
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
    SetStartupBehavior {
        behavior: StartupBehavior,
    },
    /// Captures the current defaults as the ones a later launch starts from.
    PinCurrentDefaults,
    /// Writes accepted changes that an earlier write failed to save.
    Retry,
    /// Returns every setting to its default. Refused while exports run.
    Reset,
    /// Applies a reviewed recovery of settings this version cannot read.
    Recover {
        expected: AppSettingsRecoveryPlan,
    },
    /// Reads the saved settings again after a failed load.
    Reload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingsOutcome {
    Applied,
    /// Nothing changed.
    Rejected {
        error: AppErrorEnvelope,
    },
    /// The saved settings were recovered; the original file is in `backup_file_name`.
    #[serde(rename_all = "camelCase")]
    Recovered {
        backup_file_name: String,
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
    /// The settings in effect. Absent while the saved file cannot be read.
    pub settings: Option<AppSettings>,
    /// Why the saved file could not be read.
    pub load_error: Option<AppErrorEnvelope>,
    /// A recovery the user may apply to make the saved file readable.
    pub recovery: Option<AppSettingsRecoveryPlan>,
    /// Why accepted changes are not on disk yet. Absent when all are saved.
    pub save_error: Option<AppErrorEnvelope>,
    pub concurrency: ConcurrencySnapshot,
    /// The defaults a host shows at launch: the pinned ones when the user
    /// chose that and has pinned some, otherwise the last used.
    pub startup_defaults: Option<PinnedDefaults>,
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
    reply: tokio::task::JoinHandle<(SettingsReply, bool)>,
}

impl SettingsRun {
    pub(crate) async fn finish(self) -> (SettingsReply, bool) {
        match self.reply.await {
            Ok(reply) => reply,
            Err(error) => (
                SettingsReply {
                    outcome: rejected(&AppError::General(format!("Settings work failed: {error}"))),
                    snapshot: self.settings.snapshot().await,
                },
                false,
            ),
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
    accepted: Option<AppSettings>,
    load_error: Option<AppErrorEnvelope>,
    recovery: Option<AppSettingsRecoveryPlan>,
    /// Accepted changes not yet on disk.
    unsaved: AppSettingsPatch,
    save_error: Option<AppErrorEnvelope>,
    /// The concurrency choice in effect. A launch from pinned defaults takes
    /// the pinned choice without rewriting the last-used one.
    concurrency: Option<ConcurrencyPreference>,
    hydration_pending: bool,
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

impl State {
    fn load(&mut self, config_dir: &Path) {
        match get_app_settings(config_dir) {
            Ok(settings) => {
                match settings.merge(self.unsaved.clone()) {
                    Ok(settings) => self.accepted = Some(settings),
                    Err(error) => {
                        self.load_error = Some(AppErrorEnvelope::from(&error));
                        return;
                    }
                }
                self.load_error = None;
                self.recovery = None;
                self.hydration_pending = true;
            }
            Err(error) => {
                self.accepted = None;
                self.load_error = Some(AppErrorEnvelope::from(&error));
                self.recovery = get_app_settings_recovery(config_dir).unwrap_or_else(|error| {
                    log::warn!("Settings recovery check failed: {error}");
                    None
                });
            }
        }
    }
}

impl SettingsRuntime {
    /// Loads the saved settings and builds the job scheduler they ask for.
    /// Settings that cannot be read leave the runtime defaults in effect.
    /// Also returns the defaults this launch starts from, when settings loaded.
    pub(crate) fn start(
        config_dir: PathBuf,
        power: PowerManager,
    ) -> (Self, ManagedJobRegistry, Option<PinnedDefaults>) {
        let mut state = State::default();
        state.load(&config_dir);
        if let Some(error) = &state.load_error {
            log::warn!(
                "Startup app settings hydration failed; using runtime defaults: {}",
                error.message
            );
        }
        let startup = state.accepted.as_ref().map(startup_defaults);
        let jobs: ManagedJobRegistry = Arc::new(
            match startup
                .as_ref()
                .map(|defaults| defaults.max_concurrent_jobs)
            {
                Some(ConcurrencyPreference::Fixed(value)) => JobRegistry::new(value),
                Some(ConcurrencyPreference::Auto) | None => JobRegistry::auto(),
            },
        );
        log::info!(
            "Job registry initialized: max_concurrent = {}",
            jobs.max_concurrent()
        );
        state.concurrency = startup
            .as_ref()
            .map(|defaults| defaults.max_concurrent_jobs);
        if let Some(settings) = &state.accepted {
            power.set_enabled(settings.keep_awake_while_working);
        }
        state.hydration_pending = false;
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
            load_error: state.load_error.clone(),
            recovery: state.recovery.clone(),
            save_error: state.save_error.clone(),
            concurrency: ConcurrencySnapshot {
                preference: state.concurrency.unwrap_or(ConcurrencyPreference::Auto),
                effective: self.inner.jobs.max_concurrent(),
                capabilities: JobRegistry::max_concurrent_jobs_capabilities(),
            },
            startup_defaults: state.accepted.as_ref().map(startup_defaults),
            default_acquisition_lane: state
                .unsaved
                .default_acquisition_lane
                .or(state
                    .accepted
                    .as_ref()
                    .map(|settings| settings.default_acquisition_lane))
                .unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub(crate) async fn dispatch(&self, intent: SettingsIntent) -> SettingsReply {
        self.begin(intent, &crate::engine::EngineTasks::default())
            .finish()
            .await
            .0
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
                (
                    SettingsReply {
                        outcome: SettingsOutcome::Applied,
                        snapshot: settings.snapshot().await,
                    },
                    false,
                )
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

    async fn apply_ordered(&self, intent: SettingsIntent) -> (SettingsReply, bool) {
        // Held for the whole intent: changes apply and write in the order asked.
        let mut state = self.inner.state.lock().await;
        let before = self.snapshot_of(&state).startup_defaults;
        let pending = state.hydration_pending;
        let outcome = self.apply(&mut state, intent).await;
        state.revision += 1;
        let reply = SettingsReply {
            outcome,
            snapshot: self.snapshot_of(&state),
        };
        let changed =
            reply.snapshot.startup_defaults != before || (pending && !state.hydration_pending);
        (reply, changed)
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
                self.write_now(
                    state,
                    AppSettingsPatch {
                        keep_awake_while_working: Some(enabled),
                        ..Default::default()
                    },
                )
                .await
            }
            SettingsIntent::SetStartupBehavior { behavior } => {
                self.write_now(
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
                self.write_unsaved(state).await;
                SettingsOutcome::Applied
            }
            SettingsIntent::Reset => self.reset(state).await,
            SettingsIntent::Recover { expected } => self.recover(state, expected).await,
            SettingsIntent::Reload => {
                if state.accepted.is_none() {
                    state.load(&self.inner.config_dir);
                }
                if state.hydration_pending {
                    if let Err(error) = self.apply_loaded_runtime(state).await {
                        return rejected(&error);
                    }
                    self.write_unsaved(state).await;
                }
                SettingsOutcome::Applied
            }
        }
    }

    /// Puts `patch` in effect, then tries to save it. A failed write leaves
    /// it in effect and unsaved.
    async fn accept(&self, state: &mut State, patch: AppSettingsPatch) -> SettingsOutcome {
        if let Some(accepted) = &state.accepted {
            match accepted.clone().merge(patch.clone()) {
                Ok(next) => state.accepted = Some(next),
                Err(error) => return rejected(&error),
            }
        }
        state.unsaved.absorb(patch);
        self.write_unsaved(state).await;
        SettingsOutcome::Applied
    }

    /// Runs settings file I/O on a blocking thread; the caller still holds
    /// the settings turn, so writes stay in order.
    async fn on_disk<T: Send + 'static>(
        &self,
        work: impl FnOnce(&std::path::Path) -> crate::errors::Result<T> + Send + 'static,
    ) -> crate::errors::Result<T> {
        let config_dir = self.inner.config_dir.clone();
        tokio::task::spawn_blocking(move || work(&config_dir))
            .await
            .map_err(|error| AppError::General(format!("Settings write failed: {error}")))?
    }

    async fn write_unsaved(&self, state: &mut State) {
        if state.unsaved.is_empty() {
            state.save_error = None;
            return;
        }
        let unsaved = state.unsaved.clone();
        match self
            .on_disk(move |dir| update_app_settings(dir, unsaved))
            .await
        {
            Ok(settings) => {
                state.accepted = Some(settings);
                state.unsaved = AppSettingsPatch::default();
                state.save_error = None;
                state.load_error = None;
                state.recovery = None;
                self.apply_to_runtime(state);
            }
            Err(error) => state.save_error = Some(AppErrorEnvelope::from(&error)),
        }
    }

    /// Applies `patch` only if it reaches disk, together with anything still
    /// unsaved.
    async fn write_now(&self, state: &mut State, patch: AppSettingsPatch) -> SettingsOutcome {
        let mut write = state.unsaved.clone();
        write.absorb(patch);
        match self
            .on_disk(move |dir| update_app_settings(dir, write))
            .await
        {
            Ok(settings) => {
                state.accepted = Some(settings);
                state.unsaved = AppSettingsPatch::default();
                state.save_error = None;
                state.load_error = None;
                state.recovery = None;
                self.apply_to_runtime(state);
                SettingsOutcome::Applied
            }
            Err(error) => rejected(&error),
        }
    }

    /// The runtime side of settings that reached disk.
    fn apply_to_runtime(&self, state: &State) {
        if let Some(settings) = &state.accepted {
            self.inner
                .power
                .set_enabled(settings.keep_awake_while_working);
        }
    }

    async fn apply_loaded_runtime(&self, state: &mut State) -> crate::errors::Result<()> {
        let Some(settings) = &state.accepted else {
            return Ok(());
        };
        let preference = state
            .concurrency
            .unwrap_or_else(|| startup_defaults(settings).max_concurrent_jobs);
        let requested = preference.requested_value(JobRegistry::default_max());
        let effective = if self.inner.jobs.max_concurrent() == requested {
            requested
        } else {
            self.inner.jobs.update_max_concurrent(requested).await?
        };
        state.concurrency = Some(preference.accepted(effective));
        self.apply_to_runtime(state);
        state.hydration_pending = false;
        Ok(())
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
        state.concurrency = Some(accepted);
        self.accept(
            state,
            AppSettingsPatch {
                max_concurrent_jobs: Some(accepted),
                ..Default::default()
            },
        )
        .await
    }

    /// Pinning captures what is on disk, so unsaved changes must save first.
    async fn pin_current_defaults(&self, state: &mut State) -> SettingsOutcome {
        self.write_unsaved(state).await;
        if let Some(error) = &state.save_error {
            return rejected(&AppError::InvalidInput(format!(
                "Save current settings before pinning defaults. {}",
                error.message
            )));
        }
        let Some(current) = &state.accepted else {
            return SettingsOutcome::Rejected {
                error: state.load_error.clone().unwrap_or_else(|| {
                    AppErrorEnvelope::from(&AppError::General(
                        "App settings are not loaded.".to_string(),
                    ))
                }),
            };
        };
        let pinned = PinnedDefaults {
            max_concurrent_jobs: current.max_concurrent_jobs,
            encoder_defaults: current.encoder_defaults.clone(),
            output_defaults: current.output_defaults.clone(),
        };
        self.write_now(
            state,
            AppSettingsPatch {
                pinned_defaults: Some(pinned),
                ..Default::default()
            },
        )
        .await
    }

    /// Resets saved settings and returns concurrency to automatic. A failed
    /// reset restores the previous concurrency and keeps unsaved changes
    /// retryable.
    async fn reset(&self, state: &mut State) -> SettingsOutcome {
        let jobs = &self.inner.jobs;
        let previous = jobs.max_concurrent();
        if jobs.reset_to_auto().await.is_err() {
            return rejected(&AppError::InvalidInput(
                "Settings can't be reset while exports are running. Try again when they finish."
                    .to_string(),
            ));
        }
        match self.on_disk(reset_app_settings).await {
            Ok(settings) => {
                *state = State {
                    revision: state.revision,
                    concurrency: Some(settings.max_concurrent_jobs),
                    accepted: Some(settings),
                    ..State::default()
                };
                self.apply_to_runtime(state);
                SettingsOutcome::Applied
            }
            Err(error) => {
                if let Err(rollback) = jobs.update_max_concurrent(previous).await {
                    log::warn!(
                        "Failed to roll back max concurrency after settings reset failed: {rollback}"
                    );
                }
                rejected(&error)
            }
        }
    }

    /// Applies the reviewed recovery, then saves what the session had
    /// accepted while the file was unreadable.
    async fn recover(
        &self,
        state: &mut State,
        expected: AppSettingsRecoveryPlan,
    ) -> SettingsOutcome {
        let result = match self
            .on_disk(move |dir| recover_app_settings(dir, expected))
            .await
        {
            Ok(result) => result,
            Err(error) => return rejected(&error),
        };
        state.accepted = match result.settings.merge(state.unsaved.clone()) {
            Ok(settings) => Some(settings),
            Err(error) => return rejected(&error),
        };
        state.load_error = None;
        state.recovery = None;
        state.hydration_pending = true;
        if let Err(error) = self.apply_loaded_runtime(state).await {
            return rejected(&error);
        }
        self.write_unsaved(state).await;
        SettingsOutcome::Recovered {
            backup_file_name: result.backup_file_name,
        }
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
