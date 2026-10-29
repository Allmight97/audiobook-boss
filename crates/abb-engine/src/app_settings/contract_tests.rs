use super::*;
use crate::audio::{
    AudioIntent, AudiobookFormat, BitrateMode, ChannelConfig, EncoderSettings, EncoderType,
    FaacProfile, SampleRateConfig,
};
use crate::output_artifact::{NamingPreset, OutputNamingConfig};
use std::path::Path;
use tempfile::TempDir;

/// Every setting at a value other than its default, so a field that stops
/// loading cannot hide behind a default.
fn chosen_settings() -> AppSettings {
    AppSettings {
        keep_awake_while_working: false,
        max_concurrent_jobs: ConcurrencyPreference::Fixed(2),
        default_acquisition_lane: AcquisitionLane::Indexer,
        encoder_defaults: EncoderDefaults {
            format: AudiobookFormat::Mp3,
            intent: AudioIntent::Encode,
            sample_rate: SampleRateConfig::Explicit(44100),
            settings: EncoderSettings {
                encoder_type: EncoderType::Faac,
                bitrate_kbps: 96,
                bitrate_mode: BitrateMode::Abr,
                channels: ChannelConfig::Mono,
                native_aac_speed: 2,
                faac_profile: FaacProfile::AacLc,
            },
        },
        output_defaults: OutputDefaults {
            output_directory: Some("/Books/Library".to_string()),
            output_naming: OutputNamingConfig {
                preset: NamingPreset::CustomTemplate,
                include_year: true,
                custom_template: Some("{author}/{title}".to_string()),
            },
        },
        startup_behavior: StartupBehavior::PinnedDefaults,
        pinned_defaults: Some(PinnedDefaults {
            max_concurrent_jobs: ConcurrencyPreference::Fixed(3),
            encoder_defaults: EncoderDefaults {
                format: AudiobookFormat::MkaOpus,
                intent: AudioIntent::Preserve,
                sample_rate: SampleRateConfig::Auto,
                settings: EncoderSettings {
                    encoder_type: EncoderType::Opus,
                    bitrate_kbps: 48,
                    bitrate_mode: BitrateMode::VbrTarget,
                    channels: ChannelConfig::Stereo,
                    native_aac_speed: 0,
                    faac_profile: FaacProfile::Auto,
                },
            },
            output_defaults: OutputDefaults {
                output_directory: Some("/Books/Pinned".to_string()),
                output_naming: OutputNamingConfig::default(),
            },
        }),
    }
}

fn encoder(
    format: AudiobookFormat,
    intent: AudioIntent,
    sample_rate: SampleRateConfig,
    settings: EncoderSettings,
) -> EncoderDefaults {
    EncoderDefaults {
        format,
        intent,
        settings,
        sample_rate,
    }
}

fn naming(include_year: bool) -> OutputNamingConfig {
    OutputNamingConfig {
        include_year,
        ..OutputNamingConfig::default()
    }
}

fn remember_last_state_settings() -> AppSettings {
    AppSettings {
        keep_awake_while_working: true,
        max_concurrent_jobs: ConcurrencyPreference::Auto,
        default_acquisition_lane: AcquisitionLane::Audible,
        encoder_defaults: encoder(
            AudiobookFormat::M4b,
            AudioIntent::Encode,
            SampleRateConfig::Auto,
            EncoderSettings {
                encoder_type: EncoderType::Faac,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Vbr(100),
                channels: ChannelConfig::Auto,
                native_aac_speed: 0,
                faac_profile: FaacProfile::HeAacV1,
            },
        ),
        output_defaults: OutputDefaults {
            output_directory: None,
            output_naming: naming(true),
        },
        startup_behavior: StartupBehavior::RememberLastState,
        pinned_defaults: None,
    }
}

