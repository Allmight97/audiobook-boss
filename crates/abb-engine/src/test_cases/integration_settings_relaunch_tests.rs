use std::fs;
use std::path::Path;

use abb_engine::app_settings::{
    ConcurrencyPreference, SettingsIntent, SettingsOutcome, SettingsSnapshot, StartupBehavior,
};
use abb_engine::audio::{AacDecoder, AudiobookFormat};
use abb_engine::session::{AudioEdit, SessionIntent, TitlePlan};
use abb_engine::ShutdownOutcome;

use super::integration_session_desk::Desk;

const OPUS_KBPS: u16 = 96;

async fn settings(desk: &Desk, intent: SettingsIntent) -> SettingsSnapshot {
    let reply = desk.engine.settings_dispatch(intent).await;
    assert_eq!(reply.outcome, SettingsOutcome::Applied);
    reply.snapshot
}

async fn choose_format_and_bitrate(desk: &Desk, format: AudiobookFormat, kbps: u16) {
    desk.wait_until("the encoders are detected", |desk| {
        desk.engine
            .session_snapshot()
            .audio
            .is_some_and(|audio| audio.capabilities.is_some())
    })
    .await;
    for edit in [AudioEdit::Format(format), AudioEdit::Bitrate(kbps)] {
        desk.send(SessionIntent::SetDefaultAudio { edit }).await;
    }
}

fn block_config_folder(desk: &Desk) {
    let config = desk.root.path().join("config");
    if config.is_dir() {
        fs::remove_dir_all(&config).expect("remove the config folder");
    }
    fs::write(&config, b"").expect("a regular file where the config folder was");
}

fn restore_config_folder(desk: &Desk) {
    let config = desk.root.path().join("config");
    fs::remove_file(&config).expect("remove the blocking file");
    fs::create_dir_all(&config).expect("restore the config folder");
}

async fn quit(desk: &Desk) {
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
}

async fn plan_format(desk: &Desk, book: &Path) -> AudiobookFormat {
    desk.import(book).await;
    desk.wait_until("the title plan leaves pending", |desk| {
        desk.plan() != TitlePlan::Pending
    })
    .await;
    match desk.plan() {
        TitlePlan::Resolved { plan } => plan.format,
        other => panic!("the imported title does not plan: {other:?}"),
    }
}

#[tokio::test]
async fn settings_chosen_before_quitting_are_in_effect_after_relaunch_and_shape_the_next_import() {
    let desk = Desk::new();
    let library = desk.root.path().join("library");
    fs::create_dir_all(&library).expect("library folder");
    choose_format_and_bitrate(&desk, AudiobookFormat::MkaOpus, OPUS_KBPS).await;
    desk.send(SessionIntent::SetOutputDirectory {
        directory: library.to_string_lossy().into_owned(),
    })
    .await;
    desk.send(SessionIntent::SetIncludeYear { include_year: true })
        .await;
    settings(
        &desk,
        SettingsIntent::SetAacDecoder {
            decoder: AacDecoder::Faad,
        },
    )
    .await;
    settings(
        &desk,
        SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(2),
        },
    )
    .await;
    quit(&desk).await;

    let desk = desk.relaunch();

    let snapshot = desk.engine.settings_snapshot().await;
    assert_eq!(
        snapshot.settings.encoder_defaults.format,
        AudiobookFormat::MkaOpus
    );
    assert_eq!(
        snapshot.settings.encoder_defaults.settings.bitrate_kbps,
        OPUS_KBPS
    );
    assert_eq!(
        snapshot
            .settings
            .output_defaults
            .output_directory
            .as_deref(),
        Some(library.to_string_lossy().as_ref())
    );
    assert!(snapshot.settings.output_defaults.output_naming.include_year);
    assert_eq!(snapshot.settings.aac_decoder, AacDecoder::Faad);
    assert_eq!(
        snapshot.concurrency.preference,
        ConcurrencyPreference::Fixed(2)
    );
    assert_eq!(snapshot.concurrency.effective, 2);
    assert_eq!(snapshot.save_error, None);
    assert_eq!(snapshot.recovery, None);

    let book = desk.audiobook(&library.join("alpha.m4b"), 0.2).await;
    let format = plan_format(&desk, &book).await;
    assert_eq!(format, AudiobookFormat::MkaOpus);
    assert_eq!(format.extension(), "mka");
    let titles = desk.engine.session_snapshot().titles.expect("titles part");
    assert_eq!(
        titles.files[0].selected_decoder.as_deref(),
        Some("FAAD3 (bundled)")
    );
    quit(&desk).await;
}

