//! Active work owns the idle-sleep hold; an open, idle app owns no OS resource.
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(any(target_os = "macos", target_os = "linux"))]
type NativeHold = keepawake::KeepAwake;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
struct NativeHold;

#[derive(Clone, Default)]
pub(crate) struct PowerManager(Arc<Mutex<PowerState>>);

struct PowerState {
    enabled: bool,
    active: usize,
    hold: Option<NativeHold>,
}

impl Default for PowerState {
    fn default() -> Self {
        Self {
            enabled: true,
            active: 0,
            hold: None,
        }
    }
}

impl PowerManager {
    pub(crate) fn begin(&self) -> ActiveWork {
        let mut state = self.lock();
        state.active += 1;
        state.reconcile();
        ActiveWork(self.clone())
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        let mut state = self.lock();
        state.enabled = enabled;
        state.reconcile();
    }

    fn lock(&self) -> MutexGuard<'_, PowerState> {
        // Releasing a native hold must also work during panic unwinding.
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(crate) struct ActiveWork(PowerManager);

impl Drop for ActiveWork {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.active -= 1;
        state.reconcile();
    }
}

impl PowerState {
    fn reconcile(&mut self) {
        if !self.enabled || self.active == 0 {
            if self.hold.take().is_some() {
                log::info!("power idle_sleep_hold=released active_work={}", self.active);
            }
        } else if self.hold.is_none() {
            match acquire_native_hold() {
                Ok(hold) => {
                    self.hold = Some(hold);
                    log::info!("power idle_sleep_hold=acquired display_sleep=allowed");
                }
                Err(error) => log::warn!(
                    "power idle_sleep_hold=unavailable; active work may be interrupted by idle sleep: {error}"
                ),
            }
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn acquire_native_hold() -> Result<NativeHold, String> {
    keepawake::Builder::default()
        .idle(true)
        .display(false)
        .sleep(false)
        .reason("AudioBook Boss active work")
        .create()
        .map_err(|error| error.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn acquire_native_hold() -> Result<NativeHold, String> {
    Err("idle-sleep prevention is supported on macOS and Linux".into())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "power_tests.rs"]
mod tests;
