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
fn blank_user_paths_normalize_to_empty_preferences() {
    let temp = TempDir::new().expect("temp dir");
    let mut encoder_defaults = AppSettings::default().encoder_defaults;
    encoder_defaults.external_toolchain.override_path = Some("   ".to_string());
    let mut output_defaults = AppSettings::default().output_defaults;
    output_defaults.output_directory = Some("   ".to_string());

    let settings = update_app_settings(
        temp.path(),
        AppSettingsPatch {
            encoder_defaults: Some(encoder_defaults),
            output_defaults: Some(output_defaults),
            ..AppSettingsPatch::default()
        },
    )
    .expect("normalize blank paths");

    assert_eq!(
        settings.encoder_defaults.external_toolchain.override_path,
        None
    );
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
