mod storage;
mod types;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use crate::errors::{AppError, Result};

pub use types::{
    AcquisitionLane, AppSettings, AppSettingsPatch, AppSettingsRecoveryPlan,
    AppSettingsRecoveryResult, ConcurrencyPreference, EncoderDefaults, EncoderDefaultsScope,
    IncompatibleEncoderDefaults, OutputDefaults, PinnedDefaults, StartupBehavior,
    ToolchainPreferences,
};

static SETTINGS_UPDATE_LOCK: Mutex<()> = Mutex::new(());

pub fn get_app_settings(config_dir: &Path) -> Result<AppSettings> {
    storage::load(config_dir)
}

pub fn update_app_settings(config_dir: &Path, patch: AppSettingsPatch) -> Result<AppSettings> {
    let _guard = settings_update_lock()?;
    let current = storage::load(config_dir)?;
    let settings = current.merge(patch)?;
    storage::save(config_dir, &settings)?;
    Ok(settings)
}

pub fn reset_app_settings(config_dir: &Path) -> Result<AppSettings> {
    let _guard = settings_update_lock()?;
    storage::reset(config_dir)?;
    Ok(AppSettings::default())
}

pub fn get_app_settings_recovery(config_dir: &Path) -> Result<Option<AppSettingsRecoveryPlan>> {
    storage::recovery_plan(config_dir)
}

pub fn recover_app_settings(
    config_dir: &Path,
    expected: AppSettingsRecoveryPlan,
) -> Result<AppSettingsRecoveryResult> {
    let _guard = settings_update_lock()?;
    storage::recover(config_dir, expected)
}

fn settings_update_lock() -> Result<MutexGuard<'static, ()>> {
    SETTINGS_UPDATE_LOCK
        .lock()
        .map_err(|error| AppError::General(format!("App settings update lock poisoned: {error}")))
}

#[cfg(test)]
mod contract_tests;
