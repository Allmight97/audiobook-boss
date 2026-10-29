use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::app_settings::{
    AcquisitionLane, AppSettings, ConcurrencyPreference, EncoderDefaults, OutputDefaults,
    PinnedDefaults, SettingsUnit, StartupBehavior,
};
use crate::audio::{
    AudioIntent, AudiobookFormat, BitrateMode, ChannelConfig, EncoderSettings, EncoderType,
    FaacProfile, SampleRateConfig,
};
use crate::errors::{AppError, Result};
use crate::output_artifact::{NamingPreset, OutputNamingConfig};

const SETTINGS_FILE_NAME: &str = "settings.toml";

pub(super) struct Loaded {
    pub(super) settings: AppSettings,
    pub(super) reset: Vec<SettingsUnit>,
    /// Present when the file could not be read or parsed: the name it was
    /// kept under, or why it could not be kept.
    pub(super) set_aside: Option<io::Result<Option<String>>>,
}

pub(super) fn load(config_dir: &Path) -> Loaded {
    let table = match std::fs::read_to_string(settings_path(config_dir)) {
        Ok(content) => toml::from_str::<toml::Table>(&content).map_err(|error| error.to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(error) => Err(error.to_string()),
    };
    match table {
        Ok(table) => recover(&table),
        Err(error) => {
            log::warn!("App settings file is unusable; using defaults: {error}");
            Loaded {
                settings: AppSettings::default(),
                reset: Vec::new(),
                set_aside: Some(set_aside(config_dir)),
            }
        }
    }
}

fn recover(table: &toml::Table) -> Loaded {
    let defaults = AppSettings::default();
    let mut reset = Vec::new();
    let settings = AppSettings {
        keep_awake_while_working: pick(
            &mut reset,
            SettingsUnit::KeepAwake,
            read(table, &["general", "keep_awake_while_working"], Ok),
        )
        .unwrap_or(defaults.keep_awake_while_working),
        max_concurrent_jobs: pick(
            &mut reset,
            SettingsUnit::JobLimit,
            read(table, &["general", "max_concurrent_jobs"], job_limit),
        )
        .unwrap_or(defaults.max_concurrent_jobs),
        default_acquisition_lane: pick(
            &mut reset,
            SettingsUnit::AcquisitionLane,
            read(table, &["general", "default_acquisition_lane"], Ok),
        )
        .unwrap_or(defaults.default_acquisition_lane),
        encoder_defaults: pick(
            &mut reset,
            SettingsUnit::EncoderDefaults,
            read::<Audio, _>(table, &["audio"], EncoderDefaults::try_from),
        )
        .unwrap_or(defaults.encoder_defaults),
        output_defaults: pick(
            &mut reset,
            SettingsUnit::OutputDefaults,
            read(table, &["output"], |output: Output| Ok(output.into())),
        )
        .unwrap_or(defaults.output_defaults),
        startup_behavior: pick(
            &mut reset,
            SettingsUnit::StartupBehavior,
            read(table, &["startup", "behavior"], Ok),
        )
        .unwrap_or(defaults.startup_behavior),
        pinned_defaults: pick(
            &mut reset,
            SettingsUnit::PinnedDefaults,
            read::<Pinned, _>(table, &["startup", "pinned"], PinnedDefaults::try_from),
        ),
    };
    Loaded {
        settings,
        reset,
        set_aside: None,
    }
}

fn read<S: DeserializeOwned, T>(
    table: &toml::Table,
    path: &[&str],
    convert: impl FnOnce(S) -> std::result::Result<T, String>,
) -> std::result::Result<Option<T>, String> {
    let Some((key, sections)) = path.split_last() else {
        return Ok(None);
    };
    let mut current = table;
    for section in sections {
        match current.get(*section) {
            None => return Ok(None),
            Some(toml::Value::Table(inner)) => current = inner,
            Some(_) => return Err(format!("[{section}] is not a section")),
        }
    }
    let Some(value) = current.get(*key) else {
        return Ok(None);
    };
    let saved = value
        .clone()
        .try_into::<S>()
        .map_err(|error| error.to_string())?;
    convert(saved).map(Some)
}

fn pick<T>(
    reset: &mut Vec<SettingsUnit>,
    unit: SettingsUnit,
    value: std::result::Result<Option<T>, String>,
) -> Option<T> {
    value.unwrap_or_else(|error| {
        log::warn!("Saved setting {unit:?} is not usable; using its default: {error}");
        reset.push(unit);
        None
    })
}

/// Keeps the settings file under a name no other file holds, so no later
/// save can destroy it. `None` when there is no file to keep.
pub(super) fn set_aside(config_dir: &Path) -> io::Result<Option<String>> {
    let path = settings_path(config_dir);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("{SETTINGS_FILE_NAME}.unusable-{stamp}");
    // A hard link fails when the name exists; a rename would replace it.
    match std::fs::hard_link(&path, config_dir.join(&name)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    log::warn!("Unusable settings file was kept as {name}");
    if let Err(error) = std::fs::remove_file(&path) {
        log::warn!("Unusable settings file could not be removed after it was kept: {error}");
    }
    Ok(Some(name))
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

#[derive(Serialize)]
struct SavedSettings {
    general: General,
    audio: Audio,
    output: Output,
    startup: Startup,
}

#[derive(Serialize)]
struct General {
    keep_awake_while_working: bool,
    max_concurrent_jobs: AutoOr<usize>,
    default_acquisition_lane: AcquisitionLane,
}

#[derive(Serialize)]
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
    vbr_quality: Option<u16>,
    channels: ChannelConfig,
    native_aac_speed: u8,
    faac_profile: FaacProfile,
}

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

fn job_limit(saved: AutoOr<usize>) -> std::result::Result<ConcurrencyPreference, String> {
    let preference = match saved {
        AutoOr::Auto(_) => ConcurrencyPreference::Auto,
        AutoOr::Value(count) => ConcurrencyPreference::Fixed(count),
    };
    preference.validate().map_err(|error| error.to_string())?;
    Ok(preference)
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

impl TryFrom<Pinned> for PinnedDefaults {
    type Error = String;

    fn try_from(pinned: Pinned) -> std::result::Result<Self, String> {
        Ok(Self {
            max_concurrent_jobs: job_limit(pinned.max_concurrent_jobs)?,
            encoder_defaults: pinned.audio.try_into()?,
            output_defaults: pinned.output.into(),
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
        let mut defaults = Self {
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
        };
        defaults.validate().map_err(|error| error.to_string())?;
        Ok(defaults)
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
        let mut defaults = Self {
            output_directory: output.folder,
            output_naming: OutputNamingConfig {
                preset: output.naming.preset,
                include_year: output.naming.include_year,
                custom_template: output.naming.custom_template,
            },
        };
        defaults.normalize();
        defaults
    }
}
