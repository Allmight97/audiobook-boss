use std::path::Path;

use abb_engine::app_settings::SettingsIntent;
use abb_engine::session::SessionIntent;
use abb_engine::Engine;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::recording::{write_fixture, FixtureEngine, Normalizer};
use crate::wav::{write_tagged, write_truncated, Tags};

const SCENARIOS: &str = include_str!("scenarios.json");
const SESSION_PARTS: [&str; 8] = [
    "titles",
    "selection",
    "metadata",
    "lookup",
    "output",
    "remote",
    "remoteLibrary",
    "remoteAcquisition",
];

#[derive(Deserialize)]
struct Scenarios {
    session: Vec<Scenario>,
    settings: Vec<Scenario>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    name: String,
    settings_toml: Option<String>,
    steps: Vec<Value>,
}

pub fn export_scenarios(fixtures: &Path) {
    let scenarios: Scenarios =
        serde_json::from_str(SCENARIOS).expect("scenarios.json matches the scenario shape");
    let session: Vec<Value> = scenarios.session.iter().map(record_session).collect();
    write_fixture(fixtures, "session-scenarios.json", &Value::Array(session));
    let settings: Vec<Value> = scenarios.settings.iter().map(record_settings).collect();
    write_fixture(fixtures, "settings-scenarios.json", &Value::Array(settings));
}

fn write_media(root: &Path) -> std::path::PathBuf {
    let media = root.join("media");
    std::fs::create_dir_all(&media).expect("failed to create the media folder");
    let tagged = |title, artist, genre, date| Tags {
        title,
        artist,
        genre,
        date,
    };
    write_tagged(
        &media.join("alpha.wav"),
        &tagged("Alpha", "Ann Author", "Fantasy", "2001"),
    );
    write_tagged(
        &media.join("beta.wav"),
        &tagged("Beta", "  Ann Author  ", "Fantasy", "2002"),
    );
    write_tagged(
        &media.join("gamma.wav"),
        &tagged("Gamma", "Gus Writer", "Mystery", "2003"),
    );
    write_truncated(&media.join("broken.wav"));
    media
}

fn is_action(step: &Value) -> bool {
    step.get("do").is_some()
}

fn run_action(step: &Value, root: &Path) {
    let config = root.join("config");
    match step["do"].as_str() {
        Some("blockConfig") => {
            let _ = std::fs::remove_dir_all(&config);
            std::fs::write(&config, b"").expect("failed to block the config folder");
        }
        Some("restoreConfig") => {
            std::fs::remove_file(&config).expect("failed to unblock the config folder");
            std::fs::create_dir_all(&config).expect("failed to restore the config folder");
        }
        other => panic!("unknown scenario action {other:?}"),
    }
}

fn real_value(value: &Value, root: &Path, titles: &[String]) -> Value {
    match value {
        Value::String(text) => Value::String(real_text(text, root, titles)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| real_value(item, root, titles))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, entry)| (key.clone(), real_value(entry, root, titles)))
                .collect::<Map<_, _>>(),
        ),
        other => other.clone(),
    }
}

fn real_text(text: &str, root: &Path, titles: &[String]) -> String {
    if let Some(number) = text.strip_prefix("title-") {
        let index: usize = number.parse().expect("title names end in a number");
        return titles
            .get(index - 1)
            .unwrap_or_else(|| panic!("{text} is named before it was imported"))
            .clone();
    }
    match text.strip_prefix("/fixture") {
        Some(rest) => format!("{}{rest}", root.display()),
        None => text.to_string(),
    }
}

fn without_revision(part: &Value) -> Value {
    let mut copy = part.clone();
    if let Some(map) = copy.as_object_mut() {
        map.remove("revision");
    }
    copy
}

/// Each reply carries only the parts that changed since the step before,
/// stamped with the step's number plus one, which is above every revision in
/// the fresh snapshot. Background timing never reaches the file.
fn record_session(scenario: &Scenario) -> Value {
    let fixture = FixtureEngine::start(&scenario.name, scenario.settings_toml.as_deref());
    write_media(&fixture.root);
    let mut normalizer = Normalizer::new(&fixture.root);
    let mut previous = serde_json::to_value(fixture.engine.session_snapshot())
        .expect("session snapshot serializes");
    normalizer.apply(&mut previous);
    let mut recorded = Vec::new();
    for (index, step) in scenario.steps.iter().enumerate() {
        let revision = index as u64 + 2;
        let real = real_value(step, &fixture.root, normalizer.real_title_ids());
        let intent: SessionIntent =
            serde_json::from_value(real).expect("the step is a session intent");
        let reply = tauri::async_runtime::block_on(fixture.engine.session_dispatch(intent));
        let mut snapshot = serde_json::to_value(fixture.engine.session_snapshot())
            .expect("session snapshot serializes");
        normalizer.apply(&mut snapshot);
        let mut outcome = serde_json::to_value(&reply.outcome).expect("outcome serializes");
        normalizer.apply(&mut outcome);
        let mut update = Map::new();
        update.insert("revision".into(), json!(revision));
        for part in SESSION_PARTS {
            let current = &snapshot[part];
            if without_revision(current) == without_revision(&previous[part]) {
                update.insert(part.into(), Value::Null);
                continue;
            }
            let mut changed = current.clone();
            changed["revision"] = json!(revision);
            previous[part] = current.clone();
            update.insert(part.into(), changed);
        }
        recorded.push(json!({ "intent": step, "outcome": outcome, "update": update }));
    }
    fixture.finish();
    json!({ "name": scenario.name, "steps": recorded })
}

fn record_settings(scenario: &Scenario) -> Value {
    let fixture = FixtureEngine::start(&scenario.name, scenario.settings_toml.as_deref());
    let mut normalizer = Normalizer::new(&fixture.root);
    let snapshot = |engine: &Engine, normalizer: &mut Normalizer| {
        let mut value =
            serde_json::to_value(tauri::async_runtime::block_on(engine.settings_snapshot()))
                .expect("settings snapshot serializes");
        normalizer.apply(&mut value);
        value
    };
    let start = snapshot(&fixture.engine, &mut normalizer);
    let mut recorded = Vec::new();
    for step in &scenario.steps {
        if is_action(step) {
            run_action(step, &fixture.root);
            continue;
        }
        let real = real_value(step, &fixture.root, &[]);
        let intent: SettingsIntent =
            serde_json::from_value(real).expect("the step is a settings intent");
        let reply = tauri::async_runtime::block_on(fixture.engine.settings_dispatch(intent));
        let mut reply = serde_json::to_value(&reply).expect("settings reply serializes");
        normalizer.apply(&mut reply);
        recorded.push(json!({
            "intent": step,
            "outcome": reply["outcome"],
            "snapshot": reply["snapshot"],
        }));
    }
    fixture.finish();
    json!({ "name": scenario.name, "start": start, "steps": recorded })
}
