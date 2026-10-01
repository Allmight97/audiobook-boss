use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;

use super::*;
use crate::audio::{AudioIntent, AudiobookFormat};

struct Rig {
    root: TempDir,
    settings: SettingsRuntime,
    jobs: ManagedJobRegistry,
}

fn config_dir(root: &TempDir) -> PathBuf {
    root.path().join("config")
}

fn start_in(root: TempDir) -> Rig {
    std::fs::create_dir_all(config_dir(&root)).expect("config dir");
    let (settings, jobs) = SettingsRuntime::start(config_dir(&root), PowerManager::default());
    Rig {
        root,
        settings,
        jobs,
    }
}

fn start() -> Rig {
    start_in(TempDir::new().expect("temp dir"))
}

/// Starts over settings a previous run saved.
fn start_with(saved: AppSettingsPatch) -> Rig {
    let root = TempDir::new().expect("temp dir");
    update_app_settings(&config_dir(&root), saved).expect("seed settings");
    start_in(root)
}

fn mp3_defaults() -> EncoderDefaults {
    EncoderDefaults {
        format: AudiobookFormat::Mp3,
        intent: AudioIntent::Preserve,
        ..EncoderDefaults::default()
    }
}

fn output_in(directory: &str) -> OutputDefaults {
    OutputDefaults {
        output_directory: Some(directory.to_string()),
        ..OutputDefaults::default()
    }
}

fn remember_output(directory: &str) -> SettingsIntent {
    SettingsIntent::Remember {
        encoder_defaults: None,
        output_defaults: Some(output_in(directory)),
        default_acquisition_lane: None,
    }
}

impl Rig {
    async fn send(&self, intent: SettingsIntent) -> SettingsReply {
        self.settings.dispatch(intent).await
    }

    fn set_writable(&self, writable: bool) {
        let mode = if writable { 0o755 } else { 0o555 };
        std::fs::set_permissions(
            config_dir(&self.root),
            std::fs::Permissions::from_mode(mode),
        )
        .expect("set config dir permissions");
    }

    fn on_disk(&self) -> AppSettings {
        get_app_settings(&config_dir(&self.root)).expect("read saved settings")
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        // A read-only folder cannot be removed with its TempDir.
        let _ = std::fs::set_permissions(
            config_dir(&self.root),
            std::fs::Permissions::from_mode(0o755),
        );
    }
}

// ---- Startup ----

#[test]
fn a_launch_starts_from_pinned_defaults_only_when_chosen_and_pinned() {
    let pinned = PinnedDefaults {
        max_concurrent_jobs: ConcurrencyPreference::Fixed(2),
        encoder_defaults: mp3_defaults(),
        output_defaults: output_in("/pinned"),
    };
    let last_used = AppSettings {
        output_defaults: output_in("/last-used"),
        ..AppSettings::default()
    };
    let with = |behavior, pinned_defaults| AppSettings {
        startup_behavior: behavior,
        pinned_defaults,
        ..last_used.clone()
    };
    let directory =
        |settings: &AppSettings| startup_defaults(settings).output_defaults.output_directory;

    let remember = StartupBehavior::RememberLastState;
    let use_pinned = StartupBehavior::PinnedDefaults;
    assert_eq!(
        directory(&with(remember, None)).as_deref(),
        Some("/last-used")
    );
    assert_eq!(
        directory(&with(use_pinned, Some(pinned.clone()))).as_deref(),
        Some("/pinned")
    );
    assert_eq!(
        directory(&with(use_pinned, None)).as_deref(),
        Some("/last-used")
    );
    assert_eq!(
        directory(&with(remember, Some(pinned))).as_deref(),
        Some("/last-used")
    );
}

#[tokio::test]
async fn a_launch_applies_the_saved_concurrency_without_rewriting_it() {
    let rig = start_with(AppSettingsPatch {
        max_concurrent_jobs: Some(ConcurrencyPreference::Fixed(1)),
        startup_behavior: Some(StartupBehavior::PinnedDefaults),
        pinned_defaults: Some(PinnedDefaults {
            max_concurrent_jobs: ConcurrencyPreference::Fixed(2),
            encoder_defaults: EncoderDefaults::default(),
            output_defaults: OutputDefaults::default(),
        }),
        ..Default::default()
    });

    let snapshot = rig.settings.snapshot().await;

    assert_eq!(rig.jobs.max_concurrent(), 2);
    assert_eq!(
        snapshot.concurrency.preference,
        ConcurrencyPreference::Fixed(2)
    );
    assert_eq!(snapshot.concurrency.effective, 2);
    // Starting from the pin does not overwrite the last-used choice.
    assert_eq!(
        rig.on_disk().max_concurrent_jobs,
        ConcurrencyPreference::Fixed(1)
    );
}

