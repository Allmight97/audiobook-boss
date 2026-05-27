use crate::audio::{
    validate_encoder_settings, validate_sample_rate_config, BitrateMode, ChannelConfig,
    EncoderSettings, EncoderType, ExternalToolchainPreference, SampleRateConfig, ThreadSetting,
};
use crate::errors::{AppError, Result};
use crate::output_artifact::OutputNamingConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub max_concurrent_jobs: ConcurrencyPreference,
    pub encoder_defaults: EncoderDefaults,
    pub output_defaults: OutputDefaults,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub max_concurrent_jobs: Option<ConcurrencyPreference>,
    pub encoder_defaults: Option<EncoderDefaults>,
    pub output_defaults: Option<OutputDefaults>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderDefaults {
    pub settings: EncoderSettings,
    pub sample_rate: SampleRateConfig,
    pub external_toolchain: ExternalToolchainPreference,
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
                threads: ThreadSetting::Auto,
                twoloop: true,
            },
            sample_rate: SampleRateConfig::Auto,
            external_toolchain: ExternalToolchainPreference::default(),
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
        self.validate()?;
        Ok(self)
    }

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
        normalize_external_toolchain(&mut self.external_toolchain);
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

fn normalize_external_toolchain(preference: &mut ExternalToolchainPreference) {
    preference.override_path = preference
        .override_path
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
}
