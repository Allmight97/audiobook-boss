use super::*;
use crate::audio::{
    AacDecoder, AudioIntent, AudiobookFormat, ChannelConfig, EncoderType, FaacProfile,
};
use crate::output_artifact::NamingPreset;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use tempfile::TempDir;

const NEWEST_SAMPLE: &str = include_str!("saved_settings/press-quit-twice.toml");
const ALL_DEFAULTS_SAMPLE: &str = include_str!("saved_settings/all-defaults.toml");

fn without_comment_header(sample: &str) -> &str {
    let mut rest = sample;
    while rest.starts_with('#') {
        rest = rest.split_once('\n').map_or("", |(_, tail)| tail);
    }
    if rest.len() == sample.len() {
        return sample;
    }
    rest.strip_prefix('\n').unwrap_or(rest)
}

fn load_text(content: &str) -> storage::Loaded {
    let temp = TempDir::new().expect("temp dir");
    std::fs::write(temp.path().join("settings.toml"), content).expect("write settings");
    storage::load(temp.path())
}

fn saved_text(settings: &AppSettings) -> String {
    let temp = TempDir::new().expect("temp dir");
    storage::save(temp.path(), settings).expect("save");
    std::fs::read_to_string(temp.path().join("settings.toml")).expect("read saved file")
}

#[test]
fn saving_what_the_newest_sample_holds_writes_the_sample_bytes() {
    let loaded = load_text(NEWEST_SAMPLE);
    assert!(loaded.reset.is_empty());

    assert_eq!(
        saved_text(&loaded.settings),
        without_comment_header(NEWEST_SAMPLE)
    );
}

#[test]
fn saving_every_default_writes_the_all_defaults_sample_bytes() {
    assert_eq!(
        saved_text(&AppSettings::default()),
        without_comment_header(ALL_DEFAULTS_SAMPLE)
    );
}

#[test]
fn the_all_defaults_sample_loads_as_the_defaults_with_nothing_reset() {
    let loaded = load_text(ALL_DEFAULTS_SAMPLE);

    assert!(loaded.reset.is_empty());
    assert_eq!(loaded.settings, AppSettings::default());
    assert!(ALL_DEFAULTS_SAMPLE.contains("aac_decoder = \"auto\""));
    assert!(!ALL_DEFAULTS_SAMPLE.contains("[startup.pinned"));
}

#[derive(Serialize, serde::Deserialize)]
struct Cell<T> {
    value: T,
}

fn assert_file_spellings<T>(variants: &[T], spelling: fn(T) -> &'static str)
where
    T: Copy + Debug + PartialEq + Serialize + DeserializeOwned,
{
    for variant in variants.iter().copied() {
        let spelled = spelling(variant);
        let written = toml::to_string(&Cell { value: variant }).expect("write variant");
        assert_eq!(written, format!("value = \"{spelled}\"\n"), "{variant:?}");
        let read: Cell<T> = toml::from_str(&written).expect("read variant");
        assert_eq!(read.value, variant, "{spelled}");
    }
}

#[test]
fn audiobook_format_keeps_its_file_spellings() {
    fn spelling(variant: AudiobookFormat) -> &'static str {
        match variant {
            AudiobookFormat::M4b => "m4b",
            AudiobookFormat::M4aOpus => "m4aOpus",
            AudiobookFormat::MkaOpus => "mkaOpus",
        }
    }
    assert_file_spellings(
        &[
            AudiobookFormat::M4b,
            AudiobookFormat::M4aOpus,
            AudiobookFormat::MkaOpus,
        ],
        spelling,
    );
}

#[test]
fn audio_intent_keeps_its_file_spellings() {
    fn spelling(variant: AudioIntent) -> &'static str {
        match variant {
            AudioIntent::Auto => "auto",
            AudioIntent::Preserve => "preserve",
            AudioIntent::Encode => "encode",
        }
    }
    assert_file_spellings(
        &[
            AudioIntent::Auto,
            AudioIntent::Preserve,
            AudioIntent::Encode,
        ],
        spelling,
    );
}

#[test]
fn encoder_type_keeps_its_file_spellings() {
    fn spelling(variant: EncoderType) -> &'static str {
        match variant {
            EncoderType::Auto => "auto",
            EncoderType::AacAt => "aac_at",
            EncoderType::NativeAac => "native_aac",
            EncoderType::Faac => "faac",
            EncoderType::Opus => "opus",
        }
    }
    assert_file_spellings(
        &[
            EncoderType::Auto,
            EncoderType::AacAt,
            EncoderType::NativeAac,
            EncoderType::Faac,
            EncoderType::Opus,
        ],
        spelling,
    );
}