#[tokio::test]
async fn unreadable_settings_leave_runtime_defaults_and_offer_recovery() {
    let root = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(config_dir(&root)).expect("config dir");
    std::fs::write(
        config_dir(&root).join("app-settings.json"),
        serde_json::json!({
            "maxConcurrentJobs": { "mode": "auto" },
            "encoderDefaults": {
                "settings": { "encoderType": "retired_encoder" },
                "sampleRate": "auto"
            },
            "outputDefaults": { "outputNaming": OutputDefaults::default().output_naming }
        })
        .to_string(),
    )
    .expect("write unreadable settings");
    let rig = start_in(root);

    let snapshot = rig.settings.snapshot().await;
    assert!(snapshot.settings.is_none() && snapshot.startup_defaults.is_none());
    assert!(snapshot.load_error.is_some());
    let plan = snapshot.recovery.expect("recovery is offered");

    // A choice made while the file is unreadable stays in effect and unsaved.
    let reply = rig.send(remember_output("/chosen")).await;
    assert_eq!(reply.outcome, SettingsOutcome::Applied);
    assert!(reply.snapshot.save_error.is_some());

    let reply = rig.send(SettingsIntent::Recover { expected: plan }).await;
    let SettingsOutcome::Recovered { backup_file_name } = reply.outcome else {
        panic!("recovery applies: {:?}", reply.outcome);
    };
    assert!(config_dir(&rig.root).join(backup_file_name).exists());
    assert!(reply.snapshot.load_error.is_none() && reply.snapshot.save_error.is_none());
    // The session's choice was saved on top of the recovered file.
    assert_eq!(
        rig.on_disk().output_defaults.output_directory.as_deref(),
        Some("/chosen")
    );
    assert_eq!(rig.on_disk().encoder_defaults, EncoderDefaults::default());
}

// ---- Acceptance and durability ----

#[tokio::test]
async fn a_change_the_scheduler_refuses_keeps_the_previous_concurrency() {
    let rig = start();
    let before = rig.settings.snapshot().await.concurrency;
    let (_job, _permit) = rig.jobs.register_job().await.expect("a job is running");

    let reply = rig
        .send(SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(1),
        })
        .await;

    assert!(matches!(reply.outcome, SettingsOutcome::Rejected { .. }));
    assert_eq!(reply.snapshot.concurrency, before);
    assert_eq!(
        rig.on_disk().max_concurrent_jobs,
        ConcurrencyPreference::Auto
    );
}

#[tokio::test]
async fn an_accepted_concurrency_survives_a_failed_write_and_retry_only_saves() {
    let rig = start();
    rig.set_writable(false);

    let reply = rig
        .send(SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(2),
        })
        .await;

    assert_eq!(reply.outcome, SettingsOutcome::Applied);
    assert_eq!(reply.snapshot.concurrency.effective, 2);
    assert_eq!(
        reply.snapshot.concurrency.preference,
        ConcurrencyPreference::Fixed(2)
    );
    assert!(reply.snapshot.save_error.is_some());

    // A job starts; retrying the write must not need the scheduler.
    let (_job, _permit) = rig.jobs.register_job().await.expect("a job is running");
    rig.set_writable(true);
    let reply = rig.send(SettingsIntent::Retry).await;

    assert!(reply.snapshot.save_error.is_none());
    assert_eq!(
        rig.on_disk().max_concurrent_jobs,
        ConcurrencyPreference::Fixed(2)
    );
}

#[tokio::test]
async fn retry_writes_the_newest_accepted_defaults() {
    let rig = start();
    rig.set_writable(false);
    rig.send(remember_output("/first")).await;
    let reply = rig
        .send(SettingsIntent::Remember {
            encoder_defaults: Some(mp3_defaults()),
            output_defaults: Some(output_in("/second")),
            default_acquisition_lane: Some(AcquisitionLane::Indexer),
        })
        .await;

    // Accepted values are in effect although nothing reached disk.
    let accepted = reply.snapshot.settings.expect("settings in effect");
    assert_eq!(accepted.output_defaults, output_in("/second"));
    assert_eq!(
        reply.snapshot.default_acquisition_lane,
        AcquisitionLane::Indexer
    );
    assert!(reply.snapshot.save_error.is_some());

    rig.set_writable(true);
    let reply = rig.send(SettingsIntent::Retry).await;

    assert!(reply.snapshot.save_error.is_none());
    let saved = rig.on_disk();
    assert_eq!(saved.output_defaults, output_in("/second"));
    assert_eq!(saved.encoder_defaults, mp3_defaults());
    assert_eq!(saved.default_acquisition_lane, AcquisitionLane::Indexer);
}

