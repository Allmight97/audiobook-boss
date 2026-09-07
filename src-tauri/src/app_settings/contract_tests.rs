use super::*;
use crate::audio::{BitrateMode, EncoderType, SampleRateConfig};
use crate::errors::AppError;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

#[test]
fn missing_settings_file_returns_defaults() {
    let temp = TempDir::new().expect("temp dir");

    let settings = get_app_settings(temp.path()).expect("load defaults");

    assert_eq!(settings, AppSettings::default());
}

#[test]
fn update_merges_top_level_patch_and_persists() {
    let temp = TempDir::new().expect("temp dir");
    let patch = AppSettingsPatch {
        max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(3)),
        ..AppSettingsPatch::default()
    };

    let updated = update_app_settings(temp.path(), patch).expect("update settings");
    let reloaded = get_app_settings(temp.path()).expect("reload settings");

    assert_eq!(updated.max_concurrent_jobs, ConcurrencyPreference::Fixed(3));
    assert_eq!(
        reloaded.max_concurrent_jobs,
        ConcurrencyPreference::Fixed(3)
    );
    assert_eq!(
        reloaded.encoder_defaults,
        AppSettings::default().encoder_defaults
    );
}

#[test]
fn serialized_updates_preserve_independent_patches() {
    let temp = TempDir::new().expect("temp dir");
    let config_dir = Arc::new(temp.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(2));

    let concurrency_dir = Arc::clone(&config_dir);
    let concurrency_barrier = Arc::clone(&barrier);
    let concurrency_thread = thread::spawn(move || {
        concurrency_barrier.wait();
        update_app_settings(
            &concurrency_dir,
            AppSettingsPatch {
                max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(2)),
                ..AppSettingsPatch::default()
            },
        )
        .expect("persist concurrency patch");
    });

    let output_dir = Arc::clone(&config_dir);
    let output_barrier = Arc::clone(&barrier);
    let output_thread = thread::spawn(move || {
        let mut output_defaults = AppSettings::default().output_defaults;
        output_defaults.output_directory = Some("/tmp/abb-output".to_string());

        output_barrier.wait();
        update_app_settings(
            &output_dir,
            AppSettingsPatch {
                output_defaults: Some(output_defaults),
                ..AppSettingsPatch::default()
            },
        )
        .expect("persist output patch");
    });

    concurrency_thread.join().expect("concurrency thread");
    output_thread.join().expect("output thread");

    let settings = get_app_settings(temp.path()).expect("reload serialized settings");
    assert_eq!(
        settings.max_concurrent_jobs,
        ConcurrencyPreference::Fixed(2)
    );
    assert_eq!(
        settings.output_defaults.output_directory.as_deref(),
        Some("/tmp/abb-output")
    );
}

#[test]
fn reset_removes_persisted_settings() {
    let temp = TempDir::new().expect("temp dir");
    update_app_settings(
        temp.path(),
        AppSettingsPatch {
            max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(2)),
            ..AppSettingsPatch::default()
        },
    )
    .expect("persist settings");

    let reset = reset_app_settings(temp.path()).expect("reset settings");
    let reloaded = get_app_settings(temp.path()).expect("reload after reset");

    assert_eq!(reset, AppSettings::default());
    assert_eq!(reloaded, AppSettings::default());
}

#[test]
fn malformed_settings_file_fails_explicitly() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::write(temp.path().join("app-settings.json"), "{ nope").expect("write malformed");

    let error = get_app_settings(temp.path()).expect_err("malformed settings should fail");

    assert!(error.to_string().contains("App settings file is invalid"));
}

#[test]
fn invalid_encoder_defaults_are_rejected() {
    let temp = TempDir::new().expect("temp dir");
    let mut encoder_defaults = AppSettings::default().encoder_defaults;
    encoder_defaults.settings.encoder_type = EncoderType::NativeAac;
    encoder_defaults.settings.bitrate_mode = BitrateMode::Vbr(3);

    let error = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            encoder_defaults: Some(encoder_defaults),
            ..AppSettingsPatch::default()
        },
    )
    .expect_err("invalid encoder combo should fail");

    assert!(error.to_string().contains("not supported"));
}

