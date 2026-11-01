use std::sync::atomic::{AtomicUsize, Ordering};

use super::{quit_prompt, settle_or_ask, SHUTDOWN_WAIT};

#[test]
fn quitting_asks_first_when_it_would_stop_downloads_or_exports() {
    let running = |exports, acquisitions| abb_engine::RunningWork {
        exports,
        acquisitions,
        ..Default::default()
    };
    assert_eq!(quit_prompt(&running(0, 0)), None);
    let (title, message) = quit_prompt(&running(0, 1)).expect("asks about the download");
    assert_eq!(title, "Downloads are still running");
    assert!(
        message.starts_with("1 Audible download is still running."),
        "{message}"
    );
    let (title, message) = quit_prompt(&running(2, 2)).expect("asks about both");
    assert_eq!(title, "Work is still running");
    assert!(message.contains("2 exports are") && message.contains("2 Audible downloads are"));
    let waiting = abb_engine::RunningWork {
        waiting_writes: 1,
        ..Default::default()
    };
    let (title, message) = quit_prompt(&waiting).expect("asks about the waiting save");
    assert_eq!(title, "Saves are still waiting");
    assert!(message.contains("1 file"), "{message}");
}

#[tokio::test(start_paused = true)]
async fn an_unsettled_shutdown_waits_until_the_user_chooses_to_quit() {
    let asked = AtomicUsize::new(0);
    let settled = settle_or_ask(std::future::pending::<()>(), || {
        let times = asked.fetch_add(1, Ordering::SeqCst) + 1;
        async move { times < 2 }
    })
    .await;
    assert!(settled.is_none());
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn a_shutdown_that_settles_in_time_asks_nothing() {
    let settled = settle_or_ask(tokio::time::sleep(SHUTDOWN_WAIT / 2), || async {
        panic!("asked although shutdown settled")
    })
    .await;
    assert!(settled.is_some());
}