#[tokio::test]
async fn defaults_that_do_not_validate_are_refused_and_change_nothing() {
    let rig = start();
    let mismatched = EncoderDefaults {
        format: AudiobookFormat::M4aOpus,
        ..EncoderDefaults::default()
    };

    let reply = rig
        .send(SettingsIntent::Remember {
            encoder_defaults: Some(mismatched),
            output_defaults: None,
            default_acquisition_lane: None,
        })
        .await;

    assert!(matches!(reply.outcome, SettingsOutcome::Rejected { .. }));
    assert_eq!(reply.snapshot.settings, Some(AppSettings::default()));
    assert!(reply.snapshot.save_error.is_none());
}

#[tokio::test]
async fn a_dialog_choice_applies_only_when_it_reaches_disk() {
    let rig = start();
    rig.set_writable(false);

    let reply = rig
        .send(SettingsIntent::SetKeepAwake { enabled: false })
        .await;

    assert!(matches!(reply.outcome, SettingsOutcome::Rejected { .. }));
    assert!(
        reply
            .snapshot
            .settings
            .expect("settings")
            .keep_awake_while_working
    );

    rig.set_writable(true);
    let reply = rig
        .send(SettingsIntent::SetKeepAwake { enabled: false })
        .await;
    assert_eq!(reply.outcome, SettingsOutcome::Applied);
    assert!(!rig.on_disk().keep_awake_while_working);
}

// ---- Pinning ----

#[tokio::test]
async fn pinning_captures_the_current_defaults() {
    let rig = start();
    rig.send(remember_output("/current")).await;

    let reply = rig.send(SettingsIntent::PinCurrentDefaults).await;

    assert_eq!(reply.outcome, SettingsOutcome::Applied);
    let pinned = rig.on_disk().pinned_defaults.expect("pinned");
    assert_eq!(pinned.output_defaults, output_in("/current"));
}

#[tokio::test]
async fn pinning_is_refused_while_current_settings_cannot_be_saved() {
    let rig = start();
    rig.set_writable(false);
    rig.send(remember_output("/unsaved")).await;

    let reply = rig.send(SettingsIntent::PinCurrentDefaults).await;

    let SettingsOutcome::Rejected { error } = reply.outcome else {
        panic!("pinning must be refused");
    };
    assert!(
        error
            .message
            .contains("Save current settings before pinning defaults."),
        "{}",
        error.message
    );
    rig.set_writable(true);
    assert!(rig.on_disk().pinned_defaults.is_none());
}

// ---- Reset ----

#[tokio::test]
async fn reset_restores_defaults_and_automatic_concurrency() {
    let rig = start();
    rig.send(SettingsIntent::SetConcurrency {
        preference: ConcurrencyPreference::Fixed(1),
    })
    .await;
    rig.send(remember_output("/custom")).await;

    let reply = rig.send(SettingsIntent::Reset).await;

    assert_eq!(reply.outcome, SettingsOutcome::Applied);
    assert_eq!(reply.snapshot.settings, Some(AppSettings::default()));
    assert_eq!(
        reply.snapshot.concurrency.preference,
        ConcurrencyPreference::Auto
    );
    assert_eq!(rig.jobs.max_concurrent(), JobRegistry::default_max());
    assert_eq!(rig.on_disk(), AppSettings::default());

    // A choice made after the reset is accepted and saved on its own.
    rig.send(remember_output("/after")).await;
    assert_eq!(rig.on_disk().output_defaults, output_in("/after"));
}

#[tokio::test]
async fn reset_during_an_export_explains_why_and_changes_nothing() {
    let rig = start();
    rig.send(remember_output("/custom")).await;
    let (_job, _permit) = rig.jobs.register_job().await.expect("running export");

    let reply = rig.send(SettingsIntent::Reset).await;

    let SettingsOutcome::Rejected { error } = reply.outcome else {
        panic!("reset waits for running exports");
    };
    assert!(
        error
            .message
            .contains("can't be reset while exports are running"),
        "{}",
        error.message
    );
    assert_eq!(rig.on_disk().output_defaults, output_in("/custom"));
}

#[tokio::test]
async fn a_failed_reset_restores_concurrency_and_keeps_unsaved_choices_retryable() {
    let rig = start();
    rig.send(SettingsIntent::SetConcurrency {
        preference: ConcurrencyPreference::Fixed(1),
    })
    .await;
    rig.set_writable(false);
    rig.send(remember_output("/unsaved")).await;

    let reply = rig.send(SettingsIntent::Reset).await;

    assert!(matches!(reply.outcome, SettingsOutcome::Rejected { .. }));
    assert_eq!(rig.jobs.max_concurrent(), 1);
    assert_eq!(
        reply.snapshot.concurrency.preference,
        ConcurrencyPreference::Fixed(1)
    );

    rig.set_writable(true);
    rig.send(SettingsIntent::Retry).await;
    assert_eq!(rig.on_disk().output_defaults, output_in("/unsaved"));
}