#[test]
fn invalid_sample_rate_defaults_are_rejected() {
    let temp = TempDir::new().expect("temp dir");
    let mut encoder_defaults = AppSettings::default().encoder_defaults;
    encoder_defaults.sample_rate = SampleRateConfig::Explicit(12345);

    let error = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            encoder_defaults: Some(encoder_defaults),
            ..AppSettingsPatch::default()
        },
    )
    .expect_err("invalid sample rate should fail");

    assert!(error.to_string().contains("Unsupported sample rate"));
}

#[test]
fn invalid_fixed_concurrency_is_rejected() {
    let temp = TempDir::new().expect("temp dir");

    let error = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(0)),
            ..AppSettingsPatch::default()
        },
    )
    .expect_err("invalid fixed concurrency should fail");

    assert!(error.to_string().contains("Max concurrent jobs"));
}

#[test]
fn fixed_concurrency_uses_job_registry_capability_bounds() {
    let temp = TempDir::new().expect("temp dir");
    let capabilities = crate::processing::JobRegistry::max_concurrent_jobs_capabilities();

    let settings = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(capabilities.fixed_max)),
            ..AppSettingsPatch::default()
        },
    )
    .expect("max supported fixed concurrency should persist");

    assert_eq!(
        settings.max_concurrent_jobs,
        ConcurrencyPreference::Fixed(capabilities.fixed_max)
    );
}

#[test]
fn blank_output_path_normalizes_to_empty_preference() {
    let temp = TempDir::new().expect("temp dir");
    let mut output_defaults = AppSettings::default().output_defaults;
    output_defaults.output_directory = Some("   ".to_string());

    let settings = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            output_defaults: Some(output_defaults),
            ..AppSettingsPatch::default()
        },
    )
    .expect("normalize blank paths");

    assert_eq!(settings.output_defaults.output_directory, None);
}

#[test]
fn save_failure_removes_temp_file() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::create_dir(temp.path().join("app-settings.json"))
        .expect("create destination directory");

    let error = storage::save(temp.path(), &AppSettings::default())
        .expect_err("directory destination should fail");

    assert!(matches!(error, AppError::Io(_)));
    let leaked_temp_files = std::fs::read_dir(temp.path())
        .expect("read config dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".app-settings-")
        })
        .count();
    assert_eq!(leaked_temp_files, 0);
}

#[test]
fn toolchain_preference_persists_and_blank_path_normalizes_to_unset() {
    let temp = TempDir::new().expect("temp dir");

    let updated = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            toolchain: Some(ToolchainPreferences {
                external_ffmpeg_path: Some("  /opt/user/ffmpeg  ".to_string()),
            }),
            ..AppSettingsPatch::default()
        },
    )
    .expect("update settings");
    let reloaded = get_app_settings(temp.path()).expect("reload settings");

    assert_eq!(
        updated.toolchain.external_ffmpeg_path.as_deref(),
        Some("/opt/user/ffmpeg"),
        "path preference is trimmed and persisted"
    );
    assert_eq!(reloaded.toolchain, updated.toolchain);

    let cleared = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            toolchain: Some(ToolchainPreferences {
                external_ffmpeg_path: Some("   ".to_string()),
            }),
            ..AppSettingsPatch::default()
        },
    )
    .expect("clear settings");

    assert_eq!(
        cleared.toolchain.external_ffmpeg_path, None,
        "blank path normalizes to unset"
    );
}

#[test]
fn settings_file_without_toolchain_field_loads_with_default() {
    let temp = TempDir::new().expect("temp dir");
    // A pre-toolchain settings file must keep loading (serde default).
    let legacy = serde_json::json!({
        "maxConcurrentJobs": {"mode": "auto"},
        "encoderDefaults": serde_json::to_value(EncoderDefaults::default()).expect("encoder json"),
        "outputDefaults": serde_json::to_value(OutputDefaults::default()).expect("output json"),
    });
    std::fs::write(
        temp.path().join("app-settings.json"),
        serde_json::to_string_pretty(&legacy).expect("legacy json"),
    )
    .expect("write legacy settings");

    let settings = get_app_settings(temp.path()).expect("load legacy settings");

    assert_eq!(settings.toolchain, ToolchainPreferences::default());
    // Pre-pinned-defaults files load with today's behavior and no pin.
    assert_eq!(
        settings.startup_behavior,
        StartupBehavior::RememberLastState
    );
    assert_eq!(settings.pinned_defaults, None);
}