fn apple_aac_and_opus_settings() -> AppSettings {
    AppSettings {
        keep_awake_while_working: true,
        max_concurrent_jobs: ConcurrencyPreference::Fixed(4),
        default_acquisition_lane: AcquisitionLane::Audible,
        encoder_defaults: encoder(
            AudiobookFormat::M4b,
            AudioIntent::Encode,
            SampleRateConfig::Explicit(48000),
            EncoderSettings {
                encoder_type: EncoderType::AacAt,
                bitrate_kbps: 128,
                bitrate_mode: BitrateMode::Cvbr,
                channels: ChannelConfig::Stereo,
                native_aac_speed: 0,
                faac_profile: FaacProfile::Auto,
            },
        ),
        output_defaults: OutputDefaults {
            output_directory: Some("/Books/Apple".to_string()),
            output_naming: naming(false),
        },
        startup_behavior: StartupBehavior::PinnedDefaults,
        pinned_defaults: Some(PinnedDefaults {
            max_concurrent_jobs: ConcurrencyPreference::Auto,
            encoder_defaults: encoder(
                AudiobookFormat::M4aOpus,
                AudioIntent::Encode,
                SampleRateConfig::Auto,
                EncoderSettings {
                    encoder_type: EncoderType::Opus,
                    bitrate_kbps: 32,
                    bitrate_mode: BitrateMode::VbrTarget,
                    channels: ChannelConfig::Mono,
                    native_aac_speed: 0,
                    faac_profile: FaacProfile::Auto,
                },
            ),
            output_defaults: OutputDefaults::default(),
        }),
    }
}

fn native_aac_settings() -> AppSettings {
    AppSettings {
        keep_awake_while_working: false,
        max_concurrent_jobs: ConcurrencyPreference::Fixed(1),
        default_acquisition_lane: AcquisitionLane::Indexer,
        encoder_defaults: encoder(
            AudiobookFormat::M4b,
            AudioIntent::Auto,
            SampleRateConfig::Auto,
            EncoderSettings {
                encoder_type: EncoderType::Auto,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Cbr,
                channels: ChannelConfig::Auto,
                native_aac_speed: 1,
                faac_profile: FaacProfile::Auto,
            },
        ),
        output_defaults: OutputDefaults::default(),
        startup_behavior: StartupBehavior::RememberLastState,
        pinned_defaults: Some(PinnedDefaults {
            max_concurrent_jobs: ConcurrencyPreference::Fixed(2),
            encoder_defaults: encoder(
                AudiobookFormat::M4b,
                AudioIntent::Encode,
                SampleRateConfig::Explicit(22050),
                EncoderSettings {
                    encoder_type: EncoderType::NativeAac,
                    bitrate_kbps: 96,
                    bitrate_mode: BitrateMode::Cbr,
                    channels: ChannelConfig::Mono,
                    native_aac_speed: 3,
                    faac_profile: FaacProfile::Auto,
                },
            ),
            output_defaults: OutputDefaults {
                output_directory: Some("/Books/Native".to_string()),
                output_naming: naming(true),
            },
        }),
    }
}

#[test]
fn every_sample_settings_file_loads_every_choice_it_holds() {
    let samples = [
        (
            "settings.toml",
            include_str!("samples/settings.toml"),
            chosen_settings(),
        ),
        (
            "remember-last-state.toml",
            include_str!("samples/remember-last-state.toml"),
            remember_last_state_settings(),
        ),
        (
            "apple-aac-and-opus.toml",
            include_str!("samples/apple-aac-and-opus.toml"),
            apple_aac_and_opus_settings(),
        ),
        (
            "native-aac.toml",
            include_str!("samples/native-aac.toml"),
            native_aac_settings(),
        ),
    ];
    for (name, content, expected) in samples {
        let temp = TempDir::new().expect("temp dir");
        std::fs::write(temp.path().join("settings.toml"), content).expect("write sample");

        let loaded = storage::load(temp.path());

        assert!(loaded.reset.is_empty(), "{name}: {:?}", loaded.reset);
        assert_eq!(loaded.settings, expected, "{name}");
        storage::save(temp.path(), &expected).expect("save");
        assert_eq!(
            storage::load(temp.path()).settings,
            expected,
            "{name} saved again"
        );
    }
}

