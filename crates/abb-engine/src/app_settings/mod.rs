mod indexer_connection;
mod runtime;
mod storage;
mod types;

pub(crate) use indexer_connection::{api_key_header_value, normalize_base_url, validate_api_key};
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
