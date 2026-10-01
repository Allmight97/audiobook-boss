//! Active work owns the idle-sleep hold; an open, idle app owns no OS resource.
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(target_os = "macos")]
type NativeHold = keepawake::KeepAwake;
#[cfg(not(target_os = "macos"))]
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

#[cfg(target_os = "macos")]
fn acquire_native_hold() -> Result<NativeHold, String> {
    keepawake::Builder::default()
        .idle(true)
        .display(false)
        .sleep(false)
        .reason("AudioBook Boss active work")
        .create()
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn acquire_native_hold() -> Result<NativeHold, String> {
    Err("idle-sleep prevention is currently supported on macOS".into())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn owned_assertions() -> Vec<String> {
        let output = std::process::Command::new("/usr/bin/pmset")
            .args(["-g", "assertions"])
            .output()
            .expect("read native power assertions");
        assert!(output.status.success());
        let pid = format!("pid {}(", std::process::id());
        String::from_utf8(output.stdout)
            .expect("pmset output is UTF-8")
            .lines()
            .filter(|line| line.contains(&pid) && line.contains("AudioBook Boss active work"))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn native_hold_tracks_overlapping_work_opt_out_and_unwinding() {
        let power = PowerManager::default();
        assert!(
            power.lock().hold.is_none(),
            "opening ABB must not prevent sleep"
        );
        let first = power.begin();
        let second = power.begin();
        assert!(power.lock().hold.is_some());
        let assertions = owned_assertions();
        assert_eq!(
            assertions.len(),
            1,
            "overlapping work shares one native hold"
        );
        assert!(assertions[0].contains("PreventUserIdleSystemSleep"));
        drop(first);
        assert!(
            power.lock().hold.is_some(),
            "other active work still needs its hold"
        );
        power.set_enabled(false);
        assert!(power.lock().hold.is_none(), "opt-out releases immediately");
        assert!(owned_assertions().is_empty());
        power.set_enabled(true);
        assert!(
            power.lock().hold.is_some(),
            "opt-in protects already-active work"
        );
        drop(second);
        assert!(power.lock().hold.is_none());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _work = power.begin();
            panic!("operation failed");
        }));
        assert!(result.is_err());
        assert_eq!(power.lock().active, 0);
        assert!(power.lock().hold.is_none());
        assert!(owned_assertions().is_empty());
    }
}