#[test]
fn no_file_loads_as_defaults() {
    let temp = TempDir::new().expect("temp dir");
    let loaded = storage::load(temp.path());
    assert_eq!(loaded.settings, AppSettings::default());
    assert!(loaded.reset.is_empty());
    assert!(loaded.set_aside.is_none());
}

fn sample_edited(from: &str, to: &str) -> String {
    let sample = include_str!("samples/settings.toml");
    assert!(sample.contains(from), "the sample holds {from}");
    sample.replace(from, to)
}

fn load_text(content: &str) -> storage::Loaded {
    let temp = TempDir::new().expect("temp dir");
    std::fs::write(temp.path().join("settings.toml"), content).expect("write settings");
    storage::load(temp.path())
}

fn load_sample_edited(from: &str, to: &str) -> storage::Loaded {
    load_text(&sample_edited(from, to))
}

#[test]
fn a_saved_job_limit_within_the_scheduler_range_loads() {
    let loaded = load_sample_edited("max_concurrent_jobs = 2", "max_concurrent_jobs = 6");
    assert_eq!(
        loaded.settings.max_concurrent_jobs,
        ConcurrencyPreference::Fixed(6)
    );
    assert!(loaded.reset.is_empty());
}

#[test]
fn a_job_limit_this_release_refuses_resets_only_the_job_limit() {
    let loaded = load_sample_edited("max_concurrent_jobs = 2", "max_concurrent_jobs = 99");
    assert_eq!(loaded.reset, vec![SettingsUnit::JobLimit]);
    assert_eq!(
        loaded.settings,
        AppSettings {
            max_concurrent_jobs: ConcurrencyPreference::Auto,
            ..chosen_settings()
        }
    );
}

#[test]
fn a_retired_encoder_resets_only_the_encoder_defaults() {
    let loaded = load_sample_edited("type = \"faac\"", "type = \"retired\"");
    assert_eq!(loaded.reset, vec![SettingsUnit::EncoderDefaults]);
    assert_eq!(
        loaded.settings,
        AppSettings {
            encoder_defaults: EncoderDefaults::default(),
            ..chosen_settings()
        }
    );
}

#[test]
fn a_bad_pinned_value_drops_the_pinned_defaults_and_keeps_the_startup_choice() {
    let loaded = load_sample_edited("max_concurrent_jobs = 3", "max_concurrent_jobs = 99");
    assert_eq!(loaded.reset, vec![SettingsUnit::PinnedDefaults]);
    assert_eq!(
        loaded.settings,
        AppSettings {
            pinned_defaults: None,
            ..chosen_settings()
        }
    );
    assert_eq!(
        loaded.settings.startup_behavior,
        StartupBehavior::PinnedDefaults
    );
}

#[test]
fn fields_from_a_newer_release_are_ignored() {
    let newer = sample_edited(
        "[audio.encoder]\n",
        "[audio.encoder]\nfuture_option = true\n",
    ) + "\n[future_area]\nanything = 1\n";
    let loaded = load_text(&newer);
    assert!(loaded.reset.is_empty());
    assert_eq!(loaded.settings, chosen_settings());
}

fn set_aside_files(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .expect("read config dir")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with("settings.toml.unusable-")
            })
        })
        .collect()
}

#[test]
fn a_file_that_is_not_toml_is_set_aside_before_defaults_are_used() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::write(temp.path().join("settings.toml"), "[general\nnope").expect("damage");

    let loaded = storage::load(temp.path());
    assert_eq!(loaded.settings, AppSettings::default());

    let kept = set_aside_files(temp.path());
    assert_eq!(kept.len(), 1, "{kept:?}");
    let Some(Ok(Some(name))) = loaded.set_aside else {
        panic!("the file was kept aside");
    };
    assert_eq!(kept[0], temp.path().join(name));
    assert_eq!(
        std::fs::read_to_string(&kept[0]).expect("kept"),
        "[general\nnope"
    );
    assert!(!temp.path().join("settings.toml").exists());
}