#[test]
fn pinned_defaults_and_startup_behavior_persist_and_round_trip() {
    let temp = TempDir::new().expect("temp dir");
    let pinned = PinnedDefaults {
        max_concurrent_jobs: ConcurrencyPreference::Fixed(2),
        encoder_defaults: EncoderDefaults::default(),
        output_defaults: OutputDefaults {
            output_directory: Some("/books/out".to_string()),
            ..OutputDefaults::default()
        },
    };

    let updated = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            pinned_defaults: Some(pinned.clone()),
            startup_behavior: Some(StartupBehavior::PinnedDefaults),
            ..AppSettingsPatch::default()
        },
    )
    .expect("update settings");
    let reloaded = get_app_settings(temp.path()).expect("reload settings");

    assert_eq!(updated.startup_behavior, StartupBehavior::PinnedDefaults);
    assert_eq!(updated.pinned_defaults, Some(pinned));
    assert_eq!(reloaded.startup_behavior, updated.startup_behavior);
    assert_eq!(reloaded.pinned_defaults, updated.pinned_defaults);

    // Switching back to remember-last must not unpin.
    let reverted = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            startup_behavior: Some(StartupBehavior::RememberLastState),
            ..AppSettingsPatch::default()
        },
    )
    .expect("revert startup behavior");
    assert_eq!(
        reverted.startup_behavior,
        StartupBehavior::RememberLastState
    );
    assert!(
        reverted.pinned_defaults.is_some(),
        "pin survives the toggle"
    );
}

#[test]
fn invalid_pinned_defaults_are_rejected_by_shared_validators() {
    let temp = TempDir::new().expect("temp dir");
    let mut encoder_defaults = EncoderDefaults::default();
    // NativeAac with VBR is the same invalid combination the live-value test
    // uses; the pinned snapshot must hit the identical validator.
    encoder_defaults.settings.encoder_type = EncoderType::NativeAac;
    encoder_defaults.settings.bitrate_mode = BitrateMode::Vbr(3);

    let error = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            pinned_defaults: Some(PinnedDefaults {
                max_concurrent_jobs: ConcurrencyPreference::Auto,
                encoder_defaults,
                output_defaults: OutputDefaults::default(),
            }),
            ..AppSettingsPatch::default()
        },
    )
    .expect_err("invalid pinned encoder defaults must be rejected");

    assert!(error.to_string().contains("not supported"));
    let reloaded = get_app_settings(temp.path()).expect("reload settings");
    assert_eq!(reloaded.pinned_defaults, None, "rejected pin is not stored");
}

#[test]
fn default_acquisition_lane_defaults_to_audible_and_persists() {
    let temp = TempDir::new().expect("temp dir");

    let settings = get_app_settings(temp.path()).expect("load defaults");
    assert_eq!(settings.default_acquisition_lane, AcquisitionLane::Audible);

    let updated = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            default_acquisition_lane: Some(AcquisitionLane::Indexer),
            ..AppSettingsPatch::default()
        },
    )
    .expect("update lane");
    let reloaded = get_app_settings(temp.path()).expect("reload settings");

    assert_eq!(updated.default_acquisition_lane, AcquisitionLane::Indexer);
    assert_eq!(reloaded.default_acquisition_lane, AcquisitionLane::Indexer);
}

#[test]
fn settings_file_without_default_acquisition_lane_loads_with_default() {
    let temp = TempDir::new().expect("temp dir");
    let legacy = serde_json::json!({
        "maxConcurrentJobs": {"mode": "auto"},
        "encoderDefaults": serde_json::to_value(EncoderDefaults::default()).expect("encoder json"),
        "outputDefaults": serde_json::to_value(OutputDefaults::default()).expect("output json"),
    });
    std::fs::write(
        temp.path().join("app-settings.json"),
        serde_json::to_string_pretty(&legacy).expect("legacy json"),
    )
    .expect("write legacy settings");

    let settings = get_app_settings(temp.path()).expect("load legacy settings");

    assert_eq!(settings.default_acquisition_lane, AcquisitionLane::Audible);
}
