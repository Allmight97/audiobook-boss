//! The settings file: `settings.toml` in the host's config folder, one
//! section per area. Its shape is defined here, apart from the IPC types, so
//! a change to what hosts send cannot silently change what a saved file means.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app_settings::{
    AcquisitionLane, AppSettings, AppSettingsPatch, ConcurrencyPreference, EncoderDefaults,
    OutputDefaults, PinnedDefaults, StartupBehavior,
};
use crate::audio::{
    AacDecoder, AudioIntent, AudiobookFormat, BitrateMode, ChannelConfig, EncoderSettings,
    EncoderType, FaacProfile, SampleRateConfig,
};
use crate::errors::{AppError, Result};
use crate::output_artifact::{NamingPreset, OutputNamingConfig};

const SETTINGS_FILE_NAME: &str = "settings.toml";

/// The saved settings, or the defaults when there are none. A file damaged
/// outside ABB is not used and is replaced by the next save.
pub(super) fn load(config_dir: &Path) -> AppSettings {
    let content = match std::fs::read_to_string(settings_path(config_dir)) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return AppSettings::default(),
        Err(error) => {
            log::warn!("App settings could not be read; using defaults: {error}");
            return AppSettings::default();
        }
    };
    toml::from_str::<SavedSettings>(&content)
        .map_err(|error| error.to_string())
        .and_then(AppSettings::try_from)
        .and_then(|settings| {
            settings
                .merge(AppSettingsPatch::default())
                .map_err(|error| error.to_string())
        })
        .unwrap_or_else(|error| {
            log::warn!("App settings were not usable; using defaults: {error}");
            AppSettings::default()
        })
}

pub(super) fn save(config_dir: &Path, settings: &AppSettings) -> Result<()> {
    let content = toml::to_string(&SavedSettings::from(settings))
        .map_err(|error| AppError::General(format!("Failed to write app settings: {error}")))?;
    crate::file_replace::write_atomically(&settings_path(config_dir), content.as_bytes())?;
    Ok(())
}

fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE_NAME)
}

#[derive(Serialize, Deserialize)]
struct SavedSettings {
    general: General,
    audio: Audio,
    output: Output,
    startup: Startup,
}

#[derive(Serialize, Deserialize)]
struct General {
    keep_awake_while_working: bool,
    max_concurrent_jobs: AutoOr<usize>,
    default_acquisition_lane: AcquisitionLane,
    /// Added after the first saved shape; files without it load as Auto.
    #[serde(default)]
    aac_decoder: AacDecoder,
}

#[derive(Serialize, Deserialize)]
struct Startup {
    behavior: StartupBehavior,
    pinned: Option<Pinned>,
}

#[derive(Serialize, Deserialize)]
struct Pinned {
    max_concurrent_jobs: AutoOr<usize>,
    audio: Audio,
    output: Output,
}

#[derive(Serialize, Deserialize)]
struct Audio {
    format: AudiobookFormat,
    intent: AudioIntent,
    sample_rate: AutoOr<u32>,
    encoder: Encoder,
}

#[derive(Serialize, Deserialize)]
struct Encoder {
    #[serde(rename = "type")]
    encoder_type: EncoderType,
    bitrate_kbps: u16,
    bitrate_mode: Mode,
    /// Present only with `bitrate_mode = "vbr"`.
    vbr_quality: Option<u16>,
    channels: ChannelConfig,
    native_aac_speed: u8,
    faac_profile: FaacProfile,
}

