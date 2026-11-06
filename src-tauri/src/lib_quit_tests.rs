use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    next_quit_step, quit_prompt, quit_when_saved, settle_or_ask, QuitStep, ShutdownWait,
    SHUTDOWN_WAIT,
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

#[test]
fn quitting_shuts_down_when_the_engine_closes_for_the_consent() {
    let step = next_quit_step(|_| Ok(()), &abb_engine::QuitConsent::default());
    assert_eq!(step, QuitStep::ShutDown);
}

#[test]
fn quitting_asks_again_when_the_work_changed_since_the_user_agreed() {
    let none = abb_engine::QuitConsent::default();
    let first = next_quit_step(|_| Err(exports_running(1, 0)), &none);
    let QuitStep::Ask { prompt, consent } = first else {
        panic!("asked nothing although an export is running");
    };
    assert_eq!(prompt.title, "Exports are still running");

    let second = next_quit_step(
        |agreed| {
            assert_eq!(*agreed, consent);
            Err(exports_running(1, 1))
        },
        &consent,
    );
    let QuitStep::Ask { prompt, consent } = second else {
        panic!("did not ask about the download that started after the first answer");
    };
    assert_eq!(prompt.title, "Work is still running");

    let third = next_quit_step(
        |agreed| {
            assert_eq!(*agreed, consent);
            Ok(())
        },
        &consent,
    );
    assert_eq!(third, QuitStep::ShutDown);
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
