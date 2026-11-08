use crate::audio::{
    validate_encoder_settings, validate_sample_rate_config, AacDecoder, AudioIntent,
    AudiobookFormat, EncoderSettings, SampleRateConfig,
};
use crate::errors::{AppError, Result};
use crate::output_artifact::OutputNamingConfig;

#[derive(
    Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum AcquisitionLane {
    #[default]
    Audible,
    Indexer,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub keep_awake_while_working: bool,
    pub press_quit_twice: bool,
    pub max_concurrent_jobs: ConcurrencyPreference,
    pub encoder_defaults: EncoderDefaults,
    pub output_defaults: OutputDefaults,
    pub startup_behavior: StartupBehavior,
    pub pinned_defaults: Option<PinnedDefaults>,
    pub default_acquisition_lane: AcquisitionLane,
    pub aac_decoder: AacDecoder,
    pub indexer_connection: IndexerConnection,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub keep_awake_while_working: Option<bool>,
    pub press_quit_twice: Option<bool>,
    pub max_concurrent_jobs: Option<ConcurrencyPreference>,
    pub encoder_defaults: Option<EncoderDefaults>,
    pub output_defaults: Option<OutputDefaults>,
    pub startup_behavior: Option<StartupBehavior>,
    /// Set-only: pinning overwrites; reverting is switching `startup_behavior`
    /// back to `RememberLastState`, never unpinning.
    pub pinned_defaults: Option<PinnedDefaults>,
    pub default_acquisition_lane: Option<AcquisitionLane>,
    pub aac_decoder: Option<AacDecoder>,
    pub indexer_connection: Option<IndexerConnection>,
}

/// The Indexer server and categories a search uses. The API key lives in the
/// credential store, under the URL.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexerConnection {
    pub base_url: Option<String>,
    pub category_ids: Vec<u32>,
}

/// A setting that loading recovers on its own: a saved value this release
/// refuses resets only that setting.
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SettingsUnit {
    KeepAwake,
    PressQuitTwice,
    JobLimit,
    AcquisitionLane,
    AacDecoder,
    IndexerConnection,
    EncoderDefaults,
    OutputDefaults,
    StartupBehavior,
    PinnedDefaults,
}

impl SettingsUnit {
    /// Where the unit sits in `settings.toml`: loading reads it from here and
    /// names a reset by it.
    pub(super) fn path(self) -> &'static [&'static str] {
        match self {
            Self::KeepAwake => &["general", "keep_awake_while_working"],
            Self::PressQuitTwice => &["general", "press_quit_twice"],
            Self::JobLimit => &["general", "max_concurrent_jobs"],
            Self::AcquisitionLane => &["general", "default_acquisition_lane"],
            Self::AacDecoder => &["general", "aac_decoder"],
            Self::IndexerConnection => &["indexer"],
            Self::EncoderDefaults => &["audio"],
            Self::OutputDefaults => &["output"],
            Self::StartupBehavior => &["startup", "behavior"],
            Self::PinnedDefaults => &["startup", "pinned"],
        }
    }

    /// Part of the launch defaults, which a pinned launch replaces with the
    /// pinned ones.
    pub(super) fn launch_default(self) -> bool {
        match self {
            Self::JobLimit | Self::EncoderDefaults | Self::OutputDefaults => true,
            Self::KeepAwake
            | Self::PressQuitTwice
            | Self::AcquisitionLane
            | Self::AacDecoder
            | Self::IndexerConnection
            | Self::StartupBehavior
            | Self::PinnedDefaults => false,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsRecovery {
    /// Settings whose saved value was refused; they took their defaults.
    pub reset: Vec<SettingsUnit>,
    /// The name an unreadable settings file was kept under; every setting
    /// took its default.
    pub set_aside_as: Option<String>,
}

/// What launch hydration restores into the panels. The panels always keep
/// auto-persisting the top-level (last-used) values; this only chooses the
/// hydration source.
#[derive(
    Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum StartupBehavior {
    #[default]
    RememberLastState,
    /// Reopen with the user-pinned defaults; in-flight panel tweaks are
    /// ephemeral across restarts.
    PinnedDefaults,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PinnedDefaults {
    pub max_concurrent_jobs: ConcurrencyPreference,
    pub encoder_defaults: EncoderDefaults,
    pub output_defaults: OutputDefaults,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderDefaults {
    pub format: AudiobookFormat,
    pub intent: AudioIntent,
    pub settings: EncoderSettings,
    pub sample_rate: SampleRateConfig,
}

#[derive(
    Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct OutputDefaults {
    pub output_directory: Option<String>,
    pub output_naming: OutputNamingConfig,
}

#[derive(
    Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type,
)]
#[serde(tag = "mode", content = "value", rename_all = "camelCase")]
pub enum ConcurrencyPreference {
    #[default]
    Auto,
    Fixed(usize),
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            keep_awake_while_working: true,
            press_quit_twice: true,
            max_concurrent_jobs: ConcurrencyPreference::Auto,
            encoder_defaults: EncoderDefaults::default(),
            output_defaults: OutputDefaults::default(),
            startup_behavior: StartupBehavior::default(),
            pinned_defaults: None,
            default_acquisition_lane: AcquisitionLane::default(),
            aac_decoder: AacDecoder::default(),
            indexer_connection: IndexerConnection::default(),
        }
    }
}