/// `"auto"` or a number, as one plain value.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
enum AutoOr<T> {
    Auto(AutoWord),
    Value(T),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AutoWord {
    Auto,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Cbr,
    Cvbr,
    Abr,
    Vbr,
    VbrTarget,
}

impl From<ConcurrencyPreference> for AutoOr<usize> {
    fn from(preference: ConcurrencyPreference) -> Self {
        match preference {
            ConcurrencyPreference::Auto => Self::Auto(AutoWord::Auto),
            ConcurrencyPreference::Fixed(count) => Self::Value(count),
        }
    }
}

impl From<AutoOr<usize>> for ConcurrencyPreference {
    fn from(saved: AutoOr<usize>) -> Self {
        match saved {
            AutoOr::Auto(_) => Self::Auto,
            AutoOr::Value(count) => Self::Fixed(count),
        }
    }
}

impl From<SampleRateConfig> for AutoOr<u32> {
    fn from(rate: SampleRateConfig) -> Self {
        match rate {
            SampleRateConfig::Auto => Self::Auto(AutoWord::Auto),
            SampleRateConfig::Explicit(hz) => Self::Value(hz),
        }
    }
}

impl From<AutoOr<u32>> for SampleRateConfig {
    fn from(saved: AutoOr<u32>) -> Self {
        match saved {
            AutoOr::Auto(_) => Self::Auto,
            AutoOr::Value(hz) => Self::Explicit(hz),
        }
    }
}

fn split_mode(mode: BitrateMode) -> (Mode, Option<u16>) {
    match mode {
        BitrateMode::Cbr => (Mode::Cbr, None),
        BitrateMode::Cvbr => (Mode::Cvbr, None),
        BitrateMode::Abr => (Mode::Abr, None),
        BitrateMode::Vbr(quality) => (Mode::Vbr, Some(quality)),
        BitrateMode::VbrTarget => (Mode::VbrTarget, None),
    }
}

/// A VBR mode saved without its quality is not a usable choice.
fn join_mode(mode: Mode, vbr_quality: Option<u16>) -> std::result::Result<BitrateMode, String> {
    Ok(match mode {
        Mode::Cbr => BitrateMode::Cbr,
        Mode::Cvbr => BitrateMode::Cvbr,
        Mode::Abr => BitrateMode::Abr,
        Mode::Vbr => BitrateMode::Vbr(vbr_quality.ok_or("VBR quality is missing")?),
        Mode::VbrTarget => BitrateMode::VbrTarget,
    })
}

#[derive(Serialize, Deserialize)]
struct Output {
    folder: Option<String>,
    naming: Naming,
}

#[derive(Serialize, Deserialize)]
struct Naming {
    preset: NamingPreset,
    include_year: bool,
    custom_template: Option<String>,
}

impl From<&AppSettings> for SavedSettings {
    fn from(settings: &AppSettings) -> Self {
        Self {
            general: General {
                keep_awake_while_working: settings.keep_awake_while_working,
                max_concurrent_jobs: settings.max_concurrent_jobs.into(),
                default_acquisition_lane: settings.default_acquisition_lane,
                aac_decoder: settings.aac_decoder,
            },
            audio: Audio::from(&settings.encoder_defaults),
            output: Output::from(&settings.output_defaults),
            startup: Startup {
                behavior: settings.startup_behavior,
                pinned: settings.pinned_defaults.as_ref().map(|pinned| Pinned {
                    max_concurrent_jobs: pinned.max_concurrent_jobs.into(),
                    audio: Audio::from(&pinned.encoder_defaults),
                    output: Output::from(&pinned.output_defaults),
                }),
            },
        }
    }
}

impl TryFrom<SavedSettings> for AppSettings {
    type Error = String;

    fn try_from(saved: SavedSettings) -> std::result::Result<Self, String> {
        Ok(Self {
            keep_awake_while_working: saved.general.keep_awake_while_working,
            max_concurrent_jobs: saved.general.max_concurrent_jobs.into(),
            default_acquisition_lane: saved.general.default_acquisition_lane,
            aac_decoder: saved.general.aac_decoder,
            encoder_defaults: saved.audio.try_into()?,
            output_defaults: saved.output.into(),
            startup_behavior: saved.startup.behavior,
            pinned_defaults: saved
                .startup
                .pinned
                .map(|pinned| -> std::result::Result<_, String> {
                    Ok(PinnedDefaults {
                        max_concurrent_jobs: pinned.max_concurrent_jobs.into(),
                        encoder_defaults: pinned.audio.try_into()?,
                        output_defaults: pinned.output.into(),
                    })
                })
                .transpose()?,
        })
    }
}

impl From<&EncoderDefaults> for Audio {
    fn from(defaults: &EncoderDefaults) -> Self {
        let settings = &defaults.settings;
        let (bitrate_mode, vbr_quality) = split_mode(settings.bitrate_mode);
        Self {
            format: defaults.format,
            intent: defaults.intent,
            sample_rate: defaults.sample_rate.into(),
            encoder: Encoder {
                encoder_type: settings.encoder_type,
                bitrate_kbps: settings.bitrate_kbps,
                bitrate_mode,
                vbr_quality,
                channels: settings.channels,
                native_aac_speed: settings.native_aac_speed,
                faac_profile: settings.faac_profile,
            },
        }
    }
}

impl TryFrom<Audio> for EncoderDefaults {
    type Error = String;

    fn try_from(audio: Audio) -> std::result::Result<Self, String> {
        let encoder = audio.encoder;
        Ok(Self {
            format: audio.format,
            intent: audio.intent,
            sample_rate: audio.sample_rate.into(),
            settings: EncoderSettings {
                encoder_type: encoder.encoder_type,
                bitrate_kbps: encoder.bitrate_kbps,
                bitrate_mode: join_mode(encoder.bitrate_mode, encoder.vbr_quality)?,
                channels: encoder.channels,
                native_aac_speed: encoder.native_aac_speed,
                faac_profile: encoder.faac_profile,
            },
        })
    }
}

impl From<&OutputDefaults> for Output {
    fn from(defaults: &OutputDefaults) -> Self {
        let naming = &defaults.output_naming;
        Self {
            folder: defaults.output_directory.clone(),
            naming: Naming {
                preset: naming.preset,
                include_year: naming.include_year,
                custom_template: naming.custom_template.clone(),
            },
        }
    }
}

impl From<Output> for OutputDefaults {
    fn from(output: Output) -> Self {
        Self {
            output_directory: output.folder,
            output_naming: OutputNamingConfig {
                preset: output.naming.preset,
                include_year: output.naming.include_year,
                custom_template: output.naming.custom_template,
            },
        }
    }
}
