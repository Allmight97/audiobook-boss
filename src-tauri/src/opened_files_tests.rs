use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use abb_engine::{DiscardEvents, Engine, EngineConfig};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Manager, Url};

use super::{import_held, open, HeldFiles};

struct Fixture {
    root: PathBuf,
    app: App<MockRuntime>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "a leftover temp folder fails no behavior"
        )]
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(name: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!("abb-opened-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("the test folder");
    let app = mock_builder()
        .manage(HeldFiles::default())
        .build(mock_context(noop_assets()))
        .expect("the app builds");
    Fixture { root, app }
}

fn start_engine(root: &std::path::Path) -> Engine {
    Engine::start(EngineConfig {
        cache_dir: root.join("cache"),
        config_dir: root.join("config"),
        app_identifier: "com.audiobook-boss.opened-files-test".into(),
        events: Arc::new(DiscardEvents),
    })
    .expect("the engine starts")
}

fn unsupported_file(root: &std::path::Path) -> Url {
    let path = root.join("cover.png");
    std::fs::write(&path, b"image").expect("the opened file");
    Url::from_file_path(path).expect("a file URL")
}

fn assert_import_reached(engine: &Engine) {
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            while engine
                .session_snapshot()
                .titles
                .expect("titles part")
                .notice
                .is_none()
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the opened file reaches the session");
    });
}

#[test]
fn a_file_opened_on_a_thread_with_no_runtime_reaches_the_session() {
    let fixture = fixture("running");
    let engine = start_engine(&fixture.root);
    fixture.app.manage(engine.clone());

    open(fixture.app.handle(), vec![unsupported_file(&fixture.root)]);

    assert_import_reached(&engine);
}

#[test]
fn a_file_opened_before_the_engine_starts_reaches_the_session() {
    let fixture = fixture("cold-launch");
    open(fixture.app.handle(), vec![unsupported_file(&fixture.root)]);

    let engine = start_engine(&fixture.root);
    import_held(fixture.app.handle(), &engine);

    assert_import_reached(&engine);
}