impl Default for IndexerConnection {
    fn default() -> Self {
        Self {
            base_url: None,
            category_ids: vec![3000, 3030],
        }
    }
}

impl Default for EncoderDefaults {
    fn default() -> Self {
        Self {
            format: AudiobookFormat::default(),
            intent: AudioIntent::default(),
            settings: EncoderSettings::default(),
            sample_rate: SampleRateConfig::Auto,
        }
    }
}

impl AppSettings {
    pub(super) fn merge(mut self, patch: AppSettingsPatch) -> Result<Self> {
        let Self {
            keep_awake_while_working,
            press_quit_twice,
            max_concurrent_jobs,
            encoder_defaults,
            output_defaults,
            startup_behavior,
            pinned_defaults,
            default_acquisition_lane,
            aac_decoder,
            indexer_connection,
        } = &mut self;
        let AppSettingsPatch {
            keep_awake_while_working: patch_keep_awake_while_working,
            press_quit_twice: patch_press_quit_twice,
            max_concurrent_jobs: patch_max_concurrent_jobs,
            encoder_defaults: patch_encoder_defaults,
            output_defaults: patch_output_defaults,
            startup_behavior: patch_startup_behavior,
            pinned_defaults: patch_pinned_defaults,
            default_acquisition_lane: patch_default_acquisition_lane,
            aac_decoder: patch_aac_decoder,
            indexer_connection: patch_indexer_connection,
        } = patch;
        replace_with(keep_awake_while_working, patch_keep_awake_while_working);
        replace_with(press_quit_twice, patch_press_quit_twice);
        replace_with(max_concurrent_jobs, patch_max_concurrent_jobs);
        replace_with(encoder_defaults, patch_encoder_defaults);
        replace_with(output_defaults, patch_output_defaults);
        replace_with(startup_behavior, patch_startup_behavior);
        if patch_pinned_defaults.is_some() {
            *pinned_defaults = patch_pinned_defaults;
        }
        replace_with(default_acquisition_lane, patch_default_acquisition_lane);
        replace_with(aac_decoder, patch_aac_decoder);
        replace_with(indexer_connection, patch_indexer_connection);
        self.validate()?;
        Ok(self)
    }

    /// The job limit and the encoder and output defaults a launch starts
    /// from: the last used ones, or the pinned ones.
    pub(super) fn launch(&self) -> PinnedDefaults {
        PinnedDefaults {
            max_concurrent_jobs: self.max_concurrent_jobs,
            encoder_defaults: self.encoder_defaults.clone(),
            output_defaults: self.output_defaults.clone(),
        }
    }

    pub(super) fn set_launch(&mut self, launch: PinnedDefaults) {
        let PinnedDefaults {
            max_concurrent_jobs,
            encoder_defaults,
            output_defaults,
        } = launch;
        self.max_concurrent_jobs = max_concurrent_jobs;
        self.encoder_defaults = encoder_defaults;
        self.output_defaults = output_defaults;
    }

    fn validate(&mut self) -> Result<()> {
        let mut launch = self.launch();
        launch.validate()?;
        self.set_launch(launch);
        self.indexer_connection.validate()?;
        if let Some(pinned) = self.pinned_defaults.as_mut() {
            pinned.validate()?;
        }
        Ok(())
    }
}

fn replace_with<T>(target: &mut T, patch: Option<T>) {
    if let Some(value) = patch {
        *target = value;
    }
}

impl PinnedDefaults {
    fn validate(&mut self) -> Result<()> {
        self.max_concurrent_jobs.validate()?;
        self.encoder_defaults.validate()?;
        self.output_defaults.normalize();
        Ok(())
    }
}

impl ConcurrencyPreference {
    pub fn accepted(self, effective: usize) -> Self {
        match self {
            Self::Auto => Self::Auto,
            Self::Fixed(_) => Self::Fixed(effective),
        }
    }

    pub fn requested_value(self, default_value: usize) -> usize {
        match self {
            Self::Auto => default_value,
            Self::Fixed(value) => value,
        }
    }

    pub(super) fn validate(self) -> Result<()> {
        let capabilities = crate::processing::JobRegistry::max_concurrent_jobs_capabilities();
        match self {
            Self::Auto => Ok(()),
            Self::Fixed(value) if capabilities.fixed_options.contains(&value) => Ok(()),
            Self::Fixed(_) => Err(AppError::InvalidInput(format!(
                "Max concurrent jobs must be auto or a fixed value from {} to {}.",
                capabilities.fixed_min, capabilities.fixed_max
            ))),
        }
    }
}

impl EncoderDefaults {
    pub(super) fn validate(&mut self) -> Result<()> {
        if !self
            .format
            .spec()
            .encoders
            .contains(&self.settings.encoder_type)
        {
            return Err(AppError::InvalidInput(
                "The default encoder must match the selected output format.".into(),
            ));
        }

        validate_encoder_settings(&self.settings)?;
        validate_sample_rate_config(&self.sample_rate)?;
        Ok(())
    }
}

impl OutputDefaults {
    pub(super) fn normalize(&mut self) {
        self.output_directory = self
            .output_directory
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }
}
