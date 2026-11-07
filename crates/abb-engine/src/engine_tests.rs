use super::*;
use std::sync::Mutex;

fn config(root: &Path) -> EngineConfig {
    EngineConfig {
        cache_dir: root.join("cache"),
        config_dir: root.join("config"),
        app_identifier: "com.audiobook-boss.test".to_string(),
        events: Arc::new(crate::DiscardEvents),
    }
}

#[tokio::test]
async fn a_reset_is_refused_while_the_indexer_holds_its_connection_and_applies_after() {
    let root = tempfile::TempDir::new().expect("engine root");
    let engine = Engine::start(config(root.path())).expect("start headless engine");
    let revision = engine.settings_snapshot().await.revision;

    let held = engine
        .inner
        .remote_source
        .reset_lease()
        .expect("an idle Indexer");
    let refused = engine.settings_dispatch(SettingsIntent::Reset).await;
    drop(held);

    assert!(matches!(
        &refused.outcome,
        SettingsOutcome::Rejected { error } if error.message.contains("before resetting settings")
    ));
    assert_eq!(refused.snapshot.revision, revision);

    let applied = engine.settings_dispatch(SettingsIntent::Reset).await;
    assert_eq!(applied.outcome, SettingsOutcome::Applied);
    assert!(
        engine.inner.remote_source.reset_lease().is_ok(),
        "the Reset released its hold"
    );
}

#[derive(Default)]
struct QuitHints(Mutex<Vec<u32>>);

impl EventSink for QuitHints {
    fn emit(&self, event: crate::EngineEvent) {
        if let crate::EngineEvent::QuitPressAgain { window_ms } = event {
            self.0.lock().expect("hints").push(window_ms);
        }
    }
}

#[tokio::test]
async fn the_quit_shortcut_quits_on_the_second_press_and_tells_the_host_after_the_first() {
    let root = tempfile::TempDir::new().expect("engine root");
    let hints = Arc::new(QuitHints::default());
    let engine = Engine::start(EngineConfig {
        events: hints.clone(),
        ..config(root.path())
    })
    .expect("start headless engine");

    assert_eq!(engine.quit_pressed(), QuitPress::PressAgain);
    assert_eq!(engine.quit_pressed(), QuitPress::Proceed);

    assert_eq!(*hints.0.lock().expect("hints"), vec![3000]);
}

#[tokio::test]
async fn the_quit_shortcut_quits_on_one_press_once_the_user_chooses_that() {
    let root = tempfile::TempDir::new().expect("engine root");
    let engine = Engine::start(config(root.path())).expect("start headless engine");

    engine
        .settings_dispatch(SettingsIntent::SetPressQuitTwice { enabled: false })
        .await;

    assert_eq!(engine.quit_pressed(), QuitPress::Proceed);
}

#[tokio::test]
async fn opening_an_unsupported_file_says_no_supported_files() {
    let root = tempfile::TempDir::new().expect("engine root");
    let engine = Engine::start(config(root.path())).expect("start headless engine");
    let unsupported = root.path().join("cover.jpg");
    std::fs::write(&unsupported, b"image").expect("unsupported file");

    engine
        .open_audio_files(vec![unsupported])
        .await
        .expect("opened files admitted");

    let notice = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let titles = engine.session_snapshot().titles.expect("titles part");
            if let Some(notice) = titles.notice {
                break notice;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the open leaves a notice");
    assert!(matches!(
        notice,
        crate::session::InputNotice::NoSupportedFiles { .. }
    ));
}
