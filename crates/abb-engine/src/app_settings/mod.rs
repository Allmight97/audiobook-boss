mod runtime;
mod storage;
mod types;

use std::path::Path;

use crate::errors::Result;

pub use runtime::{
    ConcurrencySnapshot, SettingsIntent, SettingsOutcome, SettingsReply, SettingsSnapshot,
};
pub(crate) use runtime::{SettingsRun, SettingsRuntime};
pub use types::{
    AcquisitionLane, AppSettings, AppSettingsPatch, AppSettingsRecoveryPlan,
    AppSettingsRecoveryResult, ConcurrencyPreference, EncoderDefaults, EncoderDefaultsScope,
    IncompatibleEncoderDefaults, OutputDefaults, PinnedDefaults, StartupBehavior,
};

fn get_app_settings(config_dir: &Path) -> Result<AppSettings> {
    storage::load(config_dir)
}

fn update_app_settings(config_dir: &Path, patch: AppSettingsPatch) -> Result<AppSettings> {
    let current = storage::load(config_dir)?;
    let settings = current.merge(patch)?;
    storage::save(config_dir, &settings)?;
    Ok(settings)
}

fn reset_app_settings(config_dir: &Path) -> Result<AppSettings> {
    storage::reset(config_dir)?;
    Ok(AppSettings::default())
}

fn get_app_settings_recovery(config_dir: &Path) -> Result<Option<AppSettingsRecoveryPlan>> {
    storage::recovery_plan(config_dir)
}

fn recover_app_settings(
    config_dir: &Path,
    expected: AppSettingsRecoveryPlan,
) -> Result<AppSettingsRecoveryResult> {
    storage::recover(config_dir, expected)
}

#[cfg(test)]
mod contract_tests;
