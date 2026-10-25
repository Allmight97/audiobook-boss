//! The engine's events as Tauri events. Names and payload shapes here are the
//! frontend contract; `ipc_contract.rs` registers them for binding generation.

use abb_engine::app_settings::SettingsSnapshot;
use abb_engine::session::SessionUpdate;
use abb_engine::work_runtime::WorkOperationsUpdate;
use abb_engine::{EngineEvent, EventSink};
use serde::Serialize;
use tauri::Emitter;

// Rust does not allow implementing `tauri_specta::Event` here for a type the
// engine defines, so each engine payload gets a host-owned event type.

/// An accepted operation changed, with the display order as of then.
#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct WorkOperationsUpdateEvent(pub WorkOperationsUpdate);

impl tauri_specta::Event for WorkOperationsUpdateEvent {
    const NAME: &'static str = "work-operations-update";
}

/// What changed in the working session without the frontend asking, or
/// before a change it asked for finished.
#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct SessionUpdateEvent(pub SessionUpdate);

impl tauri_specta::Event for SessionUpdateEvent {
    const NAME: &'static str = "session-update";
}

/// The settings after a change made outside a settings intent.
#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct SettingsUpdateEvent(pub SettingsSnapshot);

impl tauri_specta::Event for SettingsUpdateEvent {
    const NAME: &'static str = "settings-update";
}

/// Forwards engine events to every webview.
pub struct TauriEvents(pub tauri::AppHandle);

// TEMPORARY measurement (ABB_EVENT_STATS=1): count and serialized bytes per event name,
// logged every 5 seconds. Removed before merge.
fn record_event_stats(name: &'static str, payload: &impl Serialize) {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    type Stats = (Instant, BTreeMap<&'static str, (u64, u64)>);
    static STATS: OnceLock<Option<Mutex<Stats>>> = OnceLock::new();
    let Some(stats) = STATS.get_or_init(|| {
        (std::env::var("ABB_EVENT_STATS").as_deref() == Ok("1"))
            .then(|| Mutex::new((Instant::now(), BTreeMap::new())))
    }) else {
        return;
    };
    let bytes = serde_json::to_vec(payload).map_or(0, |json| json.len() as u64);
    let mut stats = stats
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = stats.1.entry(name).or_default();
    entry.0 += 1;
    entry.1 += bytes;
    if stats.0.elapsed() >= Duration::from_secs(5) {
        for (event, (count, bytes)) in &stats.1 {
            log::info!("event-stats name={event} count={count} bytes={bytes} window_s=5");
        }
        stats.1.clear();
        stats.0 = Instant::now();
    }
}

impl TauriEvents {
    fn send<E: tauri_specta::Event + Serialize + Clone>(&self, event: E) {
        record_event_stats(E::NAME, &event);
        if let Err(error) = self.0.emit(E::NAME, event) {
            log::warn!("Failed to emit {}: {error}", E::NAME);
        }
    }
}

impl EventSink for TauriEvents {
    fn emit(&self, event: EngineEvent) {
        match event {
            EngineEvent::WorkOperations(update) => self.send(WorkOperationsUpdateEvent(update)),
            EngineEvent::Session(update) => self.send(SessionUpdateEvent(update)),
            EngineEvent::Settings(snapshot) => self.send(SettingsUpdateEvent(*snapshot)),
        }
    }
}
