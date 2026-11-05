use std::path::{Path, PathBuf};
use std::sync::Arc;

use abb_engine::{DiscardEvents, Engine, EngineConfig};

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/generated/tauri.ts")
        });
    audiobook_boss_lib::ipc_contract::export_typescript_bindings(&output)
        .expect("failed to export tauri-specta TypeScript bindings");
    let fixtures = output
        .parent()
        .expect("bindings output has a parent folder")
        .join("fixtures");
    export_settings_fixture(&fixtures);
}

fn export_settings_fixture(fixtures: &Path) {
    let root = std::env::temp_dir().join(format!("abb-contract-fixture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let engine = Engine::start(EngineConfig {
        cache_dir: root.join("cache"),
        config_dir: root.join("config"),
        app_identifier: "com.audiobook-boss.contract-fixture".into(),
        events: Arc::new(DiscardEvents),
        aaxclean_helper: None,
    })
    .expect("failed to start the fixture engine");
    let snapshot = tauri::async_runtime::block_on(engine.settings_snapshot());
    let mut value = serde_json::to_value(&snapshot).expect("settings snapshot serializes");
    value["concurrency"]
        .as_object_mut()
        .expect("snapshot has concurrency")
        .remove("effective");
    value["concurrency"]["capabilities"]
        .as_object_mut()
        .expect("concurrency has capabilities")
        .remove("autoEffective");
    std::fs::create_dir_all(fixtures).expect("failed to create the fixtures folder");
    let mut text = serde_json::to_string_pretty(&value).expect("fixture serializes");
    text.push('\n');
    std::fs::write(fixtures.join("settings-fresh.json"), text)
        .expect("failed to write settings-fresh.json");
    let _ = tauri::async_runtime::block_on(engine.shutdown());
    drop(engine);
    let _ = std::fs::remove_dir_all(root);
}
