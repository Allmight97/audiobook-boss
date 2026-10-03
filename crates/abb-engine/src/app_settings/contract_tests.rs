use super::*;
use crate::audio::{
    AudioIntent, AudiobookFormat, BitrateMode, ChannelConfig, EncoderSettings, EncoderType,
    FaacProfile, SampleRateConfig,
};
use crate::output_artifact::{NamingPreset, OutputNamingConfig};
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

// A saved file must keep meaning what it meant. Changing how a setting is
// saved fails here; keep the sample loading, or the change says what each
// saved choice becomes.
#[test]
fn the_sample_settings_file_loads_every_choice_it_holds() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::write(
        temp.path().join("settings.toml"),
        include_str!("samples/settings.toml"),
    )
    .expect("write sample");

    assert_eq!(get_app_settings(temp.path()), chosen_settings());
}

#[test]
fn saved_settings_load_back_exactly() {
    let temp = TempDir::new().expect("temp dir");
    save_app_settings(temp.path(), &chosen_settings()).expect("save");

    assert_eq!(get_app_settings(temp.path()), chosen_settings());
}

#[test]
fn no_file_and_a_damaged_file_both_load_as_defaults() {
    let temp = TempDir::new().expect("temp dir");
    assert_eq!(get_app_settings(temp.path()), AppSettings::default());

    std::fs::write(temp.path().join("settings.toml"), "[general\nnope").expect("damage");
    assert_eq!(get_app_settings(temp.path()), AppSettings::default());
}

#[test]
fn a_failed_save_leaves_no_temporary_file() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::create_dir(temp.path().join("settings.toml")).expect("block the destination");

    save_app_settings(temp.path(), &AppSettings::default()).expect_err("save must fail");

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
