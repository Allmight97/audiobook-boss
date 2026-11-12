use std::sync::atomic::{AtomicUsize, Ordering};

use abb_engine::{QuitConsent, RunningWork};

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Manager};

use super::{
    quit_prompt, quit_when_saved, settle_or_ask, settle_unheld_exit, Quit, QuitAction, QuitInput,
    ShutdownWait, SHUTDOWN_WAIT,
};

#[test]
fn quitting_asks_first_when_it_would_stop_downloads_or_exports() {
    let running = |exports, acquisitions| abb_engine::RunningWork {
        exports,
        acquisitions,
        ..Default::default()
    };
    let prompt = quit_prompt(&running(0, 1));
    assert_eq!(prompt.title, "Downloads are still running");
    assert!(
        prompt
            .message
            .starts_with("1 Audible download is still running."),
        "{}",
        prompt.message
    );
    let prompt = quit_prompt(&running(2, 2));
    assert_eq!(prompt.title, "Work is still running");
    assert!(
        prompt.message.contains("2 exports are")
            && prompt.message.contains("2 Audible downloads are")
    );
    let waiting = abb_engine::RunningWork {
        waiting_writes: 1,
        ..Default::default()
    };
    let prompt = quit_prompt(&waiting);
    assert_eq!(prompt.title, "Saves are still waiting");
    assert!(prompt.message.contains("1 file"), "{}", prompt.message);
}

fn exports_running(exports: usize, acquisitions: usize) -> abb_engine::RunningWork {
    abb_engine::RunningWork {
        exports,
        acquisitions,
        ..Default::default()
    }
}

fn closes(_: &QuitConsent) -> Result<(), RunningWork> {
    Ok(())
}

fn not_asked(_: &QuitConsent) -> Result<(), RunningWork> {
    panic!("the engine was asked although the request is held")
}

fn asks(quit: &Quit, input: QuitInput, running: RunningWork) -> QuitConsent {
    match quit.advance(input, |_| Err(running)) {
        QuitAction::Ask(running) => running.consent,
        other => panic!("asked nothing although work is running: {other:?}"),
    }
}

#[test]
fn a_second_request_while_the_prompt_is_open_opens_nothing() {
    let quit = Quit::default();
    asks(&quit, QuitInput::Request, exports_running(1, 0));

    assert_eq!(
        quit.advance(QuitInput::Request, not_asked),
        QuitAction::Nothing
    );
}

#[test]
fn keep_open_returns_to_running_so_a_later_request_asks_again() {
    let quit = Quit::default();
    asks(&quit, QuitInput::Request, exports_running(1, 0));

    assert_eq!(
        quit.advance(QuitInput::KeepOpen, not_asked),
        QuitAction::Nothing
    );
    asks(&quit, QuitInput::Request, exports_running(1, 0));
}

#[test]
fn quitting_shuts_down_when_the_engine_closes_for_the_consent() {
    let quit = Quit::default();
    assert_eq!(
        quit.advance(QuitInput::Request, closes),
        QuitAction::ShutDown
    );
    assert_eq!(
        quit.advance(QuitInput::Request, not_asked),
        QuitAction::Nothing
    );
}

#[test]
fn quitting_asks_again_when_the_work_changed_since_the_user_agreed() {
    let quit = Quit::default();
    let consent = asks(&quit, QuitInput::Request, exports_running(1, 0));

    let next = quit.advance(QuitInput::QuitAnyway(consent.clone()), |agreed| {
        assert_eq!(*agreed, consent);
        Err(exports_running(1, 1))
    });
    let QuitAction::Ask(running) = next else {
        panic!("did not ask about the download that started after the first answer");
    };
    assert_eq!(quit_prompt(&running).title, "Work is still running");
    let consent = running.consent;

    let last = quit.advance(QuitInput::QuitAnyway(consent.clone()), |agreed| {
        assert_eq!(*agreed, consent);
        Ok(())
    });
    assert_eq!(last, QuitAction::ShutDown);
}