#[cfg(unix)]
#[test]
fn a_file_abb_may_not_read_is_set_aside_before_defaults_are_used() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("settings.toml");
    std::fs::write(&path, include_str!("samples/settings.toml")).expect("write sample");
    // allow-permission-fake: the test returns early when root can still read it
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    if std::fs::read(&path).is_ok() {
        return;
    }

    assert_eq!(storage::load(temp.path()).settings, AppSettings::default());

    let kept = set_aside_files(temp.path());
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(!path.exists());
}

#[test]
fn a_failed_save_leaves_no_temporary_file() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::create_dir(temp.path().join("settings.toml")).expect("block the destination");

    storage::save(temp.path(), &AppSettings::default()).expect_err("save must fail");

    let leftovers = std::fs::read_dir(temp.path())
        .expect("read config dir")
        .filter(|entry| {
            entry
                .as_ref()
                .is_ok_and(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        })
        .count();
    assert_eq!(leftovers, 0);
}

fn rejection(patch: AppSettingsPatch) -> String {
    AppSettings::default()
        .merge(patch)
        .expect_err("change must be refused")
        .to_string()
}

#[test]
fn invalid_choices_are_refused_in_last_used_and_pinned_defaults() {
    let mut encoder_defaults = EncoderDefaults::default();
    encoder_defaults.settings.encoder_type = EncoderType::NativeAac;
    encoder_defaults.settings.bitrate_mode = BitrateMode::Vbr(3);
    assert!(rejection(AppSettingsPatch {
        encoder_defaults: Some(encoder_defaults.clone()),
        ..Default::default()
    })
    .contains("not supported"));
    assert!(rejection(AppSettingsPatch {
        pinned_defaults: Some(PinnedDefaults {
            max_concurrent_jobs: ConcurrencyPreference::Auto,
            encoder_defaults,
            output_defaults: OutputDefaults::default(),
        }),
        ..Default::default()
    })
    .contains("not supported"));

    let odd_rate = EncoderDefaults {
        sample_rate: SampleRateConfig::Explicit(12345),
        ..Default::default()
    };
    assert!(rejection(AppSettingsPatch {
        encoder_defaults: Some(odd_rate),
        ..Default::default()
    })
    .contains("Unsupported sample rate"));

    let opus_into_m4b = EncoderDefaults {
        format: AudiobookFormat::M4aOpus,
        ..Default::default()
    };
    assert!(rejection(AppSettingsPatch {
        encoder_defaults: Some(opus_into_m4b),
        ..Default::default()
    })
    .contains("must match"));

    assert!(rejection(AppSettingsPatch {
        max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(0)),
        ..Default::default()
    })
    .contains("Max concurrent jobs"));
}

#[test]
fn fixed_concurrency_accepts_the_scheduler_maximum() {
    let max = crate::processing::JobRegistry::max_concurrent_jobs_capabilities().fixed_max;
    let settings = AppSettings::default()
        .merge(AppSettingsPatch {
            max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(max)),
            ..Default::default()
        })
        .expect("maximum accepted");
    assert_eq!(
        settings.max_concurrent_jobs,
        ConcurrencyPreference::Fixed(max)
    );
}

#[test]
fn a_blank_output_folder_means_no_folder() {
    let settings = AppSettings::default()
        .merge(AppSettingsPatch {
            output_defaults: Some(OutputDefaults {
                output_directory: Some("   ".to_string()),
                ..OutputDefaults::default()
            }),
            ..Default::default()
        })
        .expect("blank folder accepted");
    assert_eq!(settings.output_defaults.output_directory, None);
}
