use std::path::{Path, PathBuf};
use std::sync::Arc;

use abb_engine::{DiscardEvents, Engine, EngineConfig};
use serde_json::{Map, Value};

pub struct FixtureEngine {
    pub engine: Engine,
    pub root: PathBuf,
}

impl FixtureEngine {
    pub fn start(name: &str, settings_toml: Option<&str>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "abb-contract-fixture-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        if let Some(toml) = settings_toml {
            std::fs::create_dir_all(root.join("config"))
                .expect("failed to create the config folder");
            std::fs::write(root.join("config/settings.toml"), toml)
                .expect("failed to write settings.toml");
        }
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

pub fn export_fresh(fixtures: &Path) {
    let fixture = FixtureEngine::start("fresh", None);
    let mut normalizer = Normalizer::new(&fixture.root);
    let mut settings = serde_json::to_value(tauri::async_runtime::block_on(
        fixture.engine.settings_snapshot(),
    ))
    .expect("settings snapshot serializes");
    normalizer.apply(&mut settings);
    write_fixture(fixtures, "settings-fresh.json", &settings);
    let mut session = serde_json::to_value(fixture.engine.session_snapshot())
        .expect("session snapshot serializes");
    normalizer.apply(&mut session);
    write_fixture(fixtures, "session-fresh.json", &session);
    fixture.finish();
}

/// Rewrites what differs between runs or machines in a recorded snapshot.
/// Title ids are random per run; they are named `title-N` in the order the
/// titles first appear in a `titles.files` list, so the name does not depend
/// on map order. The numbering carries from one snapshot to the next.
pub struct Normalizer {
    roots: Vec<String>,
    titles: Vec<String>,
}

const MACHINE_JOBS: u64 = 4;

impl Normalizer {
    pub fn new(root: &Path) -> Self {
        let mut roots = Vec::new();
        if let Ok(canonical) = root.canonicalize() {
            roots.push(canonical.to_string_lossy().into_owned());
        }
        roots.push(root.to_string_lossy().into_owned());
        Self {
            roots,
            titles: Vec::new(),
        }
    }

    pub fn apply(&mut self, value: &mut Value) {
        self.register_titles(value);
        self.rewrite(value);
    }

    pub fn real_title_ids(&self) -> &[String] {
        &self.titles
    }

    fn register_titles(&mut self, value: &Value) {
        let Some(Value::Array(files)) = value.pointer("/titles/files") else {
            return;
        };
        for id in files.iter().filter_map(|file| file["inputId"].as_str()) {
            if !self.titles.iter().any(|known| known == id) {
                self.titles.push(id.to_string());
            }
        }
    }

    fn rewrite(&self, value: &mut Value) {
        match value {
            Value::String(text) => *text = self.rewrite_text(text),
            Value::Array(items) => {
                for item in items {
                    self.rewrite(item);
                }
            }
            Value::Object(map) => {
                pin_machine_values(map);
                let entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
                for (key, mut entry) in entries {
                    let key = self.rewrite_text(&key);
                    self.rewrite(&mut entry);
                    map.insert(key, entry);
                }
            }
            _ => {}
        }
    }

    fn rewrite_text(&self, text: &str) -> String {
        if is_uuid(text) {
            let position = self
                .titles
                .iter()
                .position(|known| known == text)
                .unwrap_or_else(|| {
                    panic!("title id {text} is in a snapshot but not in a titles list")
                });
            return format!("title-{}", position + 1);
        }
        self.roots.iter().fold(text.to_string(), |text, root| {
            text.replace(root, "/fixture")
        })
    }
}

/// `aac_at` exists only in a macOS FFmpeg, so the recorded availability is
/// pinned to what the owner's Mac reports. The automatic job count follows
/// the machine's cores.
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
    if map.contains_key("autoEffective") {
        map.insert("autoEffective".into(), Value::from(MACHINE_JOBS));
    }
    if map.contains_key("effective")
        && map.get("preference") == Some(&serde_json::json!({"mode": "auto"}))
    {
        map.insert("effective".into(), Value::from(MACHINE_JOBS));
    }
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, character)| match index {
            8 | 13 | 18 | 23 => character == '-',
            _ => character.is_ascii_hexdigit(),
        })
}