#[tokio::test]
async fn a_launch_from_pinned_defaults_ignores_the_values_last_used_before_quitting() {
    let desk = Desk::new();
    let pinned_folder = desk.root.path().join("pinned");
    let last_used_folder = desk.root.path().join("last-used");
    choose_format_and_bitrate(&desk, AudiobookFormat::MkaOpus, OPUS_KBPS).await;
    desk.send(SessionIntent::SetOutputDirectory {
        directory: pinned_folder.to_string_lossy().into_owned(),
    })
    .await;
    settings(
        &desk,
        SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(2),
        },
    )
    .await;
    settings(&desk, SettingsIntent::PinCurrentDefaults).await;

    choose_format_and_bitrate(&desk, AudiobookFormat::Mp3, 64).await;
    desk.send(SessionIntent::SetOutputDirectory {
        directory: last_used_folder.to_string_lossy().into_owned(),
    })
    .await;
    settings(
        &desk,
        SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(1),
        },
    )
    .await;
    let before_quit = settings(
        &desk,
        SettingsIntent::SetStartupBehavior {
            behavior: StartupBehavior::PinnedDefaults,
        },
    )
    .await;
    assert_eq!(
        before_quit.settings.encoder_defaults.format,
        AudiobookFormat::Mp3,
        "the last-used values are in effect until the next launch"
    );
    quit(&desk).await;

    let desk = desk.relaunch();

    let snapshot = desk.engine.settings_snapshot().await;
    assert_eq!(
        snapshot.settings.encoder_defaults.format,
        AudiobookFormat::MkaOpus
    );
    assert_eq!(
        snapshot.settings.encoder_defaults.settings.bitrate_kbps,
        OPUS_KBPS
    );
    assert_eq!(
        snapshot
            .settings
            .output_defaults
            .output_directory
            .as_deref(),
        Some(pinned_folder.to_string_lossy().as_ref())
    );
    assert_eq!(snapshot.concurrency.effective, 2);
    assert_eq!(
        snapshot.startup_defaults.encoder_defaults.format,
        AudiobookFormat::MkaOpus
    );
    let session = desk.engine.session_snapshot();
    assert_eq!(
        session.audio.expect("audio part").defaults.choice.format,
        AudiobookFormat::MkaOpus
    );
    assert_eq!(
        session.output.expect("output part").directory.as_deref(),
        Some(pinned_folder.to_string_lossy().as_ref())
    );
    quit(&desk).await;
}

#[tokio::test]
async fn quitting_with_a_failed_save_reports_it_and_quitting_again_after_the_folder_returns_saves()
{
    let desk = Desk::new();
    block_config_folder(&desk);

    let snapshot = settings(
        &desk,
        SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(2),
        },
    )
    .await;
    assert!(snapshot.save_error.is_some(), "the failure is reported");
    assert_eq!(
        snapshot.concurrency.effective, 2,
        "the choice stays in effect"
    );
    desk.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Format(AudiobookFormat::MkaOpus),
    })
    .await;

    let ShutdownOutcome::SettingsUnsaved { error } = desk.engine.shutdown().await else {
        panic!("quitting with an unwritable config folder must say settings are unsaved");
    };
    assert!(!error.message.is_empty());

    restore_config_folder(&desk);
    quit(&desk).await;

    let desk = desk.relaunch();
    let snapshot = desk.engine.settings_snapshot().await;
    assert_eq!(snapshot.concurrency.effective, 2);
    assert_eq!(
        snapshot.settings.encoder_defaults.format,
        AudiobookFormat::MkaOpus
    );
    assert_eq!(snapshot.recovery, None);
    quit(&desk).await;
}

#[tokio::test]
async fn a_retry_after_a_failed_save_writes_every_choice_made_while_it_failed() {
    let desk = Desk::new();
    block_config_folder(&desk);
    settings(
        &desk,
        SettingsIntent::SetConcurrency {
            preference: ConcurrencyPreference::Fixed(2),
        },
    )
    .await;
    let failing = settings(
        &desk,
        SettingsIntent::SetAacDecoder {
            decoder: AacDecoder::Faad,
        },
    )
    .await;
    assert!(failing.save_error.is_some());

    restore_config_folder(&desk);
    let retried = settings(&desk, SettingsIntent::Retry).await;
    assert_eq!(retried.save_error, None);
    quit(&desk).await;

    let desk = desk.relaunch();
    let snapshot = desk.engine.settings_snapshot().await;
    assert_eq!(snapshot.concurrency.effective, 2);
    assert_eq!(snapshot.settings.aac_decoder, AacDecoder::Faad);
    quit(&desk).await;
}
