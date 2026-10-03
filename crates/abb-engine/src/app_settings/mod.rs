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
    AcquisitionLane, AppSettings, AppSettingsPatch, ConcurrencyPreference, EncoderDefaults,
    OutputDefaults, PinnedDefaults, StartupBehavior,
};

fn get_app_settings(config_dir: &Path) -> AppSettings {
    storage::load(config_dir)
}

fn save_app_settings(config_dir: &Path, settings: &AppSettings) -> Result<()> {
    storage::save(config_dir, settings)
}

#[cfg(test)]
mod contract_tests;
