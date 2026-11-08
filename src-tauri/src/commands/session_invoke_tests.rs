use std::sync::Arc;

use abb_engine::{DiscardEvents, Engine, EngineConfig};
use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

use super::FrontendLink;

fn call(webview: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().expect("a valid origin"),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|reply| reply.deserialize::<Value>().expect("a JSON reply"))
}

fn error_message(error: &Value) -> &str {
    error["message"].as_str().expect("an error envelope")
}

struct InvokeApp {
    app: tauri::App<MockRuntime>,
    webview: WebviewWindow<MockRuntime>,
    root: std::path::PathBuf,
}

impl InvokeApp {
    fn start(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("abb-invoke-test-{name}-{}", std::process::id()));
        #[expect(
            clippy::let_underscore_must_use,
            reason = "the directory is usually absent before the test"
        )]
        let _ = std::fs::remove_dir_all(&root);
        let engine = Engine::start(EngineConfig {
            cache_dir: root.join("cache"),
            config_dir: root.join("config"),
            app_identifier: "com.audiobook-boss.invoke-test".into(),
            events: Arc::new(DiscardEvents),
        })
        .expect("the engine starts");
        let app = mock_builder()
            .invoke_handler(crate::ipc_contract::builder::<MockRuntime>().invoke_handler())
            .manage(FrontendLink::default())
            .manage(engine)
            .build(mock_context(noop_assets()))
            .expect("the app builds");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("the window builds");
        Self { app, webview, root }
    }

    fn finish(self) {
        let engine = self.app.state::<Engine>();
        #[expect(
            clippy::let_underscore_must_use,
            reason = "shutdown outcome is not under test"
        )]
        let _ = tauri::async_runtime::block_on(engine.shutdown());
        drop(self.app);
        #[expect(
            clippy::let_underscore_must_use,
            reason = "best-effort removal of the test directory"
        )]
        let _ = std::fs::remove_dir_all(self.root);
    }
}

#[test]
fn session_and_settings_round_trip_through_the_real_invoke_handler() {
    let harness = InvokeApp::start("round-trip");
    let webview = &harness.webview;

    let attached = call(webview, "attach_frontend", json!({})).expect("attach succeeds");
    assert_eq!(attached["client"], 1);
    assert!(attached["session"]["revision"].is_number());
    assert!(attached["settings"]["settings"].is_object());
    assert!(attached["workOperations"]["operations"].is_array());

    let applied = call(
        webview,
        "session_dispatch",
        json!({
            "client": 1,
            "sequence": 0,
            "intent": { "kind": "setIncludeYear", "includeYear": true }
        }),
    )
    .expect("the session intent is answered");
    assert_eq!(applied["outcome"]["kind"], "applied");
    assert_eq!(applied["update"]["output"]["includeYear"], true);

    let saved = call(
        webview,
        "settings_dispatch",
        json!({
            "client": 1,
            "sequence": 0,
            "intent": { "kind": "setKeepAwake", "enabled": false }
        }),
    )
    .expect("the settings intent is answered");
    assert_eq!(
        saved["snapshot"]["settings"]["keepAwakeWhileWorking"],
        false
    );

    let repeated = call(
        webview,
        "session_dispatch",
        json!({
            "client": 1,
            "sequence": 0,
            "intent": { "kind": "setIncludeYear", "includeYear": false }
        }),
    )
    .expect_err("a repeated number is refused");
    assert_eq!(repeated["code"], "invalid_input");
    assert!(error_message(&repeated).contains("later ones"));

    call(
        webview,
        "open_preview_output",
        json!({ "client": 1, "sequence": 1, "runId": "none" }),
    )
    .expect("the command takes a camelCase run id");

    let replacement = call(webview, "attach_frontend", json!({})).expect("attach succeeds");
    assert_eq!(replacement["client"], 2);
    let replaced = call(
        webview,
        "session_dispatch",
        json!({
            "client": 1,
            "sequence": 2,
            "intent": { "kind": "setIncludeYear", "includeYear": true }
        }),
    )
    .expect_err("the replaced window is refused");
    assert!(error_message(&replaced).contains("replaced"));

    harness.finish();
}

#[test]
fn the_generic_session_route_cannot_claim_a_preview_output() {
    let harness = InvokeApp::start("claim");
    call(&harness.webview, "attach_frontend", json!({})).expect("attach succeeds");

    let claimed = call(
        &harness.webview,
        "session_dispatch",
        json!({
            "client": 1,
            "sequence": 0,
            "intent": { "kind": "takePreviewOutput", "runId": "none" }
        }),
    )
    .expect_err("the generic route cannot claim a preview output");
    assert!(
        claimed.to_string().contains("takePreviewOutput"),
        "{claimed}"
    );

    harness.finish();
}
