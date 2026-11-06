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

#[test]
fn session_and_settings_round_trip_through_the_real_invoke_handler() {
    let root = std::env::temp_dir().join(format!("abb-invoke-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let engine = Engine::start(EngineConfig {
        cache_dir: root.join("cache"),
        config_dir: root.join("config"),
        app_identifier: "com.audiobook-boss.invoke-test".into(),
        events: Arc::new(DiscardEvents),
        aaxclean_helper: None,
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

    let attached = call(&webview, "attach_frontend", json!({})).expect("attach succeeds");
    assert_eq!(attached["client"], 1);
    assert!(attached["session"]["revision"].is_number());
    assert!(attached["settings"]["settings"].is_object());

    let applied = call(
        &webview,
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
        &webview,
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
        &webview,
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
        &webview,
        "open_preview_output",
        json!({ "client": 1, "sequence": 1, "runId": "none" }),
    )
    .expect("the command takes a camelCase run id");

    let replacement = call(&webview, "attach_frontend", json!({})).expect("attach succeeds");
    assert_eq!(replacement["client"], 2);
    let replaced = call(
        &webview,
        "session_dispatch",
        json!({
            "client": 1,
            "sequence": 2,
            "intent": { "kind": "setIncludeYear", "includeYear": true }
        }),
    )
    .expect_err("the replaced window is refused");
    assert!(error_message(&replaced).contains("replaced"));

    let engine = app.state::<Engine>();
    let _ = tauri::async_runtime::block_on(engine.shutdown());
    drop(app);
    let _ = std::fs::remove_dir_all(root);
}