#[test]
fn channel_config_keeps_its_file_spellings() {
    fn spelling(variant: ChannelConfig) -> &'static str {
        match variant {
            ChannelConfig::Auto => "auto",
            ChannelConfig::Mono => "mono",
            ChannelConfig::Stereo => "stereo",
        }
    }
    assert_file_spellings(
        &[
            ChannelConfig::Auto,
            ChannelConfig::Mono,
            ChannelConfig::Stereo,
        ],
        spelling,
    );
}

#[test]
fn faac_profile_keeps_its_file_spellings() {
    fn spelling(variant: FaacProfile) -> &'static str {
        match variant {
            FaacProfile::Auto => "auto",
            FaacProfile::AacLc => "aac_lc",
            FaacProfile::HeAacV1 => "he_aac_v1",
        }
    }
    assert_file_spellings(
        &[FaacProfile::Auto, FaacProfile::AacLc, FaacProfile::HeAacV1],
        spelling,
    );
}

#[test]
fn aac_decoder_keeps_its_file_spellings() {
    fn spelling(variant: AacDecoder) -> &'static str {
        match variant {
            AacDecoder::Auto => "auto",
            AacDecoder::Faad => "faad",
        }
    }
    assert_file_spellings(&[AacDecoder::Auto, AacDecoder::Faad], spelling);
}

#[test]
fn acquisition_lane_keeps_its_file_spellings() {
    fn spelling(variant: AcquisitionLane) -> &'static str {
        match variant {
            AcquisitionLane::Audible => "audible",
            AcquisitionLane::Indexer => "indexer",
        }
    }
    assert_file_spellings(
        &[AcquisitionLane::Audible, AcquisitionLane::Indexer],
        spelling,
    );
}

#[test]
fn startup_behavior_keeps_its_file_spellings() {
    fn spelling(variant: StartupBehavior) -> &'static str {
        match variant {
            StartupBehavior::RememberLastState => "rememberLastState",
            StartupBehavior::PinnedDefaults => "pinnedDefaults",
        }
    }
    assert_file_spellings(
        &[
            StartupBehavior::RememberLastState,
            StartupBehavior::PinnedDefaults,
        ],
        spelling,
    );
}

#[test]
fn naming_preset_keeps_its_file_spellings() {
    fn spelling(variant: NamingPreset) -> &'static str {
        match variant {
            NamingPreset::AbsDefault => "absDefault",
            NamingPreset::CustomTemplate => "customTemplate",
        }
    }
    assert_file_spellings(
        &[NamingPreset::AbsDefault, NamingPreset::CustomTemplate],
        spelling,
    );
}

fn newest_sample_with(section: &str, value: toml::Value) -> storage::Loaded {
    let mut table: toml::Table = toml::from_str(NEWEST_SAMPLE).expect("sample is toml");
    table.insert(section.to_string(), value);
    load_text(&toml::to_string(&table).expect("write table"))
}

#[test]
fn a_general_that_is_not_a_section_resets_every_general_setting() {
    let loaded = newest_sample_with("general", 5.into());

    assert_eq!(
        loaded.reset,
        vec![
            SettingsUnit::KeepAwake,
            SettingsUnit::PressQuitTwice,
            SettingsUnit::JobLimit,
            SettingsUnit::AcquisitionLane,
            SettingsUnit::AacDecoder,
        ]
    );
    let defaults = AppSettings::default();
    let expected = AppSettings {
        keep_awake_while_working: defaults.keep_awake_while_working,
        press_quit_twice: defaults.press_quit_twice,
        max_concurrent_jobs: defaults.max_concurrent_jobs,
        default_acquisition_lane: defaults.default_acquisition_lane,
        aac_decoder: defaults.aac_decoder,
        ..load_text(NEWEST_SAMPLE).settings
    };
    assert_eq!(loaded.settings, expected);
}

#[test]
fn a_startup_that_is_not_a_section_resets_the_startup_choice_and_the_pinned_defaults() {
    let loaded = newest_sample_with("startup", "x".into());

    assert_eq!(
        loaded.reset,
        vec![SettingsUnit::StartupBehavior, SettingsUnit::PinnedDefaults]
    );
    let expected = AppSettings {
        startup_behavior: StartupBehavior::default(),
        pinned_defaults: None,
        ..load_text(NEWEST_SAMPLE).settings
    };
    assert_eq!(loaded.settings, expected);
}