#[test]
fn once_exit_is_committed_a_request_passes_through() {
    let quit = Quit::default();
    quit.advance(QuitInput::Request, closes);
    quit.advance(QuitInput::ExitCommitted, not_asked);

    assert_eq!(
        quit.advance(QuitInput::Request, not_asked),
        QuitAction::Release
    );
}

#[tokio::test(start_paused = true)]
async fn an_unsettled_shutdown_waits_until_the_user_chooses_to_quit() {
    let asked = AtomicUsize::new(0);
    let settled = settle_or_ask(std::future::pending::<()>(), || {
        let times = asked.fetch_add(1, Ordering::SeqCst) + 1;
        async move { times < 2 }
    })
    .await;
    assert!(matches!(settled, ShutdownWait::QuitChosen));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn a_shutdown_that_settles_in_time_asks_nothing() {
    let settled = settle_or_ask(tokio::time::sleep(SHUTDOWN_WAIT / 2), || async {
        panic!("asked although shutdown settled")
    })
    .await;
    assert!(matches!(settled, ShutdownWait::Finished(())));
}

fn unsaved() -> abb_engine::ShutdownOutcome {
    abb_engine::ShutdownOutcome::SettingsUnsaved {
        error: abb_engine::AppErrorEnvelope::from(abb_engine::AppError::General(
            "disk is read-only".to_string(),
        )),
    }
}

#[tokio::test]
async fn quitting_retries_the_settings_save_until_it_settles() {
    let shutdowns = AtomicUsize::new(0);
    let reasons = std::sync::Mutex::new(Vec::new());
    quit_when_saved(
        || {
            let attempt = shutdowns.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt == 0 {
                    unsaved()
                } else {
                    abb_engine::ShutdownOutcome::Settled
                }
            }
        },
        || async { panic!("asked to keep waiting although shutdown returned") },
        |reason| {
            reasons.lock().expect("reasons").push(reason);
            async { true }
        },
    )
    .await;
    assert_eq!(shutdowns.load(Ordering::SeqCst), 2);
    let reasons = reasons.into_inner().expect("reasons");
    assert_eq!(reasons.len(), 1);
    assert!(reasons[0].contains("disk is read-only"), "{reasons:?}");
}

#[tokio::test]
async fn quitting_anyway_with_unsaved_settings_shuts_down_once() {
    let shutdowns = AtomicUsize::new(0);
    quit_when_saved(
        || {
            shutdowns.fetch_add(1, Ordering::SeqCst);
            async { unsaved() }
        },
        || async { panic!("asked to keep waiting although shutdown returned") },
        |_| async { false },
    )
    .await;
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

fn app_with_engine(name: &str) -> (App<MockRuntime>, abb_engine::Engine, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("abb-quit-{name}-{}", std::process::id()));
    let engine = abb_engine::Engine::start(abb_engine::EngineConfig {
        cache_dir: root.join("cache"),
        config_dir: root.join("config"),
        app_identifier: "com.audiobook-boss.quit-test".into(),
        events: std::sync::Arc::new(abb_engine::DiscardEvents),
    })
    .expect("the engine starts");
    let app = mock_builder()
        .manage(Quit::default())
        .manage(engine.clone())
        .build(mock_context(noop_assets()))
        .expect("the app builds");
    (app, engine, root)
}

fn engine_is_open(engine: &abb_engine::Engine) -> bool {
    tauri::async_runtime::block_on(engine.open_audio_files(Vec::new())).is_ok()
}

fn remove(root: &std::path::Path) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "a leftover temp folder fails no behavior"
    )]
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_exit_no_hold_saw_settles_the_engine_first() {
    let (app, engine, root) = app_with_engine("unheld");
    assert!(engine_is_open(&engine));

    settle_unheld_exit(app.handle());

    assert!(!engine_is_open(&engine));
    remove(&root);
}

#[test]
fn an_exit_after_the_held_quit_settles_nothing_again() {
    let (app, engine, root) = app_with_engine("held");
    let quit = app.state::<Quit>();
    quit.advance(QuitInput::Request, closes);
    quit.advance(QuitInput::ExitCommitted, not_asked);

    settle_unheld_exit(app.handle());

    assert!(engine_is_open(&engine));
    remove(&root);
}
