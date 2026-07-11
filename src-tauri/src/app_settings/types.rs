use crate::audio::{
    validate_encoder_settings, validate_sample_rate_config, BitrateMode, ChannelConfig,
    EncoderSettings, EncoderType, SampleRateConfig,
};
use crate::errors::{AppError, Result};
use crate::output_artifact::OutputNamingConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub max_concurrent_jobs: ConcurrencyPreference,
    pub encoder_defaults: EncoderDefaults,
    pub output_defaults: OutputDefaults,
    #[serde(default)]
    pub toolchain: ToolchainPreferences,
    #[serde(default)]
    pub startup_behavior: StartupBehavior,
    #[serde(default)]
    pub density: DensityPreference,
    #[serde(default)]
    pub pinned_defaults: Option<PinnedDefaults>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub max_concurrent_jobs: Option<ConcurrencyPreference>,
    pub encoder_defaults: Option<EncoderDefaults>,
    pub output_defaults: Option<OutputDefaults>,
    pub toolchain: Option<ToolchainPreferences>,
    pub startup_behavior: Option<StartupBehavior>,
    pub density: Option<DensityPreference>,
    /// Set-only: pinning overwrites; reverting is switching `startup_behavior`
    /// back to `RememberLastState`, never unpinning.
    pub pinned_defaults: Option<PinnedDefaults>,
}

/// What launch hydration restores into the panels. The panels always keep
/// auto-persisting the top-level (last-used) values; this only chooses the
/// hydration source.
#[derive(
    Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum StartupBehavior {
    /// Today's behavior: reopen with whatever the panels last persisted.
    #[default]
    RememberLastState,
    /// Reopen with the user-pinned defaults; in-flight panel tweaks are
    /// ephemeral across restarts.
    PinnedDefaults,
}

/// The global UI layout density. Comfortable preserves the existing default;
/// compact reduces rows and padding for high-information workflows.
#[derive(
    Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum DensityPreference {
    #[default]
    Comfortable,
    Compact,
}

/// A deliberately captured snapshot of the panel-owned durable preferences.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PinnedDefaults {
    pub max_concurrent_jobs: ConcurrencyPreference,
    pub encoder_defaults: EncoderDefaults,
    pub output_defaults: OutputDefaults,
}

/// Durable toolchain preferences. Preference data only: the audio toolchain
/// owner probes and validates the path before any runtime use.
#[derive(
    Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainPreferences {
    /// User-selected external FFmpeg binary expected to expose `libfdk_aac`.
    pub external_ffmpeg_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderDefaults {
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

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, specta::Type)]
#[serde(tag = "mode", content = "value", rename_all = "camelCase")]
pub enum ConcurrencyPreference {
    Auto,
    Fixed(usize),
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: ConcurrencyPreference::Auto,
            encoder_defaults: EncoderDefaults::default(),
            output_defaults: OutputDefaults::default(),
            toolchain: ToolchainPreferences::default(),
            startup_behavior: StartupBehavior::default(),
            density: DensityPreference::default(),
            pinned_defaults: None,
        }
    }
}

impl Default for EncoderDefaults {
    fn default() -> Self {
        Self {
            settings: EncoderSettings {
                encoder_type: EncoderType::Auto,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Vbr(3),
                channels: ChannelConfig::Auto,
                afterburner: true,
            },
            sample_rate: SampleRateConfig::Auto,
        }
    }
}

impl AppSettings {
    pub(super) fn merge(mut self, patch: AppSettingsPatch) -> Result<Self> {
        if let Some(max_concurrent_jobs) = patch.max_concurrent_jobs {
            self.max_concurrent_jobs = max_concurrent_jobs;
        }
        if let Some(encoder_defaults) = patch.encoder_defaults {
            self.encoder_defaults = encoder_defaults;
        }
        if let Some(output_defaults) = patch.output_defaults {
            self.output_defaults = output_defaults;
        }
        if let Some(toolchain) = patch.toolchain {
            self.toolchain = toolchain;
        }
        if let Some(startup_behavior) = patch.startup_behavior {
            self.startup_behavior = startup_behavior;
        }
        if let Some(density) = patch.density {
            self.density = density;
        }
        if let Some(pinned_defaults) = patch.pinned_defaults {
            self.pinned_defaults = Some(pinned_defaults);
        }
        self.validate()?;
        Ok(self)
    }

    fn validate(&mut self) -> Result<()> {
        self.max_concurrent_jobs.validate()?;
        self.encoder_defaults.validate()?;
        self.output_defaults.normalize();
        self.toolchain.normalize();
        if let Some(pinned) = self.pinned_defaults.as_mut() {
            // Same validators as the live values: a stale or hand-edited
            // pinned snapshot must never brick launch hydration.
            pinned.max_concurrent_jobs.validate()?;
            pinned.encoder_defaults.validate()?;
            pinned.output_defaults.normalize();
        }
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

    fn validate(self) -> Result<()> {
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
    fn validate(&mut self) -> Result<()> {
        validate_encoder_settings(&self.settings)?;
        validate_sample_rate_config(&self.sample_rate)?;
        Ok(())
    }
}

impl OutputDefaults {
    fn normalize(&mut self) {
        self.output_directory = self
            .output_directory
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }
}

impl ToolchainPreferences {
    fn normalize(&mut self) {
        self.external_ffmpeg_path = self
            .external_ffmpeg_path
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }
}
