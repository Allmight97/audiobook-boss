mod edit_stamp;
mod indexer_connection;
mod runtime;
mod storage;
mod types;

pub(crate) use edit_stamp::{EditStamp, LatestEdit};
pub(crate) use indexer_connection::normalize_base_url;
pub use runtime::{
    ConcurrencySnapshot, SettingsIntent, SettingsOutcome, SettingsReply, SettingsSnapshot,
};
pub(crate) use runtime::{SettingsRun, SettingsRuntime};
pub use types::{
    AcquisitionLane, AppSettings, AppSettingsPatch, ConcurrencyPreference, EncoderDefaults,
    IndexerConnection, OutputDefaults, PinnedDefaults, SettingsRecovery, SettingsUnit,
    StartupBehavior,
};

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod saved_format_tests;
