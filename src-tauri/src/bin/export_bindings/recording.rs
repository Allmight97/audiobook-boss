use std::path::{Path, PathBuf};
use std::sync::Arc;

use abb_engine::{DiscardEvents, Engine, EngineConfig};
use serde_json::{Map, Value};

pub struct FixtureEngine {
    pub engine: Engine,
    pub root: PathBuf,
}

impl FixtureEngine {
    pub fn start() -> Self {
        let root =
            std::env::temp_dir().join(format!("abb-contract-fixture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let engine = Engine::start(EngineConfig {
            cache_dir: root.join("cache"),
            config_dir: root.join("config"),
            app_identifier: "com.audiobook-boss.contract-fixture".into(),
            events: Arc::new(DiscardEvents),
            aaxclean_helper: None,
        })
        .expect("failed to start the fixture engine");
        Self { engine, root }
    }

    pub fn finish(self) {
        let _ = tauri::async_runtime::block_on(self.engine.shutdown());
        drop(self.engine);
        let _ = std::fs::remove_dir_all(self.root);
    }
}

pub fn write_fixture(fixtures: &Path, name: &str, value: &Value) {
    std::fs::create_dir_all(fixtures).expect("failed to create the fixtures folder");
    let mut text = serde_json::to_string_pretty(value).expect("fixture serializes");
    text.push('\n');
    std::fs::write(fixtures.join(name), text)
        .unwrap_or_else(|error| panic!("failed to write {name}: {error}"));
}

pub fn export_settings_fresh(fixtures: &Path) {
    let fixture = FixtureEngine::start();
    let snapshot = tauri::async_runtime::block_on(fixture.engine.settings_snapshot());
    let mut value = serde_json::to_value(&snapshot).expect("settings snapshot serializes");
    value["concurrency"]
        .as_object_mut()
        .expect("snapshot has concurrency")
        .remove("effective");
    value["concurrency"]["capabilities"]
        .as_object_mut()
        .expect("concurrency has capabilities")
        .remove("autoEffective");
    write_fixture(fixtures, "settings-fresh.json", &value);
    fixture.finish();
}

pub fn export_session_fresh(fixtures: &Path) {
    let fixture = FixtureEngine::start();
    let snapshot = fixture.engine.session_snapshot();
    let mut value = serde_json::to_value(&snapshot).expect("session snapshot serializes");
    normalize(&mut value, &fixture.root, &mut Vec::new());
    write_fixture(fixtures, "session-fresh.json", &value);
    fixture.finish();
}

/// Title ids are random per run. They are named `title-N` in the order the
/// titles first appear in a `titles.files` list, so the name does not depend
/// on map order; `known` carries that numbering from one step to the next.
pub fn normalize(value: &mut Value, root: &Path, known: &mut Vec<String>) {
    register_titles(value, known);
    rewrite(value, &root.to_string_lossy(), known);
}

fn register_titles(value: &Value, known: &mut Vec<String>) {
    let Some(Value::Array(files)) = value.pointer("/titles/files") else {
        return;
    };
    for id in files.iter().filter_map(|file| file["inputId"].as_str()) {
        if !known.iter().any(|existing| existing == id) {
            known.push(id.to_string());
        }
    }
}

/// `aac_at` exists only in a macOS FFmpeg, so the recorded availability is
/// pinned to what the owner's Mac reports.
fn pin_machine_values(map: &mut Map<String, Value>) {
    if map.contains_key("aacAtAvailable") {
        map.insert("aacAtAvailable".into(), Value::Bool(true));
    }
    if map.get("encoder") == Some(&Value::from("aac_at")) && map.contains_key("available") {
        map.insert("available".into(), Value::Bool(true));
    }
    if map.contains_key("sourceFingerprint") {
        map.insert("sourceFingerprint".into(), Value::from("fixture"));
    }
}

fn rewrite(value: &mut Value, root: &str, ids: &[String]) {
    match value {
        Value::String(text) => *text = rewrite_text(text, root, ids),
        Value::Array(items) => {
            for item in items {
                rewrite(item, root, ids);
            }
        }
        Value::Object(map) => {
            pin_machine_values(map);
            let entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            for (key, mut entry) in entries {
                let key = rewrite_text(&key, root, ids);
                rewrite(&mut entry, root, ids);
                map.insert(key, entry);
            }
        }
        _ => {}
    }
}

fn rewrite_text(text: &str, root: &str, ids: &[String]) -> String {
    if is_uuid(text) {
        return title_name(text, ids);
    }
    text.replace(root, "/fixture")
}

fn title_name(uuid: &str, ids: &[String]) -> String {
    let position = ids
        .iter()
        .position(|known| known == uuid)
        .unwrap_or_else(|| panic!("title id {uuid} is in the snapshot but not in a titles list"));
    format!("title-{}", position + 1)
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, character)| match index {
            8 | 13 | 18 | 23 => character == '-',
            _ => character.is_ascii_hexdigit(),
        })
}
